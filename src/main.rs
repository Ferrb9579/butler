mod empty_workspace;
mod project_picker;
mod provider_settings;
mod welcome;
mod workspace;

use butler::{
    landing::{Landing, resolve_landing},
    onboarding::{Route, StartupEvidence},
    profile::Profile,
    projects::{FolderListing, LocalProjects},
};
use incular::prelude::*;
use project_picker::Picker;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    incular::run(application(
        profile_root(),
        local_home(),
        machine_label(),
        #[cfg(test)]
        None,
    )?)?;
    Ok(())
}

fn application(
    state: Option<PathBuf>,
    home: Option<PathBuf>,
    device: String,
    #[cfg(test)] resume: Option<uuid::Uuid>,
) -> Result<Application, Box<dyn std::error::Error>> {
    let settings_open = Signal::new(false);
    let settings_provider = Signal::new(2_usize);
    #[cfg(test)]
    let fixture_enabled = Signal::new(resume.is_some());
    #[cfg(test)]
    let fixture = offline_fixture::Fixture::new(state.clone(), resume)?;
    let profile = state.as_ref().map(Profile::new);
    let projects = state.map(LocalProjects::new);
    let saved_project = projects.as_ref().map(LocalProjects::list).transpose();
    let active_project = Signal::new(
        saved_project
            .as_ref()
            .ok()
            .and_then(|p| p.as_ref())
            .and_then(|p| p.first())
            .cloned(),
    );
    let picker_error = Signal::new(
        saved_project
            .err()
            .map(|e| format!("Could not load saved projects: {e}")),
    );
    let picker = Signal::new(Picker::Closed);
    let browse = Signal::new(None::<FolderListing>);
    let browse_error = Signal::new(None::<String>);
    let path = TextEditingController::new();
    let completed = profile
        .as_ref()
        .map(Profile::onboarding_completed)
        .transpose();
    let initial_route = StartupEvidence {
        enabled: true,
        hydrated: true,
        previously_completed: matches!(completed, Ok(Some(true))),
        bootstrapped: true,
        authoritative: true,
        workspace_authoritative: true,
        provenance_authoritative: true,
        catalog_ready: true,
        server_config_available: true,
        fresh_workspace: true,
        ..Default::default()
    }
    .resolve()
    .route;
    let route = Signal::new(initial_route);
    let error = Signal::new(completed.err().map(|error| error.to_string()));
    let agents_open = Signal::new(true);
    #[cfg(test)]
    let scratch_open = Signal::new(fixture.resume_scratch);
    #[cfg(not(test))]
    let scratch_open = Signal::new(false);
    let app = Application::new(move |_cx| {
        let root = if route.get() == Route::Welcome {
            let route = route.clone();
            let error_signal = error.clone();
            let profile = profile.clone();
            welcome::view(&device, agents_open.clone(), error.get(), move || {
                let result = profile
                    .as_ref()
                    .ok_or_else(|| {
                        std::io::Error::other("No local profile directory is available.")
                    })
                    .and_then(Profile::complete_onboarding);
                match result {
                    Ok(()) => {
                        error_signal.set(None);
                        route.set(Route::Workspace);
                    }
                    Err(error) => {
                        error_signal.set(Some(format!("Could not save setup: {error}")));
                    }
                }
            })
        } else {
            let project = active_project.get();
            let workspace = if project.is_some() || scratch_open.get() {
                let settings_open = settings_open.clone();
                let title = project.as_ref().map(|project| project.title());
                workspace::draft(
                    title,
                    move || {
                        settings_open.set(true);
                    },
                    #[cfg(test)]
                    fixture_enabled
                        .get()
                        .then(|| fixture.draft(project.map(|p| p.root))),
                )
            } else {
                match resolve_landing(&[], true, profile.is_some()) {
                    Landing::Empty {
                        can_start_without_project,
                    } => {
                        let scratch_open = scratch_open.clone();
                        let picker = picker.clone();
                        let path = path.clone();
                        empty_workspace::view(
                            can_start_without_project,
                            move || {
                                scratch_open.set(true);
                            },
                            move || {
                                path.clear();
                                picker.set(Picker::Sources);
                            },
                        )
                    }
                    _ => unreachable!("no projects are loaded in the current local workspace"),
                }
            };
            let active_project = active_project.clone();
            let step = picker.clone();
            let error = picker_error.clone();
            let projects = projects.clone();
            let desktop = project_picker::overlay(
                workspace,
                picker.clone(),
                path.clone(),
                home.clone(),
                browse.clone(),
                browse_error.clone(),
                picker_error.get(),
                move |input| {
                    let result = projects
                        .as_ref()
                        .ok_or_else(|| {
                            std::io::Error::other("No local profile directory is available.")
                        })
                        .and_then(|store| store.open(Path::new(&input)));
                    match result {
                        Ok(project) => {
                            error.set(None);
                            active_project.set(Some(project));
                            step.set(Picker::Closed);
                        }
                        Err(e) => {
                            error.set(Some(format!("Could not open folder: {e}")));
                        }
                    }
                },
            );
            if settings_open.get() {
                let settings_open = settings_open.clone();
                provider_settings::overlay(
                    desktop,
                    &device,
                    settings_provider.clone(),
                    move || {
                        settings_open.set(false);
                    },
                    #[cfg(test)]
                    fixture_enabled.clone(),
                )
            } else {
                desktop
            }
        };
        Directionality::new(TextDirection::Ltr, root).into()
    })?;
    Ok(app)
}

fn local_home() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn profile_root() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            env::var_os("LOCALAPPDATA")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".local/state"))
        })
        .map(|base| base.join("butler"))
}

fn machine_label() -> String {
    env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "this computer".to_owned())
}

#[cfg(test)]
mod offline_fixture {
    use super::*;
    use butler::{
        scratch::ScratchWorkspaces,
        threads::{FirstTurn, ThreadStore, Workspace},
    };
    use std::{io, rc::Rc};
    use uuid::Uuid;

    pub struct Fixture {
        state: Option<PathBuf>,
        pub resume_scratch: bool,
        thread: Uuid,
        input: TextEditingController,
        sent: Signal<Option<FirstTurn>>,
        error: Signal<Option<String>>,
    }

    impl Fixture {
        pub fn new(state: Option<PathBuf>, resume: Option<Uuid>) -> io::Result<Self> {
            let turn = match resume {
                Some(id) => {
                    let store = ThreadStore::new(
                        state
                            .as_ref()
                            .ok_or_else(|| io::Error::other("No local profile."))?,
                    );
                    let turn = store.load(id)?.ok_or_else(|| {
                        io::Error::new(io::ErrorKind::NotFound, "No saved thread.")
                    })?;
                    store.workspace_directory(&turn)?;
                    Some(turn)
                }
                None => None,
            };
            Ok(Self {
                state,
                resume_scratch: turn
                    .as_ref()
                    .is_some_and(|turn| turn.workspace == Workspace::Scratch),
                thread: resume.unwrap_or_else(Uuid::new_v4),
                input: TextEditingController::new(),
                sent: Signal::new(turn),
                error: Signal::new(None),
            })
        }

        pub fn draft(&self, project: Option<PathBuf>) -> workspace::OfflineDraft {
            let state = self.state.clone();
            let thread = self.thread;
            let sent = self.sent.clone();
            let error = self.error.clone();
            workspace::OfflineDraft {
                input: self.input.clone(),
                sent: sent.get(),
                error: error.get(),
                send: Rc::new(move |prompt| {
                    if prompt.trim().is_empty() || sent.get().is_some() {
                        return;
                    }
                    let result = (|| {
                        let state = state
                            .as_ref()
                            .ok_or_else(|| io::Error::other("No local profile."))?;
                        let target = match &project {
                            Some(project) => Workspace::Local(project.clone()),
                            None => {
                                ScratchWorkspaces::new(state).prepare(
                                    thread,
                                    "2026-11-24",
                                    &prompt,
                                )?;
                                Workspace::Scratch
                            }
                        };
                        let turn = FirstTurn {
                            id: thread,
                            workspace: target,
                            prompt,
                            reply: "Offline test reply received.".into(),
                        };
                        ThreadStore::new(state).save(&turn)?;
                        Ok::<_, io::Error>(turn)
                    })();
                    match result {
                        Ok(turn) => {
                            error.set(None);
                            sent.set(Some(turn));
                        }
                        Err(problem) => {
                            error.set(Some(problem.to_string()));
                        }
                    }
                }),
            }
        }
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use incular::testing::Simulation;
    use std::{fs, thread, time::Duration};

    fn interact<R: Send + 'static>(
        app: &mut Application,
        test: impl FnOnce(Simulation) -> R + Send + 'static,
    ) -> R {
        let simulation = app.simulation();
        let worker = thread::spawn(move || test(simulation));
        for _ in 0..2500 {
            app.process_simulation_requests();
            if worker.is_finished() {
                return worker.join().unwrap();
            }
            thread::park_timeout(Duration::from_millis(1));
        }
        panic!("simulation request was not serviced");
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let tmp = env::temp_dir().join(format!("butler-ui-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&tmp).unwrap();
            Self(tmp)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn imports_a_folder_through_the_picker_then_reopens_the_project() {
        let tmp = Fixture::new();
        let root = tmp.0.join("Native test folder");
        let profile = tmp.0.join("profile");
        fs::create_dir(&root).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let canonical = root.canonicalize().unwrap();
        let path = root.to_str().unwrap().to_owned();
        let mut app = application(
            Some(profile.clone()),
            Some(tmp.0.clone()),
            "Fixture".into(),
            None,
        )
        .unwrap();
        interact(&mut app, move |sim| {
            sim.click("＋ Add project").expect("open source picker");
            sim.click("Local folder")
                .expect("choose existing local folder");
            sim.type_into("Absolute folder path", path)
                .expect("enter real folder path");
            sim.click("Open folder").expect("submit local folder");
        });
        let saved = LocalProjects::new(&profile).list().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].root, canonical);
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        drop(app);
        let mut reopened =
            application(Some(profile), Some(tmp.0.clone()), "Fixture".into(), None).unwrap();
        interact(&mut reopened, |sim| {
            sim.click("Native test folder")
                .expect("reopened UI displays the imported project");
            assert!(
                sim.click("＋ Add project").is_err(),
                "reopened application must not show the empty workspace"
            );
        });
    }
    #[test]
    fn traverses_children_and_imports_the_displayed_directory() {
        let tmp = Fixture::new();
        let home = tmp.0.join("Home");
        let root = home.join("Category").join("Browsable project");
        let profile = tmp.0.join("profile");
        fs::create_dir_all(&root).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let mut app =
            application(Some(profile.clone()), Some(home), "Fixture".into(), None).unwrap();
        interact(&mut app, |sim| {
            sim.click("＋ Add project").unwrap();
            sim.click("Local folder").unwrap();
            sim.click("Category").unwrap();
            sim.click("Browsable project").unwrap();
            sim.click("Open folder").unwrap();
        });
        let projects = LocalProjects::new(profile).list().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root, root.canonicalize().unwrap());
    }
    #[test]
    fn opens_provider_settings_then_tests_first_send_offline() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        Profile::new(&profile).complete_onboarding().unwrap();
        let mut app = application(
            Some(profile.clone()),
            Some(tmp.0.clone()),
            "Fixture".into(),
            None,
        )
        .unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click("Claude").unwrap();
            sim.click("Sign-in for Claude isn't available in this build.")
                .unwrap();
            sim.click("Enable offline test provider").unwrap();
            sim.click("Back to workspace").unwrap();
            assert!(sim.focus("Ask anything").is_ok());
        });
        assert!(!profile.join("scratch").exists());
        interact(&mut app, |sim| {
            sim.type_into("Ask anything", "Check my draft").unwrap();
            sim.click("Send").unwrap();
            sim.click("Offline test reply received.").unwrap();
        });
        let mut entries = fs::read_dir(profile.join("scratch")).unwrap();
        let claimed = entries.next().unwrap().unwrap();
        assert!(
            claimed
                .file_name()
                .to_string_lossy()
                .starts_with("2026-11-24-check-my-draft-")
        );
        assert!(entries.next().is_none());
        let records = fs::read_dir(profile.join("threads"))
            .unwrap()
            .map(Result::unwrap)
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let id = uuid::Uuid::parse_str(records[0].file_stem().unwrap().to_str().unwrap()).unwrap();
        drop(app);
        let mut reopened = application(
            Some(profile),
            Some(tmp.0.clone()),
            "Fixture".into(),
            Some(id),
        )
        .unwrap();
        interact(&mut reopened, |sim| {
            sim.click("Check my draft").unwrap();
            sim.click("Offline test reply received.").unwrap();
        });
    }
}
