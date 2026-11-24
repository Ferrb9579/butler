//! Explicit public HTTPS transport for confirmed clone staging.
//! No Git process is created by URL validation or plan construction.

use crate::{clone_stage::CloneWorkspace, remote_projects};
use std::io;
use url::Url;

const HTTPS_HOST_ERROR: &str =
    "Cloning supports explicit HTTPS URLs on github.com or gitlab.com only.";

pub fn validate_url(input: &str) -> io::Result<Url> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, HTTPS_HOST_ERROR);
    if input.len() > 4096 || input.trim() != input {
        return Err(invalid());
    }
    let authority = input
        .strip_prefix("https://")
        .and_then(|rest| rest.split_once('/'))
        .map(|(authority, _)| authority)
        .ok_or_else(invalid)?;
    let host = authority.strip_suffix(":443").unwrap_or(authority);
    if !["github.com", "gitlab.com"]
        .iter()
        .any(|expected| host.eq_ignore_ascii_case(expected))
    {
        return Err(invalid());
    }
    let url = Url::parse(input).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("github.com" | "gitlab.com"))
        // The URL parser canonicalizes an explicit TLS-default 443 to None.
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    // Inspect raw segments independently of Url's normalization (e.g. encoded
    // traversal or empty userinfo). The runner may not rewrite a user-confirmed
    // address: only the first UI step may normalize a repository page.
    let prepared = remote_projects::parse(input).map_err(|_| invalid())?;
    if prepared.input != input || prepared.url != url.as_str() {
        return Err(invalid());
    }
    Ok(url)
}

/// Clone only after the user confirms an independently staged destination.
/// Errors are intentionally generic: no Git stderr or URL is exposed.
pub fn clone_https(input: &str, workspace: &CloneWorkspace) -> io::Result<()> {
    let url = validate_url(input)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        local::clone_checkout(&url, workspace.directory())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (url, workspace);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Safe HTTPS cloning is unavailable on this platform.",
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod local {
    use super::*;
    use rustix::{
        fs::{Dir, FileType, Mode, OFlags, fstat, openat},
        io::Errno,
        process::{Pid, Signal, WaitId, WaitIdOptions, fchdir, kill_process_group, waitid},
    };
    use std::{
        fs::{self, File},
        io::Read,
        os::unix::{fs::MetadataExt, process::CommandExt},
        path::{Path, PathBuf},
        process::{Child, Command, ExitStatus, Stdio},
        thread,
        time::{Duration, Instant},
    };

    const CLONE_TIMEOUT: Duration = Duration::from_secs(120);
    const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

    #[cfg(target_os = "linux")]
    const SYSTEM_GIT: &[(&str, &str)] = &[
        ("/usr/bin/git", "/usr/lib/git-core"),
        ("/usr/bin/git", "/usr/libexec/git-core"),
    ];
    // /usr/bin/git on macOS is an xcrun shim which can request installation.
    #[cfg(target_os = "macos")]
    const SYSTEM_GIT: &[(&str, &str)] = &[
        (
            "/Library/Developer/CommandLineTools/usr/bin/git",
            "/Library/Developer/CommandLineTools/usr/libexec/git-core",
        ),
        (
            "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
            "/Applications/Xcode.app/Contents/Developer/usr/libexec/git-core",
        ),
    ];

    const CONFIG: &[&str] = &[
        "protocol.allow=never",
        "protocol.https.allow=always",
        "http.followRedirects=false",
        "http.sslVerify=true",
        "http.proxy=",
        "credential.helper=",
        "credential.interactive=false",
        "core.hooksPath=/dev/null",
        "core.attributesFile=/dev/null",
        "core.excludesFile=/dev/null",
        "core.autocrlf=false",
        "core.protectHFS=true",
        "core.protectNTFS=true",
        "commit.gpgSign=false",
        "tag.gpgSign=false",
        "submodule.recurse=false",
        "fetch.recurseSubmodules=false",
        "maintenance.auto=false",
        "gc.auto=0",
        "transfer.fsckObjects=true",
    ];
    // No ambient proxy, credential, trace, PATH, Git config, SSH or loader
    // values. libcurl's optional .netrc cannot exist below this HOME.
    const ENV: &[(&str, &str)] = &[
        ("HOME", "/dev/null"),
        ("XDG_CONFIG_HOME", "/dev/null"),
        ("XDG_CACHE_HOME", "/dev/null"),
        ("XDG_DATA_HOME", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_SYSTEM", "/dev/null"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_ATTR_NOSYSTEM", "1"),
        ("GIT_ALLOW_PROTOCOL", "https"),
        ("GIT_PROTOCOL_FROM_USER", "0"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_NO_REPLACE_OBJECTS", "1"),
        ("LC_ALL", "C"),
    ];

    struct GitTool {
        executable: PathBuf,
        helper_directory: PathBuf,
    }

    impl GitTool {
        fn installed() -> io::Result<Self> {
            SYSTEM_GIT
                .iter()
                .find_map(|(git, helpers)| {
                    Self::from_system_pair(Path::new(git), Path::new(helpers))
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "A trusted system Git with its HTTPS helper is required to clone.",
                    )
                })
        }

        fn from_system_pair(git: &Path, helpers: &Path) -> Option<Self> {
            // Execute the validated canonical Git image, never a runtime PATH hit.
            let executable = git.canonicalize().ok()?;
            let helper_directory = helpers.canonicalize().ok()?;
            if !trusted_parents(executable.parent()?) || !trusted_parents(&helper_directory) {
                return None;
            }
            trusted_image(&executable)?;

            // Debian uses this single relative alias; do not follow a helper
            // chain through an unverified or user-writable directory.
            let helper = helper_directory.join("git-remote-https");
            let metadata = fs::symlink_metadata(&helper).ok()?;
            let image = if metadata.file_type().is_symlink() {
                if metadata.uid() != 0
                    || fs::read_link(&helper).ok()?.as_os_str() != "git-remote-http"
                {
                    return None;
                }
                helper_directory.join("git-remote-http")
            } else {
                helper
            };
            trusted_image(&image)?;
            Some(Self {
                executable,
                helper_directory,
            })
        }

        fn command(&self) -> Command {
            let mut command = Command::new(&self.executable);
            command
                .env_clear()
                .envs(ENV.iter().copied())
                .env("GIT_EXEC_PATH", &self.helper_directory)
                .env("PATH", &self.helper_directory)
                .arg("--no-pager");
            for option in CONFIG {
                command.args(["-c", option]);
            }
            command
        }

        fn clone_command(&self, url: &Url) -> Command {
            let mut command = self.command();
            command.args([
                "clone",
                "--quiet",
                "--no-recurse-submodules",
                "--template=",
                "--",
                url.as_str(),
                ".",
            ]);
            command
        }

        fn verify_command(&self) -> Command {
            let mut command = self.command();
            command.args([
                "--git-dir=.git",
                "--work-tree=.",
                "rev-parse",
                "--verify",
                "--quiet",
                "HEAD^{commit}",
            ]);
            command
        }
    }

    fn trusted_parents(path: &Path) -> bool {
        path.is_absolute()
            && path.ancestors().all(|p| {
                fs::symlink_metadata(p).is_ok_and(|metadata| {
                    metadata.file_type().is_dir()
                        && metadata.uid() == 0
                        && metadata.mode() & 0o022 == 0
                })
            })
    }

    fn trusted_image(path: &Path) -> Option<()> {
        let metadata = fs::symlink_metadata(path).ok()?;
        if !metadata.file_type().is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o7022 != 0
            || metadata.mode() & 0o111 == 0
        {
            return None;
        }
        let mut magic = [0; 4];
        File::open(path).ok()?.read_exact(&mut magic).ok()?;
        #[cfg(target_os = "linux")]
        let native = magic == *b"\x7fELF";
        #[cfg(target_os = "macos")]
        let native = matches!(
            magic,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        );
        native.then_some(())
    }

    pub(super) fn clone_checkout(url: &Url, directory: &File) -> io::Result<()> {
        let git = GitTool::installed()?;
        if !workspace_empty(directory)? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "The clone staging directory is not empty.",
            ));
        }
        if !run(&mut git.clone_command(url), directory, CLONE_TIMEOUT)?.success() {
            return Err(io::Error::other(
                "Clone failed. Check that the repository is public and the URL is correct.",
            ));
        }
        verify_checkout(&git, directory)
    }

    fn workspace_empty(directory: &File) -> io::Result<bool> {
        let mut dir = Dir::read_from(directory)
            .map_err(|_| io::Error::other("Could not inspect the clone staging directory."))?;
        while let Some(entry) = dir.read() {
            let entry = entry
                .map_err(|_| io::Error::other("Could not inspect the clone staging directory."))?;
            if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn verify_checkout(git: &GitTool, directory: &File) -> io::Result<()> {
        let valid = || -> rustix::io::Result<bool> {
            let repo = openat(
                directory,
                ".git",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?;
            let head = openat(
                &repo,
                "HEAD",
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )?;
            Ok(FileType::from_raw_mode(fstat(&head)?.st_mode) == FileType::RegularFile)
        };
        if !valid().unwrap_or(false)
            || !run(&mut git.verify_command(), directory, VERIFY_TIMEOUT)?.success()
        {
            return Err(io::Error::other("Git did not produce a complete checkout."));
        }
        Ok(())
    }

    fn run(command: &mut Command, directory: &File, timeout: Duration) -> io::Result<ExitStatus> {
        let directory = directory
            .try_clone()
            .map_err(|_| io::Error::other("Could not open the clone staging directory."))?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        // SAFETY: the closure borrows only an owned, CLOEXEC directory FD and
        // makes the async-signal-safe fchdir(2) call. Its error is a raw errno;
        // it does not allocate, lock, or resolve a display path before exec.
        unsafe {
            command.pre_exec(move || fchdir(&directory).map_err(io::Error::from));
        }
        let mut child = command
            .spawn()
            .map_err(|_| io::Error::other("Could not start the trusted Git process."))?;
        wait_bounded(&mut child, timeout)
    }

    fn wait_bounded(child: &mut Child, timeout: Duration) -> io::Result<ExitStatus> {
        let pid = Pid::from_raw(child.id() as i32)
            .ok_or_else(|| io::Error::other("Could not identify the Git process."))?;
        let started = Instant::now();
        let observation = loop {
            match waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            ) {
                Ok(Some(_)) => break Ok(()),
                Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
                Ok(None) => break Err(io::Error::new(io::ErrorKind::TimedOut, "Git timed out.")),
                Err(Errno::INTR) if started.elapsed() < timeout => continue,
                Err(Errno::INTR) => {
                    break Err(io::Error::new(io::ErrorKind::TimedOut, "Git timed out."));
                }
                // A foreign reaper would remove the PID reservation. Never
                // send a group signal when this no longer identifies our child.
                Err(Errno::CHILD) => {
                    return Err(io::Error::other("Could not track the Git process."));
                }
                Err(_) => break Err(io::Error::other("Could not track the Git process.")),
            }
        };
        // Even a failed/early leader cannot leave a helper behind. Observe
        // without reaping, then signal while the group's ID is still reserved.
        // No pipe joins: HTTPS and its synchronous Git helpers inherit null I/O.
        let killed = kill_process_group(pid, Signal::KILL);
        if !matches!(killed, Ok(()) | Err(Errno::SRCH)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("Could not stop the Git process group."));
        }
        let status = child
            .wait()
            .map_err(|_| io::Error::other("Could not finish the Git process."));
        observation?;
        status
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::ffi::OsStr;
        use uuid::Uuid;

        struct Sandbox(PathBuf);
        impl Sandbox {
            fn new() -> Self {
                let path =
                    std::env::temp_dir().join(format!("butler-clone-runner-{}", Uuid::new_v4()));
                fs::create_dir(&path).unwrap();
                Self(path)
            }
        }
        impl Drop for Sandbox {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn offline_git() -> GitTool {
            // Bypass installation policy solely to test *offline* Git built-ins
            // inside a sandbox, including the remapped build environment.
            GitTool {
                executable: PathBuf::from("/usr/bin/git"),
                helper_directory: PathBuf::from("/nonexistent-butler-test-help"),
            }
        }

        #[test]
        fn clone_plan_has_only_fixed_https_transport_and_isolated_configuration() {
            let git = offline_git();
            let url = validate_url("https://github.com/Team/Repo.git").unwrap();
            let plan = git.clone_command(&url);
            assert_eq!(plan.get_program(), OsStr::new("/usr/bin/git"));
            assert!(plan.get_current_dir().is_none());
            let args = plan.get_args().map(OsStr::to_str).collect::<Vec<_>>();
            assert!(args.ends_with(&[
                Some("clone"),
                Some("--quiet"),
                Some("--no-recurse-submodules"),
                Some("--template="),
                Some("--"),
                Some(url.as_str()),
                Some("."),
            ]));
            for option in CONFIG {
                assert!(args.windows(2).any(|p| p == [Some("-c"), Some(option)]));
            }
            let env = plan.get_envs().collect::<Vec<_>>();
            assert_eq!(env.len(), ENV.len() + 2);
            assert!(env.contains(&(OsStr::new("GIT_ALLOW_PROTOCOL"), Some(OsStr::new("https")))));
            assert!(env.contains(&(OsStr::new("HOME"), Some(OsStr::new("/dev/null")))));
            assert!(
                !env.iter()
                    .any(|(key, _)| key.eq_ignore_ascii_case("https_proxy"))
            );
            // This plan is deliberately never spawned.
        }

        #[cfg(target_os = "linux")]
        #[test]
        fn checks_local_checkout_and_uses_open_fd_after_path_replacement() {
            if !Path::new("/usr/bin/git").is_file() {
                return;
            }
            let temp = Sandbox::new();
            let initial = temp.0.join("initial");
            let moved = temp.0.join("moved");
            fs::create_dir(&initial).unwrap();
            let fd = File::open(&initial).unwrap();
            fs::rename(&initial, &moved).unwrap();
            fs::create_dir(&initial).unwrap();
            let git = offline_git();
            assert!(workspace_empty(&fd).unwrap());
            assert!(verify_checkout(&git, &fd).is_err());
            let mut init = git.command();
            init.args(["init", "--quiet", "--template=", "--initial-branch=main"]);
            assert!(run(&mut init, &fd, VERIFY_TIMEOUT).unwrap().success());
            assert!(moved.join(".git").is_dir());
            assert!(!initial.join(".git").exists());
            assert!(!workspace_empty(&fd).unwrap());
            assert!(
                verify_checkout(&git, &fd).is_err(),
                "unborn HEAD is not a checkout"
            );
            let mut commit = git.command();
            commit.args([
                "-c",
                "user.name=Local Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "--allow-empty",
                "--no-gpg-sign",
                "--message=Fixture",
            ]);
            assert!(run(&mut commit, &fd, VERIFY_TIMEOUT).unwrap().success());
            verify_checkout(&git, &fd).unwrap();
        }

        #[test]
        fn timeout_kills_the_owned_group_and_waits_for_the_leader() {
            let temp = Sandbox::new();
            let fd = File::open(&temp.0).unwrap();
            // Use the full path regardless of whether this file is crate root.
            let test_name = module_path!()
                .split("::")
                .skip(1)
                .collect::<Vec<_>>()
                .join("::");
            let test_name = format!("{test_name}::offline_fixture_member");
            // The harness needs the exact qualified name, and the same name
            // is passed to the grandchild without a shell.
            let mut fixture = Command::new(std::env::current_exe().unwrap());
            fixture
                .env_clear()
                .env("BUTLER_CLONE_FIXTURE", "leader")
                .env("BUTLER_CLONE_FIXTURE_TEST", &test_name)
                .args(["--exact", &test_name, "--nocapture"]);
            let result = run(&mut fixture, &fd, Duration::from_millis(800)).unwrap_err();
            assert_eq!(result.kind(), io::ErrorKind::TimedOut);
            assert!(temp.0.join("helper-started").exists());
            thread::sleep(Duration::from_millis(900));
            assert!(!temp.0.join("helper-finished").exists());
        }

        #[test]
        fn offline_fixture_member() {
            match std::env::var("BUTLER_CLONE_FIXTURE").as_deref() {
                Ok("leader") => {
                    let name = std::env::var("BUTLER_CLONE_FIXTURE_TEST").unwrap();
                    let mut helper = Command::new(std::env::current_exe().unwrap())
                        .env_clear()
                        .env("BUTLER_CLONE_FIXTURE", "helper")
                        .args(["--exact", &name, "--nocapture"])
                        .spawn()
                        .unwrap();
                    // Match the trusted Git parent: waits on the HTTPS helper
                    // in ordinary operation, but both belong to the same group.
                    let _ = helper.wait().unwrap();
                }
                Ok("helper") => {
                    fs::write("helper-started", []).unwrap();
                    thread::sleep(Duration::from_millis(1500));
                    fs::write("helper-finished", []).unwrap();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_rechecks_host_transport_and_raw_safety() {
        for input in [
            "https://github.com/Owner/Repo.git",
            "https://gitlab.com/Group/Subgroup/R%C3%A9po.git",
            "https://github.com:443/Owner/Repo.git",
        ] {
            assert!(validate_url(input).is_ok());
        }
        for input in [
            "http://github.com/owner/repo.git",
            "https://www.github.com/owner/repo.git",
            "https://github.com./owner/repo.git",
            "https://github.com.example.com/owner/repo.git",
            "https://github.com:8443/owner/repo.git",
            "https://github.com:/owner/repo.git",
            "https://ｇｉｔｈｕｂ.com/owner/repo.git",
            "https://credential@github.com/owner/repo.git",
            "https://@github.com/owner/repo.git",
            "https://github.com/owner/repo.git?",
            "https://github.com/owner/repo.git#",
            "https://github.com/owner/%2E%2E/repo.git",
            "https://github.com/owner/%2Frepo.git",
            "https://%67ithub.com/owner/repo.git",
            "https://github.com/owner/repo/tree/main",
            "https://127.0.0.1/repo",
            "https://[::1]/repo",
            "owner/repo",
            "git@github.com:owner/repo.git",
            "file:///tmp/example",
            "ext::something",
        ] {
            let error = validate_url(input).unwrap_err();
            assert_eq!(error.to_string(), HTTPS_HOST_ERROR);
        }
    }
}
