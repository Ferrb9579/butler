//! Local destination transaction only. This does not run Git or register a project.
//!
//! A caller presents an exact destination, then calls `commit` only after the
//! user confirms that destination. The callback must be trusted and must stop
//! all writes before it returns. This is not a sandbox against another process
//! with the same permissions as Butler.

use std::{
    ffi::OsStr,
    fs::File,
    io,
    path::{Path, PathBuf},
};

/// An existing parent and an exact, unoccupied child name. No disk reservation
/// is made by construction; a competing destination always wins.
pub struct CloneDestination {
    path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    binding: local::Binding,
}

/// The private, empty workspace offered only to the confirmed operation.
pub struct CloneWorkspace {
    path: PathBuf,
    directory: File,
}

impl CloneWorkspace {
    /// A display path. Use the open directory rather than resolving this path
    /// again to set a subprocess working directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// An open handle to the checkout directory, not its staging container.
    pub fn directory(&self) -> &File {
        &self.directory
    }
}

impl CloneDestination {
    /// Binds a canonical (not symlinked), user-controlled existing directory
    /// and an exact child name, outside the existing active Butler profile.
    /// This method performs no writes. The profile path may be a system alias
    /// but must resolve to a directory.
    pub fn confirmable(parent: &Path, name: &OsStr, active_profile: &Path) -> io::Result<Self> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let binding = local::Binding::new(parent, name, active_profile)?;
            Ok(Self {
                path: parent.join(name),
                binding,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (parent, name, active_profile);
            Err(unsupported())
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Run only after the user has confirmed `path()`. Promotes a checked-out
    /// directory with an atomic no-replace operation; never selects a suffix.
    /// An error after promotion can mean the parent moved, in which case the
    /// completed checkout is left in place rather than attempting a rollback.
    pub fn commit(
        self,
        clone: impl FnOnce(&CloneWorkspace) -> io::Result<()>,
    ) -> io::Result<PathBuf> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.binding.commit(clone)?;
            Ok(self.path)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = clone;
            Err(unsupported())
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Safe clone destination promotion is unavailable on this platform.",
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod local {
    use super::*;
    use rustix::{
        fs::{self, AtFlags, CWD, Dir, FileType, Mode, OFlags, RenameFlags, Stat},
        io::Errno,
        process::geteuid,
    };
    use std::{ffi::OsString, path::Component};

    const CHECKOUT: &str = "checkout";
    const STAGE_PREFIX: &str = ".butler-clone-";
    const MAX_CLEANUP_DEPTH: usize = 256;

    pub(super) struct Binding {
        parent_path: PathBuf,
        parent: File,
        profile_input: PathBuf,
        profile_path: PathBuf,
        profile: File,
        name: OsString,
    }

    impl Binding {
        pub(super) fn new(
            parent_path: &Path,
            name: &OsStr,
            profile_input: &Path,
        ) -> io::Result<Self> {
            if Path::new(name).file_name() != Some(name)
                || name == ".git"
                || name.as_encoded_bytes().starts_with(STAGE_PREFIX.as_bytes())
            {
                return Err(invalid("Choose a single, ordinary clone folder name."));
            }
            if !parent_path.is_absolute() || parent_path.canonicalize()? != parent_path {
                return Err(invalid(
                    "Choose the existing clone parent by its real path.",
                ));
            }
            if !profile_input.is_absolute() {
                return Err(invalid("The Butler profile must be an absolute path."));
            }
            let profile_path = profile_input.canonicalize()?;
            let profile = walk(&profile_path, None)?;
            let parent = walk(parent_path, Some(&fs::fstat(&profile)?))?;
            verify_parent_permissions(&parent)?;
            vacant(&parent, name)?;
            Ok(Self {
                parent_path: parent_path.to_owned(),
                parent,
                profile_input: profile_input.to_owned(),
                profile_path,
                profile,
                name: name.to_owned(),
            })
        }

        fn verify(&self) -> io::Result<()> {
            let profile = fs::fstat(&self.profile)?;
            if self.profile_input.canonicalize()? != self.profile_path
                || !same(&profile, &fs::fstat(walk(&self.profile_path, None)?)?)
            {
                return Err(moved("The active Butler profile moved."));
            }
            let parent = walk(&self.parent_path, Some(&profile))?;
            if !same(&fs::fstat(&self.parent)?, &fs::fstat(&parent)?) {
                return Err(moved("The confirmed clone parent moved or was replaced."));
            }
            verify_parent_permissions(&parent)
        }

        pub(super) fn commit(
            &self,
            clone: impl FnOnce(&CloneWorkspace) -> io::Result<()>,
        ) -> io::Result<()> {
            self.verify()?;
            vacant(&self.parent, &self.name)?;
            let mut stage = Stage::create(self)?;
            stage.verify()?;
            vacant(&self.parent, &self.name)?;
            clone(
                stage
                    .workspace
                    .as_ref()
                    .expect("staging opened the workspace"),
            )?;
            stage.verify()?;
            let workspace = stage
                .workspace
                .as_ref()
                .expect("staging opened the workspace");
            match entry(&workspace.directory, ".git") {
                Ok(marker) if is_dir(&marker) => {}
                Ok(_) => return Err(invalid("The clone did not produce a real .git directory.")),
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    return Err(invalid("The clone did not produce a real .git directory."));
                }
                Err(err) => return Err(err),
            }
            // Do not exchange/overwrite even an empty destination created
            // after confirmation. Unsupported target filesystems fail closed.
            fs::renameat_with(
                &stage.container,
                CHECKOUT,
                &self.parent,
                &self.name,
                RenameFlags::NOREPLACE,
            )?;
            stage.promoted = true;

            // If the path moved during promotion, don't register it and don't
            // try to delete/overwrite whatever now sits at the display path.
            self.verify()?;
            if !owns(&self.parent, &self.name, &workspace.directory)? {
                return Err(moved("The promoted clone directory moved or was replaced."));
            }
            Ok(())
        }
    }

    struct Stage<'a> {
        binding: &'a Binding,
        name: String,
        container: File,
        workspace: Option<CloneWorkspace>,
        promoted: bool,
    }

    impl<'a> Stage<'a> {
        fn create(binding: &'a Binding) -> io::Result<Self> {
            for _ in 0..8 {
                let name = format!("{STAGE_PREFIX}{}", uuid::Uuid::new_v4().simple());
                match fs::mkdirat(&binding.parent, name.as_str(), Mode::RWXU) {
                    Ok(()) => {}
                    Err(Errno::EXIST) => continue,
                    Err(err) => return Err(err.into()),
                }
                // If this returns an unexpected entry, do not delete it.
                let container = directory(&binding.parent, name.as_str())?;
                let mut stage = Self {
                    binding,
                    name,
                    container,
                    workspace: None,
                    promoted: false,
                };
                stage.verify()?;
                fs::mkdirat(&stage.container, CHECKOUT, Mode::RWXU)?;
                let checkout = directory(&stage.container, CHECKOUT)?;
                verify_private(&checkout)?;
                stage.workspace = Some(CloneWorkspace {
                    path: binding.parent_path.join(&stage.name).join(CHECKOUT),
                    directory: checkout,
                });
                return Ok(stage);
            }
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Could not allocate a private clone workspace.",
            ))
        }

        fn verify(&self) -> io::Result<()> {
            self.binding.verify()?;
            if !owns(&self.binding.parent, &self.name, &self.container)? {
                return Err(moved(
                    "The private clone staging folder moved or was replaced.",
                ));
            }
            verify_private(&self.container)?;
            if let Some(workspace) = &self.workspace {
                if !owns(&self.container, CHECKOUT, &workspace.directory)? {
                    return Err(moved("The checkout folder moved or was replaced."));
                }
                verify_private(&workspace.directory)?;
            }
            Ok(())
        }

        fn cleanup(&self) -> io::Result<()> {
            // Abandon staging if its original namespace changed. In
            // particular never recurse into a replacement empty directory.
            self.binding.verify()?;
            if !owns(&self.binding.parent, &self.name, &self.container)? {
                return Ok(());
            }
            verify_private(&self.container)?;
            if !self.promoted {
                if let Some(workspace) = &self.workspace {
                    if !owns(&self.container, CHECKOUT, &workspace.directory)? {
                        return Ok(());
                    }
                    verify_private(&workspace.directory)?;
                    clean_contents(&workspace.directory, 0)?;
                    if owns(&self.container, CHECKOUT, &workspace.directory)? {
                        fs::unlinkat(&self.container, CHECKOUT, AtFlags::REMOVEDIR)?;
                    }
                }
            }
            self.binding.verify()?;
            if owns(&self.binding.parent, &self.name, &self.container)? {
                fs::unlinkat(&self.binding.parent, self.name.as_str(), AtFlags::REMOVEDIR)?;
            }
            Ok(())
        }
    }

    impl Drop for Stage<'_> {
        fn drop(&mut self) {
            // Cleanup may intentionally leave a private leftover instead of
            // risking a competitor or deleting an uncertain path identity.
            let _ = self.cleanup();
        }
    }

    fn flags() -> OFlags {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
    }

    fn directory(parent: &File, name: impl rustix::path::Arg) -> io::Result<File> {
        Ok(fs::openat(parent, name, flags(), Mode::empty())?.into())
    }

    fn walk(path: &Path, protected: Option<&Stat>) -> io::Result<File> {
        let mut parts = path.components();
        if parts.next() != Some(Component::RootDir) {
            return Err(invalid("Choose an absolute directory path."));
        }
        let mut dir: File = fs::openat(CWD, "/", flags(), Mode::empty())?.into();
        if let Some(protected) = protected
            && same(&fs::fstat(&dir)?, protected)
        {
            return Err(profile_error());
        }
        for part in parts {
            let Component::Normal(name) = part else {
                return Err(invalid("Choose a canonical directory path."));
            };
            dir = directory(&dir, name)?;
            if let Some(protected) = protected
                && same(&fs::fstat(&dir)?, protected)
            {
                return Err(profile_error());
            }
        }
        Ok(dir)
    }

    fn verify_parent_permissions(parent: &File) -> io::Result<()> {
        let metadata = fs::fstat(parent)?;
        if metadata.st_uid != geteuid().as_raw() || metadata.st_mode & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Choose a clone parent you own that is not group- or world-writable.",
            ));
        }
        Ok(())
    }

    fn verify_private(dir: &File) -> io::Result<()> {
        let metadata = fs::fstat(dir)?;
        if metadata.st_uid != geteuid().as_raw() || metadata.st_mode & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The clone staging directory is no longer private.",
            ));
        }
        Ok(())
    }

    fn entry(dir: &File, name: impl rustix::path::Arg) -> io::Result<Stat> {
        Ok(fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?)
    }

    fn vacant(parent: &File, name: &OsStr) -> io::Result<()> {
        match fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "The exact clone destination already exists. Choose another folder.",
            )),
        }
    }

    fn owns(dir: &File, name: impl rustix::path::Arg, fd: &File) -> io::Result<bool> {
        let existing = match entry(dir, name) {
            Ok(existing) => existing,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err),
        };
        Ok(is_dir(&existing) && same(&existing, &fs::fstat(fd)?))
    }

    fn is_dir(stat: &Stat) -> bool {
        FileType::from_raw_mode(stat.st_mode) == FileType::Directory
    }

    fn same(a: &Stat, b: &Stat) -> bool {
        a.st_dev == b.st_dev && a.st_ino == b.st_ino
    }

    fn clean_contents(dir: &File, depth: usize) -> io::Result<()> {
        if depth >= MAX_CLEANUP_DEPTH {
            return Err(invalid("The staging tree was too deep to clean safely."));
        }
        let device = fs::fstat(dir)?.st_dev;
        let mut entries = Dir::read_from(dir)?;
        for item in &mut entries {
            let item = item?;
            let name = item.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let before = entry(dir, name)?;
            if before.st_dev != device {
                return Err(moved("The staging tree contains another filesystem."));
            }
            if is_dir(&before) {
                let child = directory(dir, name)?;
                if !same(&before, &fs::fstat(&child)?) {
                    return Err(moved("A staging directory was replaced during cleanup."));
                }
                clean_contents(&child, depth + 1)?;
                if !owns(dir, name, &child)? {
                    return Err(moved("A staging directory was replaced during cleanup."));
                }
                fs::unlinkat(dir, name, AtFlags::REMOVEDIR)?;
            } else {
                // Remove symlinks themselves, never their targets.
                if !same(&before, &entry(dir, name)?) {
                    return Err(moved("A staging entry was replaced during cleanup."));
                }
                fs::unlinkat(dir, name, AtFlags::empty())?;
            }
        }
        Ok(())
    }

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message)
    }

    fn moved(message: &'static str) -> io::Error {
        io::Error::other(message)
    }

    fn profile_error() -> io::Error {
        invalid("The clone destination must be outside Butler's active profile.")
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    struct Sandbox {
        root: PathBuf,
        parent: PathBuf,
        profile: PathBuf,
    }

    impl Sandbox {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "butler-clone-test-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let parent = root.join("parent");
            let profile = root.join("profile");
            fs::create_dir(&parent).unwrap();
            fs::create_dir(&profile).unwrap();
            Self {
                root,
                parent,
                profile,
            }
        }

        fn plan(&self) -> CloneDestination {
            CloneDestination::confirmable(&self.parent, OsStr::new("repo"), &self.profile).unwrap()
        }

        fn assert_no_staging(&self) {
            for item in fs::read_dir(&self.parent).unwrap() {
                assert!(
                    !item
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".butler-clone-")
                );
            }
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).ok();
        }
    }

    fn fake(workspace: &CloneWorkspace) -> io::Result<()> {
        assert!(workspace.directory().metadata()?.is_dir());
        fs::create_dir(workspace.path().join(".git"))?;
        fs::write(
            workspace.path().join(".git/HEAD"),
            b"ref: refs/heads/main\n",
        )?;
        fs::write(workspace.path().join("README.md"), b"fake checkout\n")
    }

    #[test]
    fn exact_destination_is_not_written_until_commit() {
        let s = Sandbox::new();
        let plan = s.plan();
        assert_eq!(plan.path(), s.parent.join("repo"));
        assert_eq!(fs::read_dir(&s.parent).unwrap().count(), 0);
        let out = plan.commit(fake).unwrap();
        assert_eq!(out, s.parent.join("repo"));
        assert!(out.join(".git/HEAD").is_file());
        assert_eq!(fs::read(out.join("README.md")).unwrap(), b"fake checkout\n");
        s.assert_no_staging();
    }

    #[test]
    fn errors_and_fake_git_symlinks_clean_without_following_links() {
        let s = Sandbox::new();
        let marker = s.profile.join("keep");
        fs::write(&marker, b"still here").unwrap();
        let error = s
            .plan()
            .commit(|workspace| {
                fake(workspace)?;
                symlink(&s.profile, workspace.path().join("outside"))?;
                Err(io::Error::other("fake transport failed"))
            })
            .unwrap_err();
        assert_eq!(error.to_string(), "fake transport failed");
        s.assert_no_staging();
        assert!(!s.parent.join("repo").exists());
        assert_eq!(fs::read(&marker).unwrap(), b"still here");

        let error = s
            .plan()
            .commit(|workspace| symlink(&s.profile, workspace.path().join(".git")))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        s.assert_no_staging();
        assert_eq!(fs::read(&marker).unwrap(), b"still here");

        assert!(s.plan().commit(|_| Ok(())).is_err());
        s.assert_no_staging();
    }

    #[test]
    fn exact_empty_competitor_after_checkout_is_never_replaced() {
        use std::os::unix::fs::MetadataExt;
        let s = Sandbox::new();
        let destination = s.parent.join("repo");
        let mut inode = 0;
        let error = s
            .plan()
            .commit(|workspace| {
                fake(workspace)?;
                fs::create_dir(&destination)?;
                inode = fs::metadata(&destination)?.ino();
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::metadata(&destination).unwrap().ino(), inode);
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        s.assert_no_staging();
    }

    #[test]
    fn existing_destinations_missing_parents_symlinks_and_profile_are_rejected_without_writing() {
        let s = Sandbox::new();
        let name = OsStr::new("repo");
        let missing = s.root.join("absent");
        assert!(CloneDestination::confirmable(&missing, name, &s.profile).is_err());
        assert!(!missing.exists());

        let alias = s.root.join("alias");
        symlink(&s.parent, &alias).unwrap();
        assert!(CloneDestination::confirmable(&alias, name, &s.profile).is_err());
        let deeper = s.parent.join("nested");
        fs::create_dir(&deeper).unwrap();
        assert!(CloneDestination::confirmable(&alias.join("nested"), name, &s.profile).is_err());
        fs::remove_dir(deeper).unwrap();

        for invalid in ["", ".", "..", "../repo", "repo/", "some/repo", ".git"] {
            assert!(
                CloneDestination::confirmable(&s.parent, OsStr::new(invalid), &s.profile).is_err()
            );
        }

        let subprofile = s.profile.join("subfolder");
        fs::create_dir(&subprofile).unwrap();
        assert!(CloneDestination::confirmable(&s.profile, name, &s.profile).is_err());
        assert!(CloneDestination::confirmable(&subprofile, name, &s.profile).is_err());
        assert_eq!(fs::read_dir(&subprofile).unwrap().count(), 0);

        let destination = s.parent.join("repo");
        fs::create_dir(&destination).unwrap();
        let error = CloneDestination::confirmable(&s.parent, name, &s.profile)
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        fs::remove_dir(&destination).unwrap();
        symlink(&missing, &destination).unwrap(); // Dangling still counts.
        let error = CloneDestination::confirmable(&s.parent, name, &s.profile)
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        s.assert_no_staging();
    }

    #[test]
    fn race_before_commit_never_calls_callback() {
        let s = Sandbox::new();
        let plan = s.plan();
        fs::create_dir(plan.path()).unwrap();
        let error = plan
            .commit(|_| panic!("must not invoke callback"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        s.assert_no_staging();

        fs::remove_dir(s.parent.join("repo")).unwrap();
        let plan = s.plan();
        fs::rename(&s.parent, s.root.join("old-parent")).unwrap();
        fs::create_dir(&s.parent).unwrap();
        assert!(plan.commit(|_| panic!("must not invoke callback")).is_err());
        assert_eq!(fs::read_dir(&s.parent).unwrap().count(), 0);
        assert_eq!(fs::read_dir(s.root.join("old-parent")).unwrap().count(), 0);
    }

    #[test]
    fn parent_substitution_during_callback_is_not_followed_or_cleaned() {
        let s = Sandbox::new();
        let moved_parent = s.root.join("moved-parent");
        assert!(
            s.plan()
                .commit(|workspace| {
                    fake(workspace)?;
                    fs::rename(&s.parent, &moved_parent)?;
                    symlink(&s.profile, &s.parent)
                })
                .is_err()
        );
        assert_eq!(fs::read_dir(&s.profile).unwrap().count(), 0);
        assert!(!moved_parent.join("repo").exists());
    }

    #[test]
    fn replaced_checkout_or_container_is_not_promoted_or_removed() {
        let s = Sandbox::new();
        let mut replacement = PathBuf::new();
        assert!(
            s.plan()
                .commit(|workspace| {
                    replacement = workspace.path().to_owned();
                    fs::rename(&replacement, replacement.with_file_name("moved-checkout"))?;
                    fs::create_dir(&replacement)?;
                    fs::write(replacement.join("competitor"), b"keep")?;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(replacement.join("competitor")).unwrap(), b"keep");
        assert!(!s.parent.join("repo").exists());

        assert!(
            s.plan()
                .commit(|workspace| {
                    replacement = workspace.path().parent().unwrap().to_owned();
                    fs::rename(&replacement, s.parent.join("moved-container"))?;
                    fs::create_dir(&replacement)?;
                    fs::write(replacement.join("competitor"), b"keep")?;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(replacement.join("competitor")).unwrap(), b"keep");
        assert!(!s.parent.join("repo").exists());
    }
}

#[cfg(all(test, not(any(target_os = "linux", target_os = "macos"))))]
mod unsupported_tests {
    use super::*;

    #[test]
    fn fails_before_inspecting_paths_or_writing() {
        let error = CloneDestination::confirmable(
            Path::new("missing"),
            OsStr::new("repo"),
            Path::new("profile"),
        )
        .err()
        .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
