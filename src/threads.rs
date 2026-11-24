//! Durable first-turn records. The workspace binding is validated on reopen.

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
pub struct FirstTurn {
    pub id: Uuid,
    pub workspace: Workspace,
    pub prompt: String,
    pub reply: String,
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

    /// Accept only once, even across retries after restart. Nothing in a
    /// selected project folder is modified by saving a conversation.
    pub fn save(&self, turn: &FirstTurn) -> io::Result<()> {
        validate(turn)?;
        self.workspace_directory(turn)?;
        let data = serde_json::to_vec(&(1_u8, turn)).map_err(io::Error::other)?;
        if data.len() as u64 > RECORD_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Thread record is too large.",
            ));
        }

        let folder = self.folder();
        fs::create_dir_all(&folder)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(folder.join(format!("{}.lock", turn.id)))?;
        lock.lock()?;
        if let Some(saved) = self.load(turn.id)? {
            return if &saved == turn {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Thread already stores a different turn.",
                ))
            };
        }
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

    pub fn load(&self, id: Uuid) -> io::Result<Option<FirstTurn>> {
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
        let (version, turn): (u8, FirstTurn) = serde_json::from_slice(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if version != 1 || turn.id != id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unrecognized thread record.",
            ));
        }
        validate(&turn).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(turn))
    }

    pub fn workspace_directory(&self, turn: &FirstTurn) -> io::Result<PathBuf> {
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

fn validate(turn: &FirstTurn) -> io::Result<()> {
    if turn.id.is_nil()
        || turn.prompt.trim().is_empty()
        || turn.reply.trim().is_empty()
        || matches!(&turn.workspace, Workspace::Local(path) if !Path::new(path).is_absolute())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Incomplete first turn.",
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
        let mut turn = FirstTurn {
            id: Uuid::new_v4(),
            workspace: Workspace::Scratch,
            prompt: "Quoted: «bonjour»\nSecond line".into(),
            reply: "Offline result".into(),
        };
        assert!(store.save(&turn).is_err()); // draft has not claimed a directory
        assert_eq!(store.load(turn.id).unwrap(), None);
        let folder = ScratchWorkspaces::new(&state)
            .prepare(turn.id, "2026-11-24", &turn.prompt)
            .unwrap();
        store.save(&turn).unwrap();
        let reopened = ThreadStore::new(&state);
        assert_eq!(reopened.load(turn.id).unwrap(), Some(turn.clone()));
        assert_eq!(reopened.workspace_directory(&turn).unwrap(), folder);
        reopened.save(&turn).unwrap();
        turn.reply = "A different reply".into();
        assert_eq!(
            reopened.save(&turn).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn named_project_uses_its_existing_folder_read_only() {
        let tmp = Temp::new();
        let root = tmp.0.join("project");
        fs::create_dir(&root).unwrap();
        let state = tmp.0.join("profile");
        let store = ThreadStore::new(&state);
        let turn = FirstTurn {
            id: Uuid::new_v4(),
            workspace: Workspace::Local(root.canonicalize().unwrap()),
            prompt: "First".into(),
            reply: "Second".into(),
        };
        store.save(&turn).unwrap();
        assert_eq!(store.load(turn.id).unwrap(), Some(turn));
        assert!(!state.join("scratch").exists());
        assert!(fs::read_dir(root).unwrap().next().is_none());
    }
}
