mod empty_workspace;
mod project_picker;
mod welcome;
mod workspace;

use butler::{
    landing::{Landing, resolve_landing},
    onboarding::{Route, StartupEvidence},
    profile::Profile,
    projects::LocalProjects,
};
use incular::prelude::*;
use project_picker::Picker;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = profile_root();
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
    let device = machine_label();

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
    incular::run(app)?;
    Ok(())
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
