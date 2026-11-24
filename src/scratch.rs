//! Managed directories for submitted local scratch threads.
//! Naming follows T3 Code v0.1.11, apps/server/src/project/ManagedProjectFolders.ts.

use icu_normalizer::DecomposingNormalizer;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ScratchWorkspaces {
    profile: PathBuf,
}

impl ScratchWorkspaces {
    pub fn new(profile: impl Into<PathBuf>) -> Self {
        Self {
            profile: profile.into(),
        }
    }

    /// Resolve a submitted thread once. Retrying the same thread keeps its original
    /// directory, even if the current time or title differs. Drafts do not call this.
    pub fn prepare(&self, id: Uuid, utc_date: &str, first_prompt: &str) -> io::Result<PathBuf> {
        let first_prompt = first_prompt.trim();
        if id.is_nil() || first_prompt.is_empty() || !valid_date(utc_date) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid scratch launch.",
            ));
        }
        let profile = self.ensure_profile()?;
        let root = managed_dir(&profile, "scratch")?;
        let bindings = managed_dir(&profile, "scratch-bindings")?;
        let key = id.simple().to_string();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(bindings.join(format!("{key}.lock")))?;
        lock.lock()?;
        let binding = bindings.join(&key);
        if let Some(path) = read_binding(&binding, &root)? {
            return Ok(path);
        }

        let slug = folder_words(first_prompt);
        let stem = if slug.is_empty() {
            utc_date.to_owned()
        } else {
            format!("{utc_date}-{slug}")
        };
        let full = format!("{stem}-{key}");
        let mut candidates = [format!("{stem}-{}", &key[24..]), full.clone()].into_iter();
        let (leaf, directory) = loop {
            let leaf = candidates
                .next()
                .unwrap_or_else(|| format!("{full}-{}", &Uuid::new_v4().simple().to_string()[..8]));
            let path = root.join(&leaf);
            match fs::create_dir(&path) {
                Ok(()) => break (leaf, path),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        };

        let temp = bindings.join(format!(".{key}-{}", Uuid::new_v4().simple()));
        let record = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            writeln!(file, "{leaf}")?;
            file.sync_all()?;
            drop(file);
            sync_directory(&root)?;
            fs::rename(&temp, &binding)
        })();
        if let Err(error) = record {
            let _ = fs::remove_file(&temp);
            let _ = fs::remove_dir(&directory);
            return Err(error);
        }
        sync_directory(&bindings)?;
        Ok(directory)
    }

    pub fn reopen(&self, id: Uuid) -> io::Result<Option<PathBuf>> {
        let profile = match self.profile.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        read_binding(
            &profile
                .join("scratch-bindings")
                .join(id.simple().to_string()),
            &profile.join("scratch"),
        )
    }

    fn ensure_profile(&self) -> io::Result<PathBuf> {
        if !self.profile.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Scratch profile must be an absolute path.",
            ));
        }
        // Check before creating directories, including where an existing symlink points.
        let existing = self
            .profile
            .ancestors()
            .find(|p| p.exists())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Scratch profile has no existing parent.",
                )
            })?;
        reject_checkout(&existing.canonicalize()?)?;
        fs::create_dir_all(&self.profile)?;
        let profile = self.profile.canonicalize()?;
        reject_checkout(&profile)?;
        Ok(profile)
    }
}

fn reject_checkout(path: &Path) -> io::Result<()> {
    for directory in path.ancestors() {
        let marker = directory.join(".git");
        let metadata = match fs::metadata(&marker) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        let git_dir = if metadata.is_dir() {
            marker
        } else if metadata.is_file() && metadata.len() <= 16 * 1024 {
            let Ok(pointer) = fs::read_to_string(marker) else {
                continue;
            };
            let Some(target) = pointer.trim_end().strip_prefix("gitdir: ") else {
                continue;
            };
            directory.join(target)
        } else {
            continue;
        };
        // An empty .git directory (including a sandbox placeholder) isn't a checkout.
        if git_dir.join("HEAD").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Scratch data must be outside Git checkouts.",
            ));
        }
    }
    Ok(())
}

fn managed_dir(profile: &Path, name: &str) -> io::Result<PathBuf> {
    let path = profile.join(name);
    match fs::create_dir(&path) {
        Ok(()) => sync_directory(profile)?,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    if fs::symlink_metadata(&path)?.file_type().is_dir() {
        reject_checkout(&path)?;
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Managed scratch directory is not a real directory.",
        ))
    }
}

fn read_binding(binding: &Path, root: &Path) -> io::Result<Option<PathBuf>> {
    let raw = match fs::read_to_string(binding) {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let name = raw.strip_suffix('\n').unwrap_or(&raw);
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid scratch folder binding.",
        ));
    }
    let path = root.join(name);
    if !fs::symlink_metadata(root)?.file_type().is_dir()
        || !fs::symlink_metadata(&path)?.file_type().is_dir()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Scratch folder is not a real directory.",
        ));
    }
    reject_checkout(&path)?;
    Ok(Some(path))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> io::Result<()> {
    Ok(())
}

fn valid_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, b)| !matches!(i, 4 | 7) && !b.is_ascii_digit())
    {
        return false;
    }
    let year: u16 = date[..4].parse().unwrap_or_default();
    let month: u8 = date[5..7].parse().unwrap_or_default();
    let day: u8 = date[8..].parse().unwrap_or_default();
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 0,
    };
    year > 0 && (1..=days).contains(&day)
}

fn folder_words(text: &str) -> String {
    let mut latin = String::with_capacity(text.len());
    for c in text.chars() {
        latin.push_str(match c {
            'Ŀ' | 'ŀ' | 'Ł' | 'ł' => "l",
            'ŉ' | 'Ŋ' | 'ŋ' => "n",
            'Đ' | 'đ' | 'Ð' | 'ð' => "d",
            'ß' | 'ẞ' => "ss",
            'Ø' | 'ø' => "o",
            'Æ' | 'æ' => "ae",
            'Œ' | 'œ' => "oe",
            'Þ' | 'þ' => "th",
            'Ħ' | 'ħ' => "h",
            'ı' => "i",
            'ĸ' => "q",
            'Ŧ' | 'ŧ' => "t",
            _ => {
                latin.push(c);
                continue;
            }
        });
    }
    let normalized: String = DecomposingNormalizer::new_nfkd()
        .normalize_iter(latin.chars())
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .flat_map(char::to_lowercase)
        .collect();
    let slug = normalized
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join("-");
    slug[..slug.len().min(48)].trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("butler-scratch-{}", Uuid::new_v4().simple()));
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
    fn duplicate_prompts_are_isolated_and_retries_reuse_original_folder() {
        let t = Temp::new();
        let store = ScratchWorkspaces::new(t.0.join("profile"));
        let a = Uuid::parse_str("10000000-0000-4000-8000-0000cafef00d").unwrap();
        let b = Uuid::parse_str("20000000-0000-4000-8000-0000cafef00d").unwrap();
        assert_eq!(store.reopen(a).unwrap(), None);
        assert!(!t.0.join("profile").exists());
        let first = store
            .prepare(a, "2026-11-24", "Déjà vu — Straße Øresund 👋 go six")
            .unwrap();
        let second = store
            .prepare(b, "2026-11-24", "Déjà vu — Straße Øresund 👋 go six")
            .unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "2026-11-24-deja-vu-strasse-oresund-go-cafef00d"
        );
        assert_ne!(first, second);
        assert!(
            second
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(&b.simple().to_string())
        );
        assert_eq!(store.prepare(a, "2026-12-01", "changed").unwrap(), first);
        assert_eq!(
            ScratchWorkspaces::new(t.0.join("profile"))
                .reopen(b)
                .unwrap(),
            Some(second)
        );
    }

    #[test]
    fn refutes_invalid_dates_and_git_checkout_paths_before_creation() {
        let t = Temp::new();
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        let checkout = t.0.join("project");
        fs::create_dir(&checkout).unwrap();
        fs::create_dir(checkout.join("other")).unwrap();
        fs::write(checkout.join("other/HEAD"), "ref: refs/heads/main").unwrap();
        fs::write(checkout.join(".git"), "gitdir: other").unwrap();
        let profile = checkout.join("profile");
        let store = ScratchWorkspaces::new(&profile);
        assert!(store.prepare(Uuid::new_v4(), "2026-11-24", "fix").is_err());
        assert!(!profile.exists());
    }

    #[test]
    fn concurrent_retries_share_one_binding() {
        let t = Temp::new();
        let store = ScratchWorkspaces::new(t.0.join("profile"));
        let id = Uuid::new_v4();
        std::thread::scope(|scope| {
            let a = scope.spawn(|| store.prepare(id, "2026-11-24", "first").unwrap());
            let b = scope.spawn(|| store.prepare(id, "2026-11-24", "second").unwrap());
            assert_eq!(a.join().unwrap(), b.join().unwrap());
        });
        assert_eq!(
            fs::read_dir(t.0.join("profile/scratch")).unwrap().count(),
            1
        );
    }

    #[test]
    fn a_removed_workspace_is_an_error_not_a_new_directory() {
        let t = Temp::new();
        let store = ScratchWorkspaces::new(t.0.join("profile"));
        let id = Uuid::new_v4();
        let first = store.prepare(id, "2026-11-24", "same").unwrap();
        fs::remove_dir(first).unwrap();
        assert!(store.reopen(id).is_err());
        assert!(store.prepare(id, "2026-11-24", "same").is_err());
    }
}
