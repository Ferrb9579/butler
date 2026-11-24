//! Saved-thread unsent plain text. Persist only the reference's nonempty prompts,
//! debounce edits by 300ms, and flush the last committed edits on clean shutdown.

use incular::prelude::TextEditingController;
use std::{
    cell::RefCell,
    collections::{BTreeMap, btree_map::Entry},
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
const VERSION: &[u8] = b"butler-unsent-thread-text-v1\n";
const MAX_RECORD_BYTES: usize = MAX_TEXT_BYTES + VERSION.len();
const DEBOUNCE: Duration = Duration::from_millis(300);
const MAX_PENDING_THREADS: usize = 64;
const SHUTDOWN_FLUSH_LIMIT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct Drafts(Rc<Inner>);

struct Inner {
    store: Option<Store>,
    editors: RefCell<BTreeMap<Uuid, TextEditingController>>,
    writer: RefCell<Option<Writer>>,
}

#[derive(Clone)]
pub struct Editor {
    pub controller: TextEditingController,
    thread: Uuid,
    owner: Weak<Inner>,
}

impl Drafts {
    pub fn new(profile: Option<PathBuf>) -> Self {
        Self(Rc::new(Inner {
            store: profile.map(Store),
            editors: RefCell::new(BTreeMap::new()),
            writer: RefCell::new(None),
        }))
    }

    pub fn editor(&self, thread: Uuid) -> Editor {
        let mut editors = self.0.editors.borrow_mut();
        let controller = editors
            .entry(thread)
            .or_insert_with(|| {
                let input = TextEditingController::new();
                if let Some(store) = &self.0.store {
                    match store.load(thread) {
                        Ok(Some(text)) => input.set_text(text),
                        Ok(None) => {}
                        Err(_) => eprintln!("Butler: could not restore a saved-thread draft."),
                    }
                }
                input
            })
            .clone();
        Editor {
            controller,
            thread,
            owner: Rc::downgrade(&self.0),
        }
    }
}

impl Editor {
    pub fn changed(&self, text: &str) {
        // Only committed text edits call this callback. Never serialize a
        // path, provider setting, attachment, or actual conversation record.
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let Some(store) = &owner.store else { return };
        let mut slot = owner.writer.borrow_mut();
        if slot.is_none() {
            match Writer::new(store.clone()) {
                Ok(writer) => *slot = Some(writer),
                Err(_) => {
                    eprintln!("Butler: could not start the saved-thread draft writer.");
                    return;
                }
            }
        }
        if !slot
            .as_ref()
            .is_some_and(|writer| writer.put(self.thread, text))
        {
            eprintln!("Butler: could not queue a saved-thread draft.");
        }
    }
}

#[derive(Clone)]
struct Store(PathBuf);

fn filename(thread: Uuid) -> String {
    format!("unsent-thread-text-{thread}.draft")
}

fn invalid_record() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid saved-thread draft record",
    )
}

impl Store {
    fn load(&self, thread: Uuid) -> io::Result<Option<String>> {
        match Directory::open(&self.0) {
            Ok(profile) => read_record(&profile, &filename(thread)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn save(&self, thread: Uuid, text: &str) -> io::Result<()> {
        if text.len() > MAX_TEXT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "saved-thread draft is too large",
            ));
        }
        let profile = Directory::open(&self.0)?;
        let target = filename(thread);
        let existing = read_record(&profile, &target)?;
        if text.is_empty() {
            if existing.is_some() {
                profile.remove(&target)?;
                profile.sync()?;
            }
            return Ok(());
        }
        if existing.as_deref() == Some(text) {
            return Ok(());
        }
        let temp = format!(".unsent-thread-text-{}", Uuid::new_v4());
        // Creation is exclusive. Never remove someone else's temp on collision.
        let mut file = profile.create(&temp)?;
        let written = (|| {
            file.write_all(VERSION)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            profile.replace(&temp, &target)?;
            profile.sync()
        })();
        if written.is_err() {
            let _ = profile.remove(&temp);
        }
        written
    }
}

fn read_record(profile: &Directory, name: &str) -> io::Result<Option<String>> {
    let Some(file) = profile.read(name)? else {
        return Ok(None);
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES as u64 {
        return Err(invalid_record());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let payload = bytes.strip_prefix(VERSION).ok_or_else(invalid_record)?;
    if payload.is_empty() || payload.len() > MAX_TEXT_BYTES {
        return Err(invalid_record());
    }
    Ok(Some(
        std::str::from_utf8(payload)
            .map_err(|_| invalid_record())?
            .to_owned(),
    ))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod filesystem {
    use super::*;
    use rustix::{
        fs::{self, AtFlags, FileType, Mode, OFlags},
        io::Errno,
    };

    pub(super) struct Directory(File);

    impl Directory {
        pub fn open(path: &Path) -> io::Result<Self> {
            Ok(Self(
                fs::open(
                    path,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )?
                .into(),
            ))
        }

        pub fn read(&self, name: &str) -> io::Result<Option<File>> {
            // Refuse non-files before opening. No-follow/nonblocking also
            // covers a link or pipe substituted before the descriptor opens.
            match fs::statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {}
                Ok(_) => return Err(invalid_record()),
                Err(Errno::NOENT) => return Ok(None),
                Err(error) => return Err(error.into()),
            }
            match fs::openat(
                &self.0,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(file) => Ok(Some(file.into())),
                Err(Errno::NOENT) => Ok(None),
                Err(error) => Err(error.into()),
            }
        }

        pub fn create(&self, name: &str) -> io::Result<File> {
            Ok(fs::openat(
                &self.0,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )?
            .into())
        }

        pub fn replace(&self, from: &str, to: &str) -> io::Result<()> {
            Ok(fs::renameat(&self.0, from, &self.0, to)?)
        }

        pub fn remove(&self, name: &str) -> io::Result<()> {
            match fs::unlinkat(&self.0, name, AtFlags::empty()) {
                Ok(()) | Err(Errno::NOENT) => Ok(()),
                Err(error) => Err(error.into()),
            }
        }

        pub fn sync(&self) -> io::Result<()> {
            self.0.sync_all()
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod filesystem {
    use super::*;
    use std::fs::{self, OpenOptions};
    #[cfg(windows)]
    use std::os::windows::fs::OpenOptionsExt;

    pub(super) struct Directory(PathBuf);

    impl Directory {
        pub fn open(path: &Path) -> io::Result<Self> {
            if !fs::symlink_metadata(path)?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "invalid profile directory",
                ));
            }
            Ok(Self(path.to_owned()))
        }

        pub fn read(&self, name: &str) -> io::Result<Option<File>> {
            let path = self.0.join(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    let mut options = OpenOptions::new();
                    options.read(true);
                    // FILE_FLAG_OPEN_REPARSE_POINT: do not follow a leaf link
                    // if it is substituted after the metadata check.
                    #[cfg(windows)]
                    options.custom_flags(0x0020_0000);
                    Ok(Some(options.open(path)?))
                }
                Ok(_) => Err(invalid_record()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            }
        }

        pub fn create(&self, name: &str) -> io::Result<File> {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.0.join(name))
        }

        pub fn replace(&self, from: &str, to: &str) -> io::Result<()> {
            fs::rename(self.0.join(from), self.0.join(to))
        }

        pub fn remove(&self, name: &str) -> io::Result<()> {
            match fs::remove_file(self.0.join(name)) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }

        pub fn sync(&self) -> io::Result<()> {
            Ok(())
        }
    }
}

use filesystem::Directory;

#[derive(Default)]
struct Pending {
    entries: BTreeMap<Uuid, String>,
    due: Option<Instant>,
    stopping: bool,
}

type Queue = Arc<(Mutex<Pending>, Condvar)>;

struct Writer {
    queue: Queue,
    job: Option<JoinHandle<()>>,
    finished: mpsc::Receiver<()>,
}

impl Writer {
    fn new(store: Store) -> io::Result<Self> {
        let queue = Queue::default();
        let worker = queue.clone();
        let (finish, finished) = mpsc::channel();
        let job = thread::Builder::new()
            .name("butler-drafts".into())
            .spawn(move || {
                drain(store, worker);
                let _ = finish.send(());
            })?;
        Ok(Self {
            queue,
            job: Some(job),
            finished,
        })
    }

    fn put(&self, thread: Uuid, text: &str) -> bool {
        if text.len() > MAX_TEXT_BYTES {
            return false;
        }
        let Ok(mut pending) = self.queue.0.lock() else {
            return false;
        };
        if pending.entries.len() >= MAX_PENDING_THREADS && !pending.entries.contains_key(&thread) {
            return false;
        }
        let changed = match pending.entries.entry(thread) {
            Entry::Occupied(mut previous) if previous.get() != text => {
                previous.insert(text.to_owned());
                true
            }
            Entry::Vacant(slot) => {
                slot.insert(text.to_owned());
                true
            }
            _ => false,
        };
        if changed {
            pending.due = Some(Instant::now() + DEBOUNCE);
            self.queue.1.notify_one();
        }
        true
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.queue.0.lock() {
            pending.stopping = true;
            self.queue.1.notify_one();
        }
        if let Some(job) = self.job.take() {
            // A normal quit drains the coalesced queue. A blocked filesystem
            // must not hold the whole application's exit indefinitely.
            match self.finished.recv_timeout(SHUTDOWN_FLUSH_LIMIT) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = job.join();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!("Butler: saved-thread draft flush did not finish before shutdown.");
                }
            }
        }
    }
}

fn drain(store: Store, queue: Queue) {
    loop {
        let Ok(mut pending) = queue.0.lock() else {
            return;
        };
        loop {
            if pending.stopping || pending.entries.len() >= MAX_PENDING_THREADS {
                break;
            }
            if let Some(due) = pending.due {
                let delay = due.saturating_duration_since(Instant::now());
                if delay.is_zero() {
                    break;
                }
                let Ok((next, _)) = queue.1.wait_timeout(pending, delay) else {
                    return;
                };
                pending = next;
            } else {
                let Ok(next) = queue.1.wait(pending) else {
                    return;
                };
                pending = next;
            }
        }
        let stopping = pending.stopping;
        pending.due = None;
        let batch = std::mem::take(&mut pending.entries);
        drop(pending); // no filesystem access under the edit queue lock
        for (thread, text) in batch {
            if store.save(thread, &text).is_err() {
                eprintln!("Butler: could not persist a saved-thread draft.");
            }
        }
        if stopping {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("butler-unsent-text-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn store(&self) -> Store {
            Store(self.0.clone())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn opening_an_unedited_saved_thread_stays_read_only() {
        let profile = Fixture::new();
        let drafts = Drafts::new(Some(profile.0.clone()));
        assert_eq!(drafts.editor(Uuid::new_v4()).controller.text(), "");
        assert!(drafts.0.writer.borrow().is_none());
        drop(drafts);
        assert_eq!(fs::read_dir(&profile.0).unwrap().count(), 0);
    }

    #[test]
    fn latest_edit_flushes_per_thread_on_drop_and_only_empty_clears() {
        let profile = Fixture::new();
        let other = Fixture::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let drafts = Drafts::new(Some(profile.0.clone()));
        let first = drafts.editor(a);
        first.changed("earlier");
        first.changed("Café 🌿\nlast text");
        drafts.editor(b).changed(" \n ");
        drop(drafts); // even mounted editor callbacks do not keep the writer alive
        assert_eq!(
            profile.store().load(a).unwrap().as_deref(),
            Some("Café 🌿\nlast text")
        );
        assert_eq!(profile.store().load(b).unwrap().as_deref(), Some(" \n "));
        assert_eq!(other.store().load(a).unwrap(), None);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(profile.0.join(filename(a)))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "draft is never group/world-readable");
        }

        let restarted = Drafts::new(Some(profile.0.clone()));
        assert_eq!(restarted.editor(a).controller.text(), "Café 🌿\nlast text");
        assert_eq!(restarted.editor(b).controller.text(), " \n ");
        restarted.editor(a).changed("");
        drop(restarted);
        assert_eq!(profile.store().load(a).unwrap(), None);
        assert_eq!(profile.store().load(b).unwrap().as_deref(), Some(" \n "));
        assert_eq!(fs::read_dir(&profile.0).unwrap().count(), 1);
    }

    #[test]
    fn invalid_missing_or_oversized_records_cannot_erase_prior_bytes() {
        let profile = Fixture::new();
        let thread = Uuid::new_v4();
        let path = profile.0.join(filename(thread));
        let store = profile.store();
        fs::write(&path, [VERSION, &[0xff_u8]].concat()).unwrap();
        assert!(store.load(thread).is_err());
        assert!(store.save(thread, "").is_err());
        assert!(store.save(thread, "later").is_err());
        assert_eq!(fs::read(&path).unwrap(), [VERSION, &[0xff_u8]].concat());
        fs::remove_file(&path).unwrap();
        store.save(thread, "existing").unwrap();
        assert!(
            store
                .save(thread, &"é".repeat(MAX_TEXT_BYTES / 2 + 1))
                .is_err()
        );
        assert_eq!(store.load(thread).unwrap().as_deref(), Some("existing"));

        let missing = profile.0.join("absent");
        let missing_store = Store(missing.clone());
        assert_eq!(missing_store.load(thread).unwrap(), None);
        assert!(missing_store.save(thread, "edits").is_err());
        assert!(!missing.exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn symlinked_profile_and_record_are_not_followed() {
        let profile = Fixture::new();
        let other = Fixture::new();
        let thread = Uuid::new_v4();
        let external = other.0.join(filename(thread));
        other.store().save(thread, "external").unwrap();
        let bytes = fs::read(&external).unwrap();
        let marker = profile.0.join(filename(thread));
        std::os::unix::fs::symlink(&external, &marker).unwrap();
        for value in ["new", ""] {
            assert!(profile.store().save(thread, value).is_err());
        }
        assert!(profile.store().load(thread).is_err());
        assert!(fs::symlink_metadata(marker).unwrap().is_symlink());

        let linked_profile = profile.0.join("linked-profile");
        std::os::unix::fs::symlink(&other.0, &linked_profile).unwrap();
        let linked_store = Store(linked_profile);
        assert!(linked_store.load(thread).is_err());
        assert!(linked_store.save(thread, "new").is_err());
        assert_eq!(fs::read(external).unwrap(), bytes);
    }
}
