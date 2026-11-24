//! Read the named HEAD of one registered worktree without changing it.
use std::path::Path;

/// A missing, detached, unsupported or non-UTF-8 branch has no read-only label.
pub fn named(root: &Path) -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        local::probe(
            &crate::clone_runner::local::system_git_for_local_builtins()?,
            root,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = root;
        None
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod local {
    use super::*;
    use crate::clone_runner::local::{anchor_process, wait_bounded};
    use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};
    use std::{
        fs::File,
        io::Read,
        os::unix::ffi::OsStrExt,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    const MAX_OUTPUT: usize = 16 * 1024;
    const TIMEOUT: Duration = Duration::from_secs(2);

    pub(super) fn probe(git: &Path, root: &Path) -> Option<String> {
        if !root.is_absolute() || root.canonicalize().ok()? != root {
            return None;
        }
        let directory: File = fs::open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?
        .into();
        let entry = fs::statat(&directory, ".git", AtFlags::SYMLINK_NOFOLLOW).ok()?;
        // A normal worktree has a directory. Linked worktrees and submodules
        // use a Git file. Don't discover a parent repository or follow a .git
        // symlink; the built-in verifies root and interprets valid Git files.
        if !matches!(
            FileType::from_raw_mode(entry.st_mode),
            FileType::Directory | FileType::RegularFile
        ) {
            return None;
        }
        let worktree = run(git, &directory, &["rev-parse", "--show-toplevel"])?;
        if worktree.strip_suffix(b"\n")? != root.as_os_str().as_bytes() {
            return None;
        }
        let head = run(git, &directory, &["symbolic-ref", "--quiet", "HEAD"])?;
        branch(&head)
    }

    fn branch(stdout: &[u8]) -> Option<String> {
        let branch =
            std::str::from_utf8(stdout.strip_suffix(b"\n")?.strip_prefix(b"refs/heads/")?).ok()?;
        (!branch.is_empty() && !branch.chars().any(char::is_control)).then(|| branch.to_owned())
    }

    fn command(git: &Path) -> Command {
        let mut command = Command::new(git);
        command
            .env_clear()
            .envs([
                ("HOME", "/dev/null"),
                ("XDG_CONFIG_HOME", "/dev/null"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
                ("GIT_CONFIG_SYSTEM", "/dev/null"),
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_ALLOW_PROTOCOL", ""),
                ("GIT_TERMINAL_PROMPT", "0"),
                ("GIT_OPTIONAL_LOCKS", "0"),
                ("GIT_NO_LAZY_FETCH", "1"),
                ("LC_ALL", "C"),
            ])
            .args([
                "--no-pager",
                "--git-dir=.git",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "maintenance.auto=false",
                "-c",
                "gc.auto=0",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }

    fn run(git: &Path, directory: &File, args: &[&str]) -> Option<Vec<u8>> {
        let mut command = command(git);
        anchor_process(command.args(args), directory).ok()?;
        let mut child = command.spawn().ok()?;
        let stdout = child.stdout.take().expect("piped Git stdout");
        thread::scope(|scope| {
            let reader = scope.spawn(|| {
                let mut result = Vec::new();
                stdout
                    .take((MAX_OUTPUT + 1) as u64)
                    .read_to_end(&mut result)?;
                std::io::Result::Ok(result)
            });
            let status = wait_bounded(&mut child, TIMEOUT).ok()?;
            let output = reader.join().ok()?.ok()?;
            (status.success() && output.len() <= MAX_OUTPUT).then_some(output)
        })
    }

    #[cfg(all(test, target_os = "linux"))]
    mod tests {
        use super::*;
        use std::{ffi::OsStr, fs, os::unix::fs::symlink, path::PathBuf};

        struct Fixture(PathBuf);

        impl Fixture {
            fn new() -> Self {
                let path = std::env::temp_dir()
                    .canonicalize()
                    .unwrap()
                    .join(format!("butler-branch-{}", uuid::Uuid::new_v4()));
                fs::create_dir(&path).unwrap();
                Self(path)
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        const GIT: &str = "/usr/bin/git";

        fn fixture_git(root: &Path, args: &[&str]) {
            assert!(
                Command::new(GIT)
                    .env_clear()
                    .current_dir(root)
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .args([
                        "-c",
                        "core.hooksPath=/dev/null",
                        "-c",
                        "user.name=Offline fixture",
                        "-c",
                        "user.email=offline@example.test"
                    ])
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }

        #[test]
        fn named_head_in_normal_unborn_and_linked_worktrees_but_not_detached_or_plain() {
            let git = Path::new(GIT);
            if !git.is_file() {
                return;
            }
            let fixture = Fixture::new();
            let root = fixture.0.join("normal checkout");
            fs::create_dir(&root).unwrap();
            assert_eq!(probe(git, &root), None);
            fixture_git(
                &root,
                &["init", "--initial-branch=owner/unborn", "--template="],
            );
            assert_eq!(probe(git, &root).as_deref(), Some("owner/unborn"));
            assert!(!root.join(".git/index").exists());
            let child = root.join("unregistered subtree");
            fs::create_dir(&child).unwrap();
            assert_eq!(probe(git, &child), None);
            fixture_git(&root, &["commit", "--allow-empty", "-m", "offline fixture"]);
            let linked = fixture.0.join("linked worktree");
            fixture_git(
                &root,
                &[
                    "worktree",
                    "add",
                    "-b",
                    "feature/linked",
                    linked.to_str().unwrap(),
                ],
            );
            assert!(linked.join(".git").is_file());
            assert_eq!(probe(git, &linked).as_deref(), Some("feature/linked"));
            assert_eq!(probe(git, &root).as_deref(), Some("owner/unborn"));
            let git_file = fs::read_to_string(linked.join(".git")).unwrap();
            let worktree_git_dir = Path::new(git_file.strip_prefix("gitdir: ").unwrap().trim());
            let commit = Command::new(GIT)
                .env_clear()
                .current_dir(&linked)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            assert!(commit.status.success());
            fs::write(worktree_git_dir.join("HEAD"), commit.stdout).unwrap();
            assert_eq!(probe(git, &linked), None);
            assert_eq!(fs::read_to_string(linked.join(".git")).unwrap(), git_file);
            let alias = fixture.0.join("noncanonical alias");
            symlink(&root, &alias).unwrap();
            assert_eq!(probe(git, &alias), None);
            let deceptive = fixture.0.join("untrusted git symlink");
            fs::create_dir(&deceptive).unwrap();
            symlink(root.join(".git"), deceptive.join(".git")).unwrap();
            assert_eq!(probe(git, &deceptive), None);
            fs::remove_file(deceptive.join(".git")).unwrap();
            fs::write(deceptive.join(".git"), b"malformed git file").unwrap();
            assert_eq!(probe(git, &deceptive), None);

            let bare = fixture.0.join("bare");
            fs::create_dir(&bare).unwrap();
            fixture_git(&bare, &["init", "--bare", "--template="]);
            assert_eq!(probe(git, &bare), None);
        }

        #[test]
        fn probe_uses_fixed_commands_and_no_inherited_repository_or_profile_values() {
            let plan = command(Path::new(GIT));
            let env = plan.get_envs().map(|(key, _)| key).collect::<Vec<_>>();
            for excluded in [
                "PATH",
                "GIT_DIR",
                "GIT_WORK_TREE",
                "GIT_CONFIG_COUNT",
                "GIT_CONFIG_PARAMETERS",
                "GIT_COMMON_DIR",
                "GIT_INDEX_FILE",
                "GIT_NAMESPACE",
                "GIT_OBJECT_DIRECTORY",
                "GIT_TRACE",
                "SSH_AUTH_SOCK",
                "GITHUB_TOKEN",
            ] {
                assert!(!env.contains(&OsStr::new(excluded)));
            }
            assert!(
                plan.get_envs()
                    .any(|(key, value)| key == "HOME" && value == Some(OsStr::new("/dev/null")))
            );
            assert_eq!(
                branch(b"refs/heads/team/feature\n").as_deref(),
                Some("team/feature")
            );
            for invalid in [
                &b"refs/tags/not-a-branch\n"[..],
                &b"refs/heads/foo\nbar\n"[..],
                &b"refs/heads/\xff\n"[..],
            ] {
                assert_eq!(branch(invalid), None);
            }
        }
    }
}
