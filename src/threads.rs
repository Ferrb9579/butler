//! Durable conversation records. The workspace binding is validated on reopen.

use crate::scratch::ScratchWorkspaces;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const RECORD_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Workspace {
    Scratch,
    Local(PathBuf),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace: Workspace,
    pub prompt: String,
    pub reply: String,
    #[serde(default)]
    pub follow_ups: Vec<Exchange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Exchange {
    pub prompt: String,
    pub reply: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadSummary {
    pub id: Uuid,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct ThreadStore {
    state: PathBuf,
}

impl ThreadStore {
    pub fn new(state: impl Into<PathBuf>) -> Self {
        Self {
            state: state.into(),
        }
    }

    /// Store the first exchange once. Never overwrite later replies on retries.
    pub fn save(&self, turn: &Conversation) -> io::Result<()> {
        validate(turn)?;
        if !turn.follow_ups.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use append for subsequent exchanges.",
            ));
        }
        self.workspace_directory(turn)?;
        let _lock = self.lock(turn.id)?;
        if let Some(saved) = self.load(turn.id)? {
            return if saved.workspace == turn.workspace
                && saved.prompt == turn.prompt
                && saved.reply == turn.reply
            {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Thread already stores a different first exchange.",
                ))
            };
        }
        self.write(turn)
    }

    /// Append against the transcript the caller saw. Identical retries do not duplicate replies.
    pub fn append(&self, expected: &Conversation, exchange: Exchange) -> io::Result<Conversation> {
        let _lock = self.lock(expected.id)?;
        let mut saved = self.load(expected.id)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Conversation no longer exists.")
        })?;
        self.workspace_directory(&saved)?;
        let index = expected.follow_ups.len();
        let same_prefix = saved.workspace == expected.workspace
            && saved.prompt == expected.prompt
            && saved.reply == expected.reply
            && saved.follow_ups.starts_with(&expected.follow_ups);
        if same_prefix && saved.follow_ups.get(index) == Some(&exchange) {
            return Ok(saved);
        }
        if !same_prefix || saved.follow_ups.len() != index {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Conversation has moved on. Reopen it before sending.",
            ));
        }
        saved.follow_ups.push(exchange);
        validate(&saved)?;
        self.write(&saved)?;
        Ok(saved)
    }

    fn lock(&self, id: Uuid) -> io::Result<File> {
        let folder = self.folder();
        fs::create_dir_all(&folder)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(folder.join(format!("{id}.lock")))?;
        lock.lock()?;
        Ok(lock)
    }

    fn write(&self, turn: &Conversation) -> io::Result<()> {
        let data = serde_json::to_vec(&(2_u8, turn)).map_err(io::Error::other)?;
        if data.len() as u64 > RECORD_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Thread record is too large.",
            ));
        }
        let folder = self.folder();
        let target = self.record(turn.id);
        let tmp = folder.join(format!(".turn-{}", Uuid::new_v4()));
        let outcome = (|| {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            file.write_all(&data)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp, target)?;
            #[cfg(unix)]
            File::open(&folder)?.sync_all()?;
            Ok(())
        })();
        if outcome.is_err() {
            let _ = fs::remove_file(tmp);
        }
        outcome
    }

    pub fn load(&self, id: Uuid) -> io::Result<Option<Conversation>> {
        let file = match File::open(self.record(id)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut data = Vec::new();
        file.take(RECORD_LIMIT + 1).read_to_end(&mut data)?;
        if data.len() as u64 > RECORD_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thread record is too large.",
            ));
        }
        let (version, turn): (u8, Conversation) = serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if !(version == 2 || (version == 1 && turn.follow_ups.is_empty())) || turn.id != id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unrecognized thread record.",
            ));
        }
        validate(&turn).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(turn))
    }

    pub fn history(&self, workspace: &Workspace) -> io::Result<Vec<ThreadSummary>> {
        let records = match fs::read_dir(self.folder()) {
            Ok(records) => records,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut files = Vec::new();
        for entry in records.filter_map(Result::ok) {
            if !entry.file_type().is_ok_and(|k| k.is_file()) {
                continue;
            }
            let filename = entry.file_name();
            let Some(id) = filename
                .to_str()
                .and_then(|value| value.strip_suffix(".json"))
                .and_then(|value| Uuid::parse_str(value).ok())
            else {
                continue;
            };
            let modified = entry.metadata()?.modified()?;
            files.push((modified, id));
        }
        files.sort_unstable_by(|a, b| b.cmp(a));
        let mut list = Vec::new();
        for (_, id) in files {
            match self.load(id) {
                Ok(Some(turn)) if &turn.workspace == workspace => {
                    let first = turn.prompt.trim().lines().next().unwrap_or("New thread");
                    let title = first.chars().take(72).collect::<String>();
                    list.push(ThreadSummary { id, title });
                    if list.len() == 50 {
                        break;
                    }
                }
                // Ignore a file that disappeared concurrently or a damaged record.
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::InvalidData => {}
                Err(e) => return Err(e),
            }
        }
        Ok(list)
    }

    pub fn workspace_directory(&self, turn: &Conversation) -> io::Result<PathBuf> {
        match &turn.workspace {
            Workspace::Scratch => ScratchWorkspaces::new(&self.state)
                .reopen(turn.id)?
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "No scratch folder for this thread.",
                    )
                }),
            Workspace::Local(path) => {
                if path.canonicalize()? != *path || !path.is_dir() {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Project directory has changed.",
                    ))
                } else {
                    Ok(path.clone())
                }
            }
        }
    }

    fn folder(&self) -> PathBuf {
        self.state.join("threads")
    }

    fn record(&self, id: Uuid) -> PathBuf {
        self.folder().join(format!("{id}.json"))
    }
}

fn validate(turn: &Conversation) -> io::Result<()> {
    if turn.id.is_nil()
        || turn.prompt.trim().is_empty()
        || turn.reply.trim().is_empty()
        || turn
            .follow_ups
            .iter()
            .any(|exchange| exchange.prompt.trim().is_empty() || exchange.reply.trim().is_empty())
        || matches!(&turn.workspace, Workspace::Local(path) if !Path::new(path).is_absolute())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Incomplete conversation.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("butler-threads-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scratch_roundtrip_reuses_directory_without_rewriting_a_turn() {
        let tmp = Temp::new();
        let state = tmp.0.join("profile");
        let store = ThreadStore::new(&state);
        let mut turn = Conversation {
            id: Uuid::new_v4(),
            workspace: Workspace::Scratch,
            prompt: "Quoted: «bonjour»\nSecond line".into(),
            reply: "Offline result".into(),
            follow_ups: vec![],
        };
        assert!(store.save(&turn).is_err()); // draft has not claimed a directory
        assert_eq!(store.load(turn.id).unwrap(), None);
        let folder = ScratchWorkspaces::new(&state)
            .prepare(turn.id, "2026-11-24", &turn.prompt)
            .unwrap();
        store.save(&turn).unwrap();
        let reopened = ThreadStore::new(&state);
        assert_eq!(reopened.load(turn.id).unwrap(), Some(turn.clone()));
        assert_eq!(
            reopened.history(&Workspace::Scratch).unwrap(),
            [ThreadSummary {
                id: turn.id,
                title: "Quoted: «bonjour»".into(),
            }]
        );
        assert_eq!(reopened.workspace_directory(&turn).unwrap(), folder);
        reopened.save(&turn).unwrap();
        turn.reply = "A different reply".into();
        assert_eq!(
            reopened.save(&turn).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn migrates_v1_and_serializes_follow_ups_without_losing_prior_messages() {
        let tmp = Temp::new();
        let project = tmp.0.join("project");
        fs::create_dir(&project).unwrap();
        let store = ThreadStore::new(tmp.0.join("profile"));
        fs::create_dir_all(store.folder()).unwrap();
        let id = Uuid::new_v4();
        let workspace = Workspace::Local(project.canonicalize().unwrap());
        let legacy = serde_json::json!([1, {
            "id": id, "workspace": workspace, "prompt": "First", "reply": "Original reply",
        }]);
        fs::write(store.record(id), serde_json::to_vec(&legacy).unwrap()).unwrap();
        let original = store.load(id).unwrap().unwrap();
        assert!(original.follow_ups.is_empty());
        let second = Exchange {
            prompt: "Second".into(),
            reply: "New reply".into(),
        };
        let updated = store.append(&original, second.clone()).unwrap();
        assert_eq!(updated.follow_ups, [second.clone()]);
        assert_eq!(store.append(&original, second).unwrap(), updated);
        let third = Exchange {
            prompt: "Third".into(),
            reply: "Third reply".into(),
        };
        assert_eq!(
            store.append(&original, third.clone()).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(store.load(id).unwrap().unwrap(), updated);
        let latest = store.append(&updated, third).unwrap();
        store.save(&original).unwrap();
        assert_eq!(store.load(id).unwrap(), Some(latest));
        assert!(fs::read_dir(project).unwrap().next().is_none());
    }

    #[test]
    fn named_project_uses_its_existing_folder_read_only() {
        let tmp = Temp::new();
        let root = tmp.0.join("project");
        fs::create_dir(&root).unwrap();
        let state = tmp.0.join("profile");
        let store = ThreadStore::new(&state);
        let turn = Conversation {
            id: Uuid::new_v4(),
            workspace: Workspace::Local(root.canonicalize().unwrap()),
            prompt: "First".into(),
            reply: "Second".into(),
            follow_ups: vec![],
        };
        store.save(&turn).unwrap();
        assert!(store.history(&Workspace::Scratch).unwrap().is_empty());
        assert_eq!(store.history(&turn.workspace).unwrap()[0].id, turn.id);
        assert_eq!(store.load(turn.id).unwrap(), Some(turn));
        assert!(!state.join("scratch").exists());
        assert!(fs::read_dir(root).unwrap().next().is_none());
    }
}
