//! Registry for opening existing local folders without modifying them.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const TITLE_RECORD: &str = "title\t";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalProject {
    pub root: PathBuf,
    managed_title: Option<String>,
}

impl LocalProject {
    pub fn title(&self) -> String {
        if let Some(title) = &self.managed_title {
            return title.clone();
        }
        self.root
            .file_name()
            .unwrap_or(self.root.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Clone, Debug)]
pub struct LocalProjects {
    state: PathBuf,
}

impl LocalProjects {
    pub fn new(state: impl Into<PathBuf>) -> Self {
        Self {
            state: state.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.state
    }

    /// Last opened project is first. Keep missing entries so removing a drive
    /// temporarily does not silently delete saved projects.
    pub fn list(&self) -> io::Result<Vec<LocalProject>> {
        let raw = match fs::read_to_string(self.state.join("local-projects")) {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut lines = raw.lines().peekable();
        let mut projects = Vec::new();
        while let Some(entry) = lines.next() {
            let root = PathBuf::from(entry);
            if !root.is_absolute() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid saved project path.",
                ));
            }
            // Original records are still one absolute path per line. A
            // named record has an optional following line that cannot be
            // mistaken for an absolute path; JSON preserves the display text.
            let managed_title = lines
                .peek()
                .and_then(|line| line.strip_prefix(TITLE_RECORD))
                .map(serde_json::from_str::<String>)
                .transpose()
                .map_err(io::Error::other)?;
            if managed_title.is_some() {
                lines.next();
            }
            projects.push(LocalProject {
                root,
                managed_title,
            });
        }
        Ok(projects)
    }

    pub fn open(&self, input: &Path) -> io::Result<LocalProject> {
        self.register(input, None)
    }

    pub fn open_named(&self, input: &Path, title: &str) -> io::Result<LocalProject> {
        let title = title.trim();
        if title.is_empty() || title.encode_utf16().count() > 200 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid project name.",
            ));
        }
        self.register(input, Some(title.to_owned()))
    }

    fn register(&self, input: &Path, title: Option<String>) -> io::Result<LocalProject> {
        if !input.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Enter an absolute folder path.",
            ));
        }
        let canonical = input.canonicalize()?;
        // Windows canonicalization emits verbatim paths; keep those intact on disk.
        if !fs::metadata(&canonical)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Select a folder, not a file.",
            ));
        }
        if canonical
            .to_str()
            .is_none_or(|root| root.contains(['\r', '\n']))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported characters in folder path.",
            ));
        }
        fs::create_dir_all(&self.state)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.state.join("local-projects.lock"))?;
        lock.lock()?;
        let mut projects = self.list()?;
        let managed_title = title.or_else(|| {
            projects
                .iter()
                .find(|item| item.root == canonical)
                .and_then(|item| item.managed_title.clone())
        });
        let project = LocalProject {
            root: canonical,
            managed_title,
        };
        projects.retain(|item| item.root != project.root);
        projects.insert(0, project.clone());
        self.save(&projects)?;
        Ok(project)
    }

    fn save(&self, projects: &[LocalProject]) -> io::Result<()> {
        let (temporary, mut file) = loop {
            let counter = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = self
                .state
                .join(format!(".local-projects-{}-{counter}", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => break (path, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        };
        let result = (|| {
            for project in projects {
                let path = project.root.to_str().expect("validated on read or import");
                writeln!(file, "{path}")?;
                if let Some(title) = &project.managed_title {
                    writeln!(file, "{TITLE_RECORD}{}", serde_json::to_string(title)?)?;
                }
            }
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, self.state.join("local-projects"))?;
            #[cfg(unix)]
            File::open(&self.state)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let folder = loop {
                let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let p = std::env::temp_dir()
                    .join(format!("butler-open-local-{}-{id}", std::process::id()));
                match fs::create_dir(&p) {
                    Ok(()) => break p,
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("{e}"),
                }
            };
            Self(folder)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn retains_named_title_when_a_saved_project_is_reopened_by_path() {
        let tmp = Temp::new();
        let slug = tmp.0.join("local-demo");
        let legacy = tmp.0.join("Legacy Project");
        let state = tmp.0.join("state");
        fs::create_dir(&slug).unwrap();
        fs::create_dir(&legacy).unwrap();
        fs::create_dir(&state).unwrap();
        let legacy = legacy.canonicalize().unwrap();
        fs::write(
            state.join("local-projects"),
            format!("{}\n", legacy.display()),
        )
        .unwrap();
        let store = LocalProjects::new(&state);
        let named = store.open_named(&slug, "  Local Démo  ").unwrap();
        assert_eq!(named.title(), "Local Démo");
        let list = LocalProjects::new(&state).list().unwrap();
        assert_eq!(list[0], named);
        assert_eq!(list[1].title(), "Legacy Project");
        assert_eq!(store.open(&slug).unwrap(), named);
        assert_eq!(store.list().unwrap(), list);
    }

    #[test]
    fn imports_without_touching_the_selected_folder_and_reopens_most_recent() {
        let tmp = Temp::new();
        let first = tmp.0.join("one");
        let second = tmp.0.join("contains spaces");
        let state = tmp.0.join("state");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let store = LocalProjects::new(&state);
        assert!(store.list().unwrap().is_empty());
        let a = store.open(&first).unwrap();
        let b = store.open(&second).unwrap();
        assert_eq!(b.title(), "contains spaces");
        assert_eq!(b.root, second.canonicalize().unwrap());
        assert_eq!(
            LocalProjects::new(&state).list().unwrap(),
            [b.clone(), a.clone()]
        );
        assert_eq!(store.open(&first.join(".")).unwrap(), a);
        assert_eq!(LocalProjects::new(&state).list().unwrap(), [a, b]);
        assert!(fs::read_dir(&second).unwrap().next().is_none());
        assert!(fs::read_dir(&first).unwrap().next().is_none());
    }

    #[test]
    fn invalid_input_does_not_replace_a_saved_project() {
        let tmp = Temp::new();
        let state = tmp.0.join("state");
        let good = tmp.0.join("good");
        let file = tmp.0.join("not a folder");
        fs::create_dir(&good).unwrap();
        fs::write(&file, "keep").unwrap();
        let store = LocalProjects::new(state);
        let project = store.open(&good).unwrap();
        assert!(store.open(Path::new("relative")).is_err());
        assert!(store.open(&tmp.0.join("missing")).is_err());
        assert!(store.open(&file).is_err());
        assert_eq!(store.list().unwrap(), [project]);
        assert_eq!(fs::read_to_string(file).unwrap(), "keep");
    }

    #[test]
    fn concurrent_opens_keep_both_projects() {
        let tmp = Temp::new();
        let a = tmp.0.join("a");
        let b = tmp.0.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let store = LocalProjects::new(tmp.0.join("state"));
        std::thread::scope(|scope| {
            let left = scope.spawn(|| store.open(&a).unwrap());
            let right = scope.spawn(|| store.open(&b).unwrap());
            left.join().unwrap();
            right.join().unwrap();
        });
        assert_eq!(store.list().unwrap().len(), 2);
    }
}

/// Visible children for the local-folder palette. This only reads directory
/// entries, never project files or Git metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderListing {
    pub directory: PathBuf,
    pub children: Vec<(String, PathBuf)>,
}

impl FolderListing {
    pub fn read(path: &Path) -> io::Result<Self> {
        let directory = path.canonicalize()?;
        let mut children = fs::read_dir(&directory)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if name.starts_with('.') {
                    return None;
                }
                let path = entry.path();
                path.is_dir().then_some((name, path))
            })
            .collect::<Vec<_>>();
        children.sort_by_cached_key(|(name, _)| (name.to_lowercase(), name.clone()));
        Ok(Self {
            directory,
            children,
        })
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;

    #[test]
    fn lists_visible_directories_in_case_insensitive_order() {
        let path =
            std::env::temp_dir().join(format!("butler-folder-list-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        let result = (|| {
            for name in ["Zoo", ".hidden", "alpha"] {
                fs::create_dir(path.join(name))?;
            }
            fs::write(path.join("notes"), "not listed")?;
            FolderListing::read(&path)
        })();
        fs::remove_dir_all(path).unwrap();
        let listing = result.unwrap();
        assert_eq!(
            listing
                .children
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "Zoo"]
        );
    }
}
