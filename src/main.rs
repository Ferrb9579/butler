mod empty_workspace;
mod project_picker;
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
    incular::run(application(profile_root(), local_home(), machine_label())?)?;
    Ok(())
}

fn application(
    state: Option<PathBuf>,
    home: Option<PathBuf>,
    device: String,
) -> Result<Application, Box<dyn std::error::Error>> {
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
            let workspace = if let Some(project) = active_project.get() {
                workspace::no_provider_draft(Some(project.title()))
            } else if scratch_open.get() {
                workspace::no_provider_draft(None)
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
            project_picker::overlay(
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
            )
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
        let mut app =
            application(Some(profile.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
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
            application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
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
        let mut app = application(Some(profile.clone()), Some(home), "Fixture".into()).unwrap();
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
}
