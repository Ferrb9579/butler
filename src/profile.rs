//! Profile-local, durable first-run completion. Missing data is not completion.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const COMPLETED_V1: &[u8] = b"butler-onboarding-complete-v1\n";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct Profile {
    directory: PathBuf,
}

impl Profile {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    fn marker(&self) -> PathBuf {
        self.directory.join("onboarding")
    }

    pub fn provider_name(&self, provider: Provider) -> io::Result<String> {
        let file = match File::open(self.directory.join(provider.filename())) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(provider.label().into()),
            Err(e) => return Err(e),
        };
        let mut name = String::new();
        file.take(257).read_to_string(&mut name)?;
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid stored provider name.",
            ));
        }
        Ok(name)
    }

    pub fn save_provider_name(&self, provider: Provider, name: &str) -> io::Result<String> {
        let name = name.trim();
        if name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use at most 64 characters on one line.",
            ));
        }
        let name = if name.is_empty() {
            provider.label()
        } else {
            name
        };
        fs::create_dir_all(&self.directory)?;
        let (path, mut file) = self.create_pending_file("provider")?;
        let result = (|| {
            file.write_all(name.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&path, self.directory.join(provider.filename()))?;
            sync_directory(&self.directory)
        })();
        let _ = fs::remove_file(path);
        result.map(|()| name.into())
    }

    pub fn onboarding_completed(&self) -> io::Result<bool> {
        let file = match File::open(self.marker()) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let mut data = Vec::new();
        file.take(COMPLETED_V1.len() as u64 + 1)
            .read_to_end(&mut data)?;
        if data == COMPLETED_V1 {
            Ok(true)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported or corrupt onboarding marker",
            ))
        }
    }

    pub fn complete_onboarding(&self) -> io::Result<()> {
        if self.onboarding_completed()? {
            return Ok(());
        }
        fs::create_dir_all(&self.directory)?;
        let (path, mut file) = self.create_pending_file("onboarding")?;
        let result = (|| {
            file.write_all(COMPLETED_V1)?;
            file.sync_all()?;
            drop(file);
            // A concurrent application window may already have completed setup.
            if let Err(error) = fs::rename(&path, self.marker())
                && !self.onboarding_completed().unwrap_or(false)
            {
                return Err(error);
            }
            sync_directory(&self.directory)
        })();
        let _ = fs::remove_file(path);
        result
    }

    fn create_pending_file(&self, kind: &str) -> io::Result<(PathBuf, File)> {
        loop {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = self
                .directory
                .join(format!(".{kind}-{}-{sequence}", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Provider {
    Codex,
    Claude,
    Cursor,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Cursor];

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Codex => "provider-name-codex",
            Self::Claude => "provider-name-claude",
            Self::Cursor => "provider-name-cursor",
        }
    }
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> io::Result<()> {
    File::open(directory)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn create() -> Self {
            loop {
                let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "butler-profile-test-{}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create test profile: {error}"),
                }
            }
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn setup_survives_restart_and_is_isolated_per_profile() {
        let root = TestRoot::create();
        let profile = Profile::new(root.0.join("one"));
        assert!(!profile.onboarding_completed().unwrap());
        profile.complete_onboarding().unwrap();
        profile.complete_onboarding().unwrap();
        assert!(
            Profile::new(root.0.join("one"))
                .onboarding_completed()
                .unwrap()
        );
        assert!(
            !Profile::new(root.0.join("two"))
                .onboarding_completed()
                .unwrap()
        );
    }

    #[test]
    fn interrupted_write_does_not_count_as_completion() {
        let root = TestRoot::create();
        fs::write(root.0.join(".onboarding-interrupted"), b"partial").unwrap();
        let profile = Profile::new(&root.0);
        assert!(!profile.onboarding_completed().unwrap());
        profile.complete_onboarding().unwrap();
        assert!(profile.onboarding_completed().unwrap());
    }

    #[test]
    fn corrupted_marker_surfaces_as_an_error_and_is_not_overwritten() {
        let root = TestRoot::create();
        let profile = Profile::new(&root.0);
        fs::write(profile.marker(), b"unknown").unwrap();
        assert_eq!(
            profile.onboarding_completed().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(profile.complete_onboarding().is_err());
        assert_eq!(fs::read(profile.marker()).unwrap(), b"unknown");
    }
}
