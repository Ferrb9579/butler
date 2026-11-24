mod about_settings;
mod empty_workspace;
mod message_jump;
mod message_search;
mod project_favicon;
mod project_icon_view;
mod project_identity;
mod project_picker;
mod provider_settings;
mod saved_composer;
mod settings_icons;
mod sidebar_search;
mod thread_details;
mod thread_find;
mod thread_find_bar;
mod thread_title;
mod typography;
mod ui;
mod welcome;
mod workspace;

use butler::{
    clone_runner,
    clone_stage::{CloneDestination, CloneWorkspace},
    landing::{Landing, resolve_landing},
    managed_projects,
    onboarding::{Route, StartupEvidence},
    profile::Profile,
    projects::{FolderListing, LocalProject, LocalProjects},
    remote_projects::{self, RemoteProject},
    threads::{Conversation, ThreadStore, Workspace},
};
use incular::prelude::*;
use project_picker::{Picker, ProjectRequest, RemoteFlow, ThreadProjects};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    rc::Rc,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    incular::run(application(profile_root(), local_home(), machine_label())?)?;
    Ok(())
}

type CloneTransport = fn(&str, &CloneWorkspace) -> std::io::Result<()>;

#[derive(Clone, Copy)]
enum ProjectAction {
    Open,
    Create,
    Clone,
}

enum ProjectWork {
    Create(String),
    Clone {
        remote: RemoteProject,
        destination: CloneDestination,
    },
}

fn application(
    state: Option<PathBuf>,
    home: Option<PathBuf>,
    device: String,
) -> Result<Application, Box<dyn std::error::Error>> {
    application_with_transport(state, home, device, clone_runner::clone_https)
}

fn application_with_transport(
    state: Option<PathBuf>,
    home: Option<PathBuf>,
    device: String,
    clone_transport: CloneTransport,
) -> Result<Application, Box<dyn std::error::Error>> {
    typography::register();
    let settings_open = Signal::new(false);
    let settings_navigation = provider_settings::Navigation::new();
    let settings_provider = Signal::new(2_usize);
    #[cfg(test)]
    let fixture_enabled = Signal::new(false);
    #[cfg(test)]
    let fixture = offline_fixture::Fixture::new(state.clone());
    let profile = state.as_ref().map(Profile::new);
    let provider_names = provider_settings::Names::new(profile.clone());
    let history_store = state.as_ref().map(ThreadStore::new);
    let details_profile = state.clone();
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
    let history_workspace = active_project
        .get()
        .map_or(Workspace::Scratch, |p| Workspace::Local(p.root));
    let initial_history = history_store
        .as_ref()
        .map(|store| store.history(&history_workspace))
        .transpose();
    let history_error = Signal::new(
        initial_history
            .as_ref()
            .err()
            .map(|e| format!("Could not read threads: {e}")),
    );
    let thread_history = Signal::new(initial_history.ok().flatten().unwrap_or_default());
    let selected_thread = Signal::new(None::<Conversation>);
    let unsent_saved_drafts = Rc::new(RefCell::new(BTreeMap::new()));
    let message_jump = Signal::new(None::<message_jump::Jump>);
    let posted_message_jump = Cell::new(None);
    let thread_find_input = TextEditingController::new();
    let thread_title_editor = thread_title::Editor::default();
    let details_visibility = thread_details::Visibility::new(details_profile);
    let picker_error = Signal::new(
        saved_project
            .err()
            .map(|e| format!("Could not load saved projects: {e}")),
    );
    let picker = Signal::new(Picker::Closed);
    let browse = Signal::new(None::<FolderListing>);
    let browse_error = Signal::new(None::<String>);
    let path = TextEditingController::new();
    let new_project_name = TextEditingController::new();
    let new_project_preview = Signal::new(String::new());
    let remote_flow = RemoteFlow::new();
    let thread_projects = ThreadProjects::new();
    let sidebar_search = sidebar_search::Search::new();
    let message_lookup = message_search::Lookup::default();
    let project_icon_lookup = project_icon_view::Lookup::default();
    let project_icon_revision = Signal::new(0_u64);
    let message_history_revision = Signal::new(0_u64);
    let project_busy = Signal::new(false);
    let queued_project = Rc::new(RefCell::new(None::<ProjectWork>));
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
    let options = WindowOptions::new("Butler");
    #[cfg(test)]
    let options = WindowOptions {
        initial_logical_size: Size::new(1280.0, 800.0),
        ..options
    };
    let app = Application::new_with_options(options, move |cx| {
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
                let settings_navigation = settings_navigation.clone();
                let title = project.as_ref().map(|project| project.title());
                let project_icon = project_icon_lookup.poll(
                    cx,
                    project.as_ref().map(|project| project.root.as_path()),
                    project_icon_revision.get(),
                );
                let add_project = title.as_ref().map(|_| {
                    let picker = picker.clone();
                    let path = path.clone();
                    let error = picker_error.clone();
                    move || {
                        path.clear();
                        error.set(None);
                        picker.set(Picker::Sources);
                    }
                });
                let selected = selected_thread.clone();
                let new_draft = selected_thread.clone();
                let jump_on_open = message_jump.clone();
                let input_on_open = thread_find_input.clone();
                let jump_on_new = message_jump.clone();
                let here_jump = message_jump.clone();
                let here_draft = selected_thread.clone();
                let rename_on_open = thread_title_editor.clone();
                let rename_on_new = thread_title_editor.clone();
                let rename_here = thread_title_editor.clone();
                let current = selected_thread.get();
                let saved_input = current.as_ref().map(|turn| {
                    unsent_saved_drafts
                        .borrow_mut()
                        .entry(turn.id)
                        .or_insert_with(TextEditingController::new)
                        .clone()
                });
                let viewport = cx.viewport();
                let details = current.as_ref().and_then(|turn| {
                    let Workspace::Local(path) = &turn.workspace else {
                        return None;
                    };
                    Some(thread_details::LocalDetails {
                        thread: turn.id,
                        machine: device.clone(),
                        path: path.clone(),
                        canvas_height: (viewport.height - 40.0).max(0.0),
                        inline_open: details_visibility.is_open(turn.id),
                        visibility: details_visibility.clone(),
                    })
                });
                let shortcut_details = details.clone().filter(|detail| {
                    let width = viewport.width - 256.0;
                    thread_details::inline_layout(
                        width,
                        detail.canvas_height,
                        48.0,
                        message_jump.get().is_some(),
                    )
                    .is_some()
                });
                let jump = message_jump
                    .get()
                    .filter(|jump| current.as_ref().is_some_and(|turn| turn.id == jump.thread));
                if let Some(jump) = &jump {
                    jump.reveal(cx, &posted_message_jump);
                }
                let choose_project = thread_projects.clone();
                let choose_step = picker.clone();
                let choose_store = projects.clone();
                let choose_error = picker_error.clone();
                let store = history_store.clone();
                let show_error = history_error.clone();
                #[cfg(test)]
                let saved = selected_thread.clone();
                #[cfg(test)]
                let after_send_history = thread_history.clone();
                #[cfg(test)]
                let after_send_error = history_error.clone();
                #[cfg(test)]
                let after_send_store = history_store.clone();
                let target = project
                    .as_ref()
                    .map_or(Workspace::Scratch, |p| Workspace::Local(p.root.clone()));
                let rename_workspace = target.clone();
                let rename_store = history_store.clone();
                let rename_project = active_project.clone();
                let rename_scratch = scratch_open.clone();
                let rename_error = history_error.clone();
                let rename_history = thread_history.clone();
                let history = thread_history.get();
                let messages = message_lookup.poll(
                    cx,
                    history_store.as_ref(),
                    &target,
                    &history,
                    sidebar_search.observed_query(),
                    message_history_revision.get(),
                );
                #[cfg(test)]
                let message_history_after_send = message_history_revision.clone();
                let find_settings = settings_open.clone();
                let find_picker = picker.clone();
                let shortcut_thread = current.as_ref().map(|turn| turn.id);
                let find_selection = selected_thread.clone();
                let find_jump = message_jump.clone();
                let find_input = thread_find_input.clone();
                let open_find: Rc<dyn Fn()> = Rc::new(move || {
                    let Some(thread) = shortcut_thread else {
                        return;
                    };
                    if find_settings.get()
                        || find_picker.get() != Picker::Closed
                        || find_input.preedit().is_some()
                        || find_jump.get().is_some_and(|open| open.thread == thread)
                    {
                        return;
                    }
                    if let Some(turn) = find_selection.get().filter(|turn| turn.id == thread) {
                        find_input.clear();
                        find_jump.set(Some(message_jump::Jump::new(&turn, String::new(), None)));
                    }
                });
                let menu_find = open_find.clone();
                let panel_settings = settings_open.clone();
                let workspace = workspace::draft(
                    title,
                    project_icon,
                    history,
                    messages,
                    current,
                    jump,
                    message_jump.clone(),
                    thread_find_input.clone(),
                    thread_title_editor.clone(),
                    details,
                    saved_input,
                    move |id, title| {
                        let active_scope = rename_project
                            .get()
                            .map(|p| Workspace::Local(p.root))
                            .or_else(|| rename_scratch.get().then_some(Workspace::Scratch));
                        if active_scope.as_ref() != Some(&rename_workspace) {
                            return;
                        }
                        let result = rename_store
                            .as_ref()
                            .ok_or_else(|| std::io::Error::other("No local profile."))
                            .and_then(|store| {
                                store.rename(id, &rename_workspace, title)?;
                                store.history(&rename_workspace)
                            });
                        match result {
                            Ok(rows) => {
                                rename_error.set(None);
                                rename_history.set(rows);
                            }
                            Err(error) => {
                                rename_error.set(Some(error.to_string()));
                            }
                        }
                    },
                    move || menu_find(),
                    history_error.get(),
                    sidebar_search.clone(),
                    add_project,
                    move |id, seed| {
                        let outcome = (|| {
                            let store = store
                                .as_ref()
                                .ok_or_else(|| std::io::Error::other("No local profile."))?;
                            let turn = store
                                .load(id)?
                                .filter(|turn| turn.workspace == target)
                                .ok_or_else(|| {
                                    std::io::Error::other("Thread is no longer available.")
                                })?;
                            store.workspace_directory(&turn)?;
                            Ok::<_, std::io::Error>(turn)
                        })();
                        match outcome {
                            Ok(turn) => {
                                show_error.set(None);
                                rename_on_open.clear();
                                let jump = seed.and_then(|(hit, query)| {
                                    message_jump::Jump::from_result(&turn, hit, &query)
                                });
                                if let Some(jump) = jump {
                                    input_on_open.set_text(jump.query.clone());
                                    jump_on_open.set(Some(jump));
                                } else if jump_on_open
                                    .get()
                                    .is_some_and(|prior| prior.thread != turn.id)
                                {
                                    jump_on_open.set(None);
                                }
                                selected.set(Some(turn));
                            }
                            Err(error) => {
                                show_error.set(Some(format!("Could not open thread: {error}")));
                            }
                        }
                    },
                    move || {
                        let choices = choose_store
                            .as_ref()
                            .and_then(|store| store.list().ok())
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|project| project.root.is_dir())
                            .collect::<Vec<_>>();
                        if choices.len() > 1 {
                            choose_project.begin(choices);
                            choose_error.set(None);
                            choose_step.set(Picker::ThreadProjects);
                        } else {
                            rename_on_new.clear();
                            jump_on_new.set(None);
                            new_draft.set(None);
                        }
                    },
                    move || {
                        rename_here.clear();
                        here_jump.set(None);
                        here_draft.set(None);
                    },
                    move || {
                        settings_navigation.open_providers();
                        settings_open.set(true);
                    },
                    #[cfg(test)]
                    fixture_enabled.get().then(|| {
                        fixture.composer(
                            project.map(|p| p.root),
                            selected_thread.get(),
                            move |turn| {
                                let refreshed = after_send_store
                                    .as_ref()
                                    .map(|store| store.history(&turn.workspace))
                                    .transpose();
                                after_send_error.set(
                                    refreshed
                                        .as_ref()
                                        .err()
                                        .map(|e| format!("Could not read threads: {e}")),
                                );
                                if let Ok(Some(rows)) = refreshed {
                                    after_send_history.set(rows);
                                }
                                message_history_after_send
                                    .set(message_history_after_send.get().wrapping_add(1));
                                saved.set(Some(turn));
                            },
                        )
                    }),
                );
                if shortcut_thread.is_some() {
                    let modifiers = if cfg!(target_os = "macos") {
                        Modifiers::META
                    } else {
                        Modifiers::CONTROL
                    };
                    let mut shortcuts = CallbackShortcuts::new(workspace).shortcut(
                        SingleActivator::new(KeyboardKey::Character("f".into()), modifiers)
                            .with_repeats(false),
                        move || open_find(),
                    );
                    if let Some(detail) = shortcut_details {
                        let picker = picker.clone();
                        let selection = selected_thread.clone();
                        let toggle = Rc::new(move || {
                            if !panel_settings.get()
                                && picker.get() == Picker::Closed
                                && selection.get().is_some_and(|turn| turn.id == detail.thread)
                            {
                                detail.toggle();
                            }
                        });
                        for character in ["b", "B"] {
                            let toggle = toggle.clone();
                            shortcuts = shortcuts.shortcut(
                                SingleActivator::new(
                                    KeyboardKey::Character(character.into()),
                                    modifiers | Modifiers::SHIFT,
                                )
                                .with_repeats(false),
                                move || toggle(),
                            );
                        }
                    }
                    shortcuts.into()
                } else {
                    workspace
                }
            } else {
                match resolve_landing(&[], true, profile.is_some()) {
                    Landing::Empty {
                        can_start_without_project,
                    } => {
                        let scratch_open = scratch_open.clone();
                        let picker = picker.clone();
                        let path = path.clone();
                        let settings_open = settings_open.clone();
                        let settings_navigation = settings_navigation.clone();
                        empty_workspace::view(
                            can_start_without_project,
                            move || {
                                scratch_open.set(true);
                            },
                            move || {
                                path.clear();
                                picker.set(Picker::Sources);
                            },
                            move || {
                                settings_navigation.open_providers();
                                settings_open.set(true);
                            },
                        )
                    }
                    _ => unreachable!("no projects are loaded in the current local workspace"),
                }
            };
            let active_root = active_project.get().map(|project| project.root.clone());
            let active_project = active_project.clone();
            let step = picker.clone();
            let error = picker_error.clone();
            let projects = projects.clone();
            let history_store = history_store.clone();
            let thread_history = thread_history.clone();
            let history_error = history_error.clone();
            let selected_thread = selected_thread.clone();
            let search_for_project = sidebar_search.clone();
            let jump_for_project = message_jump.clone();
            let rename_for_project = thread_title_editor.clone();
            let icon_for_project = project_icon_revision.clone();
            #[cfg(test)]
            let fixture_for_project = fixture.clone();
            let finish_project = move |result: std::io::Result<(LocalProject, Option<String>)>,
                                       action: ProjectAction| {
                match result {
                    Ok((project, warning)) => {
                        error.set(None);
                        let refreshed = history_store
                            .as_ref()
                            .map(|store| store.history(&Workspace::Local(project.root.clone())))
                            .transpose();
                        history_error.set(
                            refreshed
                                .as_ref()
                                .err()
                                .map(|e| format!("Could not read threads: {e}"))
                                .or_else(|| {
                                    warning.map(|message| {
                                        format!(
                                            "Project created without an initial commit: {message}"
                                        )
                                    })
                                }),
                        );
                        thread_history.set(refreshed.ok().flatten().unwrap_or_default());
                        #[cfg(test)]
                        fixture_for_project.reset();
                        jump_for_project.set(None);
                        rename_for_project.clear();
                        selected_thread.set(None);
                        search_for_project.reset();
                        active_project.set(Some(project));
                        icon_for_project.set(icon_for_project.get().wrapping_add(1));
                        step.set(Picker::Closed);
                    }
                    Err(e) => {
                        let label = match action {
                            ProjectAction::Open => "Could not open folder",
                            ProjectAction::Create => "Could not create project",
                            ProjectAction::Clone => "Could not clone project",
                        };
                        error.set(Some(format!("{label}: {e}")));
                    }
                }
            };
            let creating = project_busy.get();
            if creating && let Some(job) = queued_project.borrow_mut().take() {
                let store = projects.clone();
                let busy = project_busy.clone();
                let finish = finish_project.clone();
                let action = match &job {
                    ProjectWork::Create(_) => ProjectAction::Create,
                    ProjectWork::Clone { .. } => ProjectAction::Clone,
                };
                cx.spawn_blocking(
                    move || match job {
                        ProjectWork::Create(name) => {
                            register_managed_project(store.as_ref(), &name)
                        }
                        ProjectWork::Clone {
                            remote,
                            destination,
                        } => register_cloned_project(
                            store.as_ref(),
                            &remote,
                            destination,
                            clone_transport,
                        ),
                    },
                    move |result, _| {
                        busy.set(false);
                        finish(
                            result.unwrap_or_else(|error| {
                                Err(std::io::Error::other(format!("{error:?}")))
                            }),
                            action,
                        );
                    },
                );
            }
            let pending = project_busy.clone();
            let work = queued_project.clone();
            let clear_error = picker_error.clone();
            let home_for_clone = home.clone();
            let active_for_clone = active_root.clone();
            let desktop = project_picker::overlay(
                workspace,
                picker.clone(),
                path.clone(),
                new_project_name.clone(),
                new_project_preview.clone(),
                remote_flow.clone(),
                thread_projects.clone(),
                &device,
                active_root.as_deref(),
                projects.as_ref().map(|store| store.data_dir().to_owned()),
                creating,
                home.clone(),
                browse.clone(),
                browse_error.clone(),
                picker_error.get(),
                move |request| {
                    if pending.get() {
                        return;
                    }
                    match request {
                        ProjectRequest::Create(name) => {
                            clear_error.set(None);
                            *work.borrow_mut() = Some(ProjectWork::Create(name));
                            pending.set(true);
                        }
                        ProjectRequest::Clone {
                            remote,
                            destination,
                        } => {
                            let result = confirm_clone_destination(
                                projects.as_ref(),
                                &destination,
                                home_for_clone.as_deref(),
                                active_for_clone.as_deref(),
                            );
                            match result {
                                Ok(destination) => {
                                    clear_error.set(None);
                                    *work.borrow_mut() = Some(ProjectWork::Clone {
                                        remote,
                                        destination,
                                    });
                                    pending.set(true);
                                }
                                Err(error) => {
                                    finish_project(Err(error), ProjectAction::Clone);
                                }
                            }
                        }
                        ProjectRequest::StartThread(path) => {
                            let result = projects
                                .as_ref()
                                .ok_or_else(|| {
                                    std::io::Error::other(
                                        "No local profile directory is available.",
                                    )
                                })
                                .and_then(|store| {
                                    if path.canonicalize()? != path
                                        || !store.list()?.iter().any(|saved| saved.root == path)
                                    {
                                        return Err(std::io::Error::new(
                                            std::io::ErrorKind::NotFound,
                                            "The saved project is no longer available.",
                                        ));
                                    }
                                    store.open(&path)
                                })
                                .map(|project| (project, None));
                            finish_project(result, ProjectAction::Open);
                        }
                        ProjectRequest::Open(input) => {
                            let result = projects
                                .as_ref()
                                .ok_or_else(|| {
                                    std::io::Error::other(
                                        "No local profile directory is available.",
                                    )
                                })
                                .and_then(|store| store.open(Path::new(&input)))
                                .map(|project| (project, None));
                            finish_project(result, ProjectAction::Open);
                        }
                    }
                },
            );
            if settings_open.get() {
                let settings_open = settings_open.clone();
                provider_settings::overlay(
                    desktop,
                    &device,
                    settings_navigation.clone(),
                    settings_provider.clone(),
                    provider_names.clone(),
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

fn register_managed_project(
    store: Option<&LocalProjects>,
    name: &str,
) -> std::io::Result<(LocalProject, Option<String>)> {
    let store =
        store.ok_or_else(|| std::io::Error::other("No local profile directory is available."))?;
    let created = managed_projects::create(store.data_dir(), name)?;
    let project = store
        .open_named(&created.root, &created.title)
        .map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "Git folder created at {} but could not be registered: {error}",
                    created.root.display()
                ),
            )
        })?;
    Ok((project, created.warning))
}

fn confirm_clone_destination(
    store: Option<&LocalProjects>,
    input: &str,
    home: Option<&Path>,
    active: Option<&Path>,
) -> std::io::Result<CloneDestination> {
    let store =
        store.ok_or_else(|| std::io::Error::other("No local profile directory is available."))?;
    let target = remote_projects::resolve_destination(input, home, active)?;
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("Choose a project directory."))?;
    let name = target
        .file_name()
        .ok_or_else(|| std::io::Error::other("Choose a project folder name."))?;
    CloneDestination::confirmable(parent, name, store.data_dir())
}

fn register_cloned_project(
    store: Option<&LocalProjects>,
    remote: &RemoteProject,
    destination: CloneDestination,
    clone_transport: CloneTransport,
) -> std::io::Result<(LocalProject, Option<String>)> {
    let store =
        store.ok_or_else(|| std::io::Error::other("No local profile directory is available."))?;
    let root = destination.commit(|workspace| clone_transport(&remote.url, workspace))?;
    let project = store.open(&root).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            "The checkout is on disk, but Butler could not register the project.",
        )
    })?;
    Ok((project, None))
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
        threads::{Conversation, Exchange, ThreadStore, Workspace},
    };
    use std::{cell::RefCell, collections::HashMap, io, rc::Rc};
    use uuid::Uuid;

    #[derive(Clone)]
    pub struct Fixture {
        state: Option<PathBuf>,
        thread: Signal<Uuid>,
        input: TextEditingController,
        error: Signal<Option<String>>,
        follow_ups: Rc<RefCell<HashMap<Uuid, FollowUp>>>,
    }

    #[derive(Clone)]
    struct FollowUp {
        input: TextEditingController,
        error: Signal<Option<String>>,
    }

    impl Fixture {
        pub fn new(state: Option<PathBuf>) -> Self {
            Self {
                state,
                thread: Signal::new(Uuid::new_v4()),
                input: TextEditingController::new(),
                error: Signal::new(None),
                follow_ups: Rc::default(),
            }
        }

        pub fn reset(&self) {
            self.thread.set(Uuid::new_v4());
            self.input.clear();
            self.error.set(None);
        }

        pub fn composer(
            &self,
            project: Option<PathBuf>,
            current: Option<Conversation>,
            on_saved: impl Fn(Conversation) + 'static,
        ) -> workspace::OfflineDraft {
            if let Some(current) = current {
                let follow_up = self
                    .follow_ups
                    .borrow_mut()
                    .entry(current.id)
                    .or_insert_with(|| FollowUp {
                        input: TextEditingController::new(),
                        error: Signal::new(None),
                    })
                    .clone();
                let state = self.state.clone();
                return workspace::OfflineDraft {
                    input: follow_up.input.clone(),
                    error: follow_up.error.get(),
                    send: Rc::new(move |prompt| {
                        if prompt.trim().is_empty() {
                            return;
                        }
                        let result = state
                            .as_ref()
                            .ok_or_else(|| io::Error::other("No local profile."))
                            .and_then(|state| {
                                ThreadStore::new(state).append(
                                    &current,
                                    Exchange {
                                        prompt,
                                        reply: format!(
                                            "Offline follow-up {} received.",
                                            current.follow_ups.len() + 1
                                        ),
                                    },
                                )
                            });
                        match result {
                            Ok(turn) => {
                                follow_up.error.set(None);
                                follow_up.input.clear();
                                on_saved(turn);
                            }
                            Err(problem) => {
                                follow_up.error.set(Some(problem.to_string()));
                            }
                        }
                    }),
                };
            }
            let fixture = self.clone();
            let state = self.state.clone();
            let thread = self.thread.get();
            workspace::OfflineDraft {
                input: self.input.clone(),
                error: self.error.get(),
                send: Rc::new(move |prompt| {
                    if prompt.trim().is_empty() || fixture.thread.get() != thread {
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
                        let turn = Conversation {
                            id: thread,
                            workspace: target,
                            prompt,
                            reply: "Offline test reply received.".into(),
                            follow_ups: vec![],
                        };
                        ThreadStore::new(state).save(&turn)?;
                        Ok::<_, io::Error>(turn)
                    })();
                    match result {
                        Ok(turn) => {
                            fixture.reset();
                            on_saved(turn);
                        }
                        Err(problem) => {
                            fixture.error.set(Some(problem.to_string()));
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
            app.process_runtime_work_at(std::time::Instant::now());
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
    fn creates_a_managed_project_and_reopens_its_original_title() {
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        Profile::new(&state).complete_onboarding().unwrap();
        let mut app =
            application(Some(state.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Add project").unwrap();
            sim.click("New project").unwrap();
            sim.focus("Project name").unwrap();
            sim.type_text("Café Build").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            for _ in 0..100 {
                if sim.focus("Open provider settings").is_ok() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("created project did not activate");
        });
        let projects = LocalProjects::new(&state).list().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].title(), "Café Build");
        assert_eq!(projects[0].root.file_name().unwrap(), "cafe-build");
        assert!(projects[0].root.join(".git").is_dir());
        assert!(projects[0].root.join("assets/icon.svg").is_file());
        assert!(
            fs::read_to_string(projects[0].root.join("README.md"))
                .unwrap()
                .contains("# Café Build")
        );
        drop(app);
        let mut reopened = application(Some(state), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut reopened, |sim| {
            sim.focus("Open provider settings").unwrap();
        });
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn git_url_requires_second_confirmation_and_registers_only_completed_checkout() {
        fn offline_checkout(url: &str, workspace: &CloneWorkspace) -> std::io::Result<()> {
            assert_eq!(url, "https://github.com/ButlerFixture/CaseRepo.git");
            let checkout = workspace.path();
            fs::create_dir(checkout.join(".git"))?;
            fs::write(checkout.join(".git/HEAD"), "ref: refs/heads/main\n")?;
            fs::write(checkout.join("README.md"), "Offline checkout fixture\n")
        }
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        Profile::new(&state).complete_onboarding().unwrap();
        let selected_parent = tmp.0.join("Choose here");
        fs::create_dir(&selected_parent).unwrap();
        let repo = selected_parent.join("CaseRepo");
        let repo_before_confirm = repo.clone();
        let state_before_confirm = state.clone();
        let mut app = application_with_transport(
            Some(state.clone()),
            Some(tmp.0.clone()),
            "Fixture".into(),
            offline_checkout,
        )
        .unwrap();
        interact(&mut app, move |sim| {
            sim.click("Add project").unwrap();
            sim.click("Git URL").unwrap();
            sim.focus("Enter Git clone URL").unwrap();
            sim.type_text("ButlerFixture/CaseRepo").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Enter path (e.g. ~/projects/my-app)").unwrap();
            sim.click("Choose here").unwrap();
            assert!(!repo_before_confirm.exists());
            assert!(
                LocalProjects::new(&state_before_confirm)
                    .list()
                    .unwrap()
                    .is_empty()
            );
            sim.click("Create & Clone").unwrap();
            for _ in 0..100 {
                if sim.focus("Open provider settings").is_ok() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("checkout did not activate");
        });
        let projects = LocalProjects::new(&state).list().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].title(), "CaseRepo");
        assert_eq!(projects[0].root, repo.canonicalize().unwrap());
        assert_eq!(
            fs::read_to_string(repo.join("README.md")).unwrap(),
            "Offline checkout fixture\n"
        );
        assert!(repo.join(".git/HEAD").is_file());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn failed_git_url_checkout_leaves_destination_and_registry_untouched() {
        fn offline_failure(_: &str, workspace: &CloneWorkspace) -> std::io::Result<()> {
            fs::write(workspace.path().join("partial"), b"incomplete")?;
            Err(std::io::Error::other("Offline fixture checkout failed."))
        }
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        Profile::new(&state).complete_onboarding().unwrap();
        let mut app = application_with_transport(
            Some(state.clone()),
            Some(tmp.0.clone()),
            "Fixture".into(),
            offline_failure,
        )
        .unwrap();
        interact(&mut app, |sim| {
            sim.click("Add project").unwrap();
            sim.click("Git URL").unwrap();
            sim.focus("Enter Git clone URL").unwrap();
            sim.type_text("ButlerFixture/Failure").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("Create & Clone").unwrap();
            for _ in 0..100 {
                if sim
                    .click("Could not clone project: Offline fixture checkout failed.")
                    .is_ok()
                {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("checkout failure was not reported");
        });
        assert!(!tmp.0.join("Failure").exists());
        assert!(LocalProjects::new(state).list().unwrap().is_empty());
        assert!(fs::read_dir(&tmp.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".butler-clone-")
        }));
    }

    #[test]
    fn new_project_keyboard_back_requires_empty_name_and_escape_closes_picker() {
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        Profile::new(&state).complete_onboarding().unwrap();
        let mut app = application(Some(state), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Add project").unwrap();
            sim.click("New project").unwrap();
            sim.focus("Project name").unwrap();
            sim.type_text("A").unwrap();
            sim.press("Backspace".parse().unwrap()).unwrap();
            sim.focus("Project name").unwrap();
            sim.press("Backspace".parse().unwrap()).unwrap();
            sim.focus("New project").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Project name").unwrap();
            sim.press("Escape".parse().unwrap()).unwrap();
            assert!(sim.focus("Project name").is_err());
            sim.focus("Add project").unwrap();
        });
    }

    #[test]
    fn sidebar_search_filters_persisted_titles_and_wraps_keyboard_selection() {
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        let project = tmp.0.join("Project");
        fs::create_dir(&project).unwrap();
        let project = LocalProjects::new(&state).open(&project).unwrap();
        Profile::new(&state).complete_onboarding().unwrap();
        let store = ThreadStore::new(&state);
        let workspace = Workspace::Local(project.root);
        for title in ["Logger alpha", "Logger beta", "Other"] {
            store
                .save(&Conversation {
                    id: uuid::Uuid::new_v4(),
                    workspace: workspace.clone(),
                    prompt: title.into(),
                    reply: format!("{title} reply"),
                    follow_ups: Vec::new(),
                })
                .unwrap();
        }
        let first = store
            .history(&workspace)
            .unwrap()
            .into_iter()
            .find(|thread| thread.title.starts_with("Logger"))
            .unwrap()
            .title;
        let expected = format!("{first} reply");
        let mut app = application(Some(state), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, move |sim| {
            sim.focus("Search threads").unwrap();
            sim.type_text("absent").unwrap();
            for _ in 0..70 {
                if sim.click("No threads found").is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            sim.click("No threads found").unwrap();
            sim.focus("Search threads").unwrap();
            sim.press("Escape".parse().unwrap()).unwrap();
            sim.focus("Open thread: Other").unwrap();
            sim.focus("Search threads").unwrap();
            sim.type_text("LOGGER").unwrap();
            assert!(sim.focus("Open thread: Other").is_err());
            sim.press("ArrowDown".parse().unwrap()).unwrap();
            sim.press("ArrowDown".parse().unwrap()).unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click(expected).unwrap();
            sim.focus("Open thread: Other").unwrap();
        });
    }

    #[test]
    fn sidebar_search_finds_body_only_in_the_active_workspace() {
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        let project = tmp.0.join("Current");
        let other = tmp.0.join("Foreign");
        for path in [&project, &other] {
            fs::create_dir(path).unwrap();
        }
        let project = LocalProjects::new(&state).open(&project).unwrap();
        Profile::new(&state).complete_onboarding().unwrap();
        let store = ThreadStore::new(&state);
        for (root, title, reply) in [
            (
                project.root.clone(),
                "Saved answer",
                "The graphite ledger shows a balance.",
            ),
            (
                project.root,
                "Graphite ledger outline",
                "Graphite ledger notes",
            ),
            (
                other.canonicalize().unwrap(),
                "Foreign thread",
                "Graphite ledger is private there.",
            ),
        ] {
            store
                .save(&Conversation {
                    id: uuid::Uuid::new_v4(),
                    workspace: Workspace::Local(root),
                    prompt: title.into(),
                    reply: reply.into(),
                    follow_ups: Vec::new(),
                })
                .unwrap();
        }
        let mut app = application(Some(state), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.focus("Search threads").unwrap();
            sim.type_text("graphite ledger").unwrap();
            sim.focus("Open thread: Graphite ledger outline").unwrap();
            for _ in 0..70 {
                if sim.focus("Open thread: Saved answer").is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            sim.focus("Open thread: Saved answer").unwrap();
            assert!(sim.focus("Open thread: Foreign thread").is_err());
            sim.focus("Search threads").unwrap();
            sim.press("ArrowDown".parse().unwrap()).unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("The graphite ledger shows a balance.").unwrap();
            sim.focus("Close thread find").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            assert!(sim.focus("Close thread find").is_err());
        });
    }

    #[test]
    fn sidebar_message_hit_reveals_and_marks_a_follow_up_below_the_viewport() {
        use butler::threads::Exchange;
        use incular::rendering::PaintCommand;

        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        Profile::new(&profile).complete_onboarding().unwrap();
        let project_path = tmp.0.join("Test project");
        fs::create_dir(&project_path).unwrap();
        let project = LocalProjects::new(&profile).open(&project_path).unwrap();
        let store = ThreadStore::new(&profile);
        let follow_ups = (0..40)
            .map(|index| Exchange {
                prompt: format!("Follow-up {index}"),
                reply: if index == 36 {
                    "The Café lantern is lit.".into()
                } else {
                    format!("Earlier or later reply {index}")
                },
            })
            .collect();
        for (title, follow_ups) in [
            ("Saved history", follow_ups),
            ("Unrelated history", Vec::new()),
        ] {
            let mut turn = Conversation {
                id: uuid::Uuid::new_v4(),
                workspace: Workspace::Local(project.root.clone()),
                prompt: title.into(),
                reply: "Initial reply".into(),
                follow_ups: Vec::new(),
            };
            store.save(&turn).unwrap();
            for exchange in follow_ups {
                turn = store.append(&turn, exchange).unwrap();
            }
        }
        let mut app = application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.focus("Search threads").unwrap();
            sim.type_text("cafe lantern").unwrap();
            for _ in 0..70 {
                if sim.focus("Open thread: Saved history").is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            sim.click("Open thread: Saved history").unwrap();
            sim.focus("Close thread find").unwrap();
        });
        let window = app.active_window_ids()[0];
        let size = Size::new(1280.0, 800.0);
        let mut visible_marker = false;
        for _ in 0..5 {
            app.process_runtime_work();
            let (list, _) = app
                .run_window_frame_at(window, Constraints::tight(size), std::time::Instant::now())
                .unwrap()
                .unwrap();
            let mut transforms = vec![incular::core::Transform::IDENTITY];
            for command in list.commands() {
                match command {
                    PaintCommand::PushTransform { transform } => {
                        transforms.push(transforms.last().unwrap().then(*transform));
                    }
                    PaintCommand::PopTransform => {
                        transforms.pop();
                    }
                    PaintCommand::GlyphRun { run, color }
                        if *color == Color::rgba(33, 24, 10, 255) =>
                    {
                        // The target occurrence's own glyphs, not its query label or a preceding reply.
                        if let Some(first) = run.glyphs.first() {
                            let origin = transforms
                                .last()
                                .unwrap()
                                .transform_point(run.origin + first.offset);
                            visible_marker |= origin.x > 255.0 && (40.0..800.0).contains(&origin.y);
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(
            visible_marker,
            "the measured transcript must display the marked follow-up"
        );
        interact(&mut app, |sim| {
            sim.click("Open thread: Unrelated history").unwrap();
            assert!(sim.focus("Close thread find").is_err());
        });
    }

    #[test]
    fn sidebar_f2_renames_another_thread_without_opening_it_and_ignores_search_rows() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let folder = tmp.0.join("Project");
        fs::create_dir(&folder).unwrap();
        let project = LocalProjects::new(&profile).open(&folder).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let store = ThreadStore::new(&profile);
        for (prompt, reply) in [
            ("Keep me open", "The active transcript remains here."),
            ("Change this title", "This is not the active transcript."),
        ] {
            store
                .save(&Conversation {
                    id: uuid::Uuid::new_v4(),
                    workspace: Workspace::Local(project.root.clone()),
                    prompt: prompt.into(),
                    reply: reply.into(),
                    follow_ups: Vec::new(),
                })
                .unwrap();
        }
        let mut app = application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Open thread: Keep me open").unwrap();
            sim.focus("Open thread: Change this title").unwrap();
            sim.press_key(KeyboardKey::Named(NamedKey::F2), Code::F2)
                .unwrap();
            sim.focus("Thread title").unwrap();
            sim.type_text("Clean title").unwrap();
            sim.press(Code::Enter).unwrap();
            sim.focus("Open thread: Clean title").unwrap();
            sim.press_key(KeyboardKey::Named(NamedKey::F2), Code::F2)
                .unwrap();
            sim.focus("Thread title").unwrap();
            sim.type_text("Saved on blur").unwrap();
            sim.focus("Search threads").unwrap();
            sim.focus("Open thread: Saved on blur").unwrap();
            sim.focus("Thread actions for Keep me open").unwrap();
            sim.type_into("Search threads", "saved on blur").unwrap();
            sim.focus("Open thread: Saved on blur").unwrap();
            sim.press_key(KeyboardKey::Named(NamedKey::F2), Code::F2)
                .unwrap();
            assert!(sim.focus("Thread title").is_err());
            sim.focus("The active transcript remains here.").unwrap();
        });
        let titles = store
            .history(&Workspace::Local(project.root))
            .unwrap()
            .into_iter()
            .map(|row| row.title)
            .collect::<Vec<_>>();
        assert!(titles.iter().any(|title| title == "Saved on blur"));
        assert!(titles.iter().any(|title| title == "Keep me open"));
    }

    #[test]
    fn saved_thread_rename_updates_search_and_survives_restart_without_editing_messages() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let folder = tmp.0.join("Project");
        fs::create_dir(&folder).unwrap();
        let project = LocalProjects::new(&profile).open(&folder).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let store = ThreadStore::new(&profile);
        let original = Conversation {
            id: uuid::Uuid::new_v4(),
            workspace: Workspace::Local(project.root),
            prompt: "Original saved question".into(),
            reply: "The saved response is unchanged.".into(),
            follow_ups: Vec::new(),
        };
        store.save(&original).unwrap();
        let mut app =
            application(Some(profile.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            let modifier = if cfg!(target_os = "macos") {
                Code::MetaLeft
            } else {
                Code::ControlLeft
            };
            let replace = |value: &str| {
                sim.focus("Thread title").unwrap();
                sim.key_down(modifier).unwrap();
                sim.press_key(KeyboardKey::Character("a".into()), Code::KeyA)
                    .unwrap();
                sim.key_up(modifier).unwrap();
                sim.type_text(value).unwrap();
            };
            sim.click("Open thread: Original saved question").unwrap();
            sim.click("Thread actions for Original saved question")
                .unwrap();
            sim.click("Rename thread").unwrap();
            replace("Do not save");
            sim.press(Code::Escape).unwrap();
            sim.click("Thread actions for Original saved question")
                .unwrap();
            sim.click("Rename thread").unwrap();
            replace("   ");
            sim.press(Code::Enter).unwrap();
            sim.focus("Thread title cannot be empty").unwrap();
            sim.click("Thread actions for Original saved question")
                .unwrap();
            sim.click("Rename thread").unwrap();
            replace("  Café notes  ");
            sim.press(Code::Enter).unwrap();
            sim.focus("Thread actions for Café notes").unwrap();
            sim.focus("The saved response is unchanged.").unwrap();
            sim.type_into("Search threads", "cafe notes").unwrap();
            sim.focus("Open thread: Café notes").unwrap();
        });
        assert_eq!(store.load(original.id).unwrap().unwrap(), original);
        drop(app);
        let mut reopened =
            application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut reopened, |sim| {
            sim.click("Open thread: Café notes").unwrap();
            sim.focus("Thread actions for Café notes").unwrap();
            sim.focus("Original saved question").unwrap();
        });
    }

    #[test]
    fn thread_find_edits_counts_steps_and_wraps_from_keyboard_or_buttons() {
        use butler::threads::Exchange;
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let folder = tmp.0.join("Project");
        fs::create_dir(&folder).unwrap();
        let project = LocalProjects::new(&profile).open(&folder).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let store = ThreadStore::new(&profile);
        let turn = Conversation {
            id: uuid::Uuid::new_v4(),
            workspace: Workspace::Local(project.root),
            prompt: "Needle first. needle twice.".into(),
            reply: "Café first.".into(),
            follow_ups: Vec::new(),
        };
        store.save(&turn).unwrap();
        store
            .append(
                &turn,
                Exchange {
                    prompt: "Needle last.".into(),
                    reply: "cafe only".into(),
                },
            )
            .unwrap();
        let mut app = application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            let modifier = if cfg!(target_os = "macos") {
                Code::MetaLeft
            } else {
                Code::ControlLeft
            };
            sim.click("Open thread: Needle first. needle twice.")
                .unwrap();
            sim.focus("Open thread: Needle first. needle twice.")
                .unwrap();
            assert!(sim.focus("Find in thread").is_err());
            sim.key_down(modifier).unwrap();
            sim.press_key(KeyboardKey::Character("f".into()), Code::KeyF)
                .unwrap();
            sim.key_up(modifier).unwrap();
            sim.type_into("Find in thread", " needle ").unwrap();
            sim.focus("1/3").unwrap();
            sim.focus("Find in thread").unwrap();
            sim.press(Code::Enter).unwrap();
            sim.press(Code::Enter).unwrap();
            sim.focus("3/3").unwrap();
            sim.click("Previous match").unwrap();
            sim.focus("2/3").unwrap();
            sim.click("Next match").unwrap();
            sim.click("Next match").unwrap();
            sim.focus("1/3").unwrap();
            sim.focus("Find in thread").unwrap();
            sim.key_down(Code::ShiftLeft).unwrap();
            sim.press(Code::Enter).unwrap();
            sim.key_up(Code::ShiftLeft).unwrap();
            sim.focus("3/3").unwrap();
            sim.focus("Find in thread").unwrap();
            sim.key_down(modifier).unwrap();
            sim.press_key(KeyboardKey::Character("a".into()), Code::KeyA)
                .unwrap();
            sim.key_up(modifier).unwrap();
            sim.type_text("absent").unwrap();
            sim.focus("0/0").unwrap();
            sim.focus("Find in thread").unwrap();
            sim.press(Code::Escape).unwrap();
            assert!(sim.focus("Find in thread").is_err());
            sim.click("Thread actions for Needle first. needle twice.")
                .unwrap();
            sim.click("Find in thread").unwrap();
            sim.focus("Close thread find").unwrap();
            sim.type_into("Find in thread", "Café").unwrap();
            sim.focus("1/1").unwrap();
            sim.focus("Find in thread").unwrap();
            sim.press(Code::Escape).unwrap();
            assert!(sim.focus("Close thread find").is_err());
            sim.click("Thread actions for Needle first. needle twice.")
                .unwrap();
            sim.click("Find in thread").unwrap();
            sim.focus("Close thread find").unwrap();
        });
        // Inspect actual painted bounds, not the width requested of the lane.
        // The saved user bubble must never sit behind the Find bar.
        use incular::core::Transform;
        use incular::rendering::{Brush, PaintCommand};
        let window = app.active_window_ids()[0];
        for width in [1280.0, 920.0] {
            app.process_runtime_work();
            let (display, _) = app
                .run_window_frame_at(
                    window,
                    Constraints::tight(Size::new(width, 800.0)),
                    std::time::Instant::now(),
                )
                .unwrap()
                .unwrap();
            let mut transforms = vec![Transform::IDENTITY];
            let mut bubble = None;
            let mut bar = None;
            for command in display.commands() {
                match command {
                    PaintCommand::PushTransform { transform } => {
                        transforms.push(transforms.last().unwrap().then(*transform));
                    }
                    PaintCommand::PopTransform => {
                        transforms.pop();
                    }
                    PaintCommand::RRect {
                        rrect,
                        brush: Brush::Solid(color),
                    } => {
                        let top_left = transforms
                            .last()
                            .unwrap()
                            .transform_point(rrect.rect.origin);
                        if *color == Color::rgba(235, 235, 237, 255) && bubble.is_none() {
                            bubble = Some((top_left, rrect.rect.size));
                        }
                        if *color == Color::rgba(255, 255, 255, 213) {
                            bar = Some((top_left, rrect.rect.size));
                        }
                    }
                    _ => {}
                }
            }
            let (message, message_size) = bubble.expect("saved user bubble painted");
            let (find, find_size) = bar.expect("Find bar painted");
            if width == 1280.0 {
                assert!((message.x + message_size.width - 956.0).abs() < 1.0);
                assert!(message.x + message_size.width <= find.x);
            } else {
                assert!(message.y >= find.y + find_size.height);
            }
        }
    }

    #[test]
    fn new_thread_chooser_filters_projects_and_only_switches_when_chosen() {
        let tmp = Fixture::new();
        let state = tmp.0.join("profile");
        let first = tmp.0.join("Project Alpha");
        let second = tmp.0.join("Project Beta");
        let missing = tmp.0.join("Project Removed");
        for path in [&first, &second, &missing] {
            fs::create_dir(path).unwrap();
        }
        Profile::new(&state).complete_onboarding().unwrap();
        let store = LocalProjects::new(&state);
        store.open(&missing).unwrap();
        let alpha = store.open(&first).unwrap();
        let beta = store.open(&second).unwrap();
        let history = ThreadStore::new(&state);
        for (project, subject) in [(alpha, "Alpha memory"), (beta, "Beta memory")] {
            history
                .save(&Conversation {
                    id: uuid::Uuid::new_v4(),
                    workspace: Workspace::Local(project.root),
                    prompt: subject.into(),
                    reply: format!("{subject} reply"),
                    follow_ups: Vec::new(),
                })
                .unwrap();
        }
        fs::remove_dir(&missing).unwrap();
        let before = store.clone();
        let mut app = application(Some(state), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, move |sim| {
            sim.click("Open thread: Beta memory").unwrap();
            sim.focus("New thread in Project Beta").unwrap();
            sim.press(Code::Enter).unwrap();
            sim.focus("Open provider settings").unwrap();
            assert_eq!(before.list().unwrap()[0].title(), "Project Beta");
            sim.click("Open thread: Beta memory").unwrap();
            sim.click("New thread").unwrap();
            sim.focus("New thread in...").unwrap();
            sim.type_text("ALPHA").unwrap();
            sim.focus("New thread in Project Alpha").unwrap();
            assert!(sim.focus("New thread in Project Removed").is_err());
            assert_eq!(before.list().unwrap()[0].title(), "Project Beta");
            sim.focus("New thread in...").unwrap();
            sim.press("Escape".parse().unwrap()).unwrap();
            assert_eq!(before.list().unwrap()[0].title(), "Project Beta");
            sim.click("Beta memory reply").unwrap();
            sim.click("New thread").unwrap();
            sim.focus("New thread in...").unwrap();
            sim.type_text("alpha").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Open provider settings").unwrap();
            sim.focus("Open thread: Alpha memory").unwrap();
            assert!(sim.focus("Open thread: Beta memory").is_err());
        });
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(LocalProject::title)
                .collect::<Vec<_>>(),
            ["Project Alpha", "Project Beta", "Project Removed"]
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn active_project_uses_local_icon_in_sidebar_and_breadcrumb_then_clears_it() {
        use incular::rendering::PaintCommand;
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let with = tmp.0.join("With icon");
        let without = tmp.0.join("Without icon");
        fs::create_dir(&with).unwrap();
        fs::create_dir(&without).unwrap();
        let mut source = Vec::new();
        let pixel = [21, 88, 177, 255];
        {
            let mut encoder = png::Encoder::new(&mut source, 4, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixel.repeat(8))
                .unwrap();
        }
        fs::write(with.join("favicon.png"), &source).unwrap();
        let store = LocalProjects::new(&profile);
        store.open(&without).unwrap();
        store.open(&with).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let mut app = application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        let window = app.active_window_ids()[0];
        let frame = |app: &mut Application| {
            app.process_runtime_work();
            let (list, _) = app
                .run_window_frame_at(
                    window,
                    Constraints::tight(Size::new(1280.0, 800.0)),
                    std::time::Instant::now(),
                )
                .unwrap()
                .unwrap();
            let mut transforms = vec![incular::core::Transform::IDENTITY];
            let mut draws = Vec::new();
            for command in list.commands() {
                match command {
                    PaintCommand::PushTransform { transform } => {
                        transforms.push(transforms.last().unwrap().then(*transform));
                    }
                    PaintCommand::PopTransform => {
                        transforms.pop();
                    }
                    PaintCommand::Image {
                        image, destination, ..
                    } if image.decoded().pixels().as_ref() == pixel.repeat(8).as_slice() => {
                        draws.push((
                            destination.size,
                            transforms
                                .last()
                                .unwrap()
                                .transform_point(destination.origin),
                        ));
                    }
                    _ => {}
                }
            }
            draws
        };
        let mut draws = Vec::new();
        for _ in 0..80 {
            draws = frame(&mut app);
            if draws.len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            draws.iter().any(|(size, _)| *size == Size::new(16.0, 8.0)),
            "sidebar preserves 2:1 icon: {draws:?}"
        );
        assert!(
            draws
                .iter()
                .any(|(size, origin)| *size == Size::new(14.0, 7.0)
                    && (270.0..310.0).contains(&origin.x)),
            "breadcrumb uses a 2:1 icon near the leading edge: {draws:?}"
        );
        interact(&mut app, |sim| {
            sim.click("New thread").unwrap();
            sim.type_into("New thread in...", "Without icon").unwrap();
            sim.press(Code::Enter).unwrap();
            sim.focus("New thread in Without icon").unwrap();
        });
        assert!(
            frame(&mut app).is_empty(),
            "switching project must not show the old icon"
        );
        assert_eq!(fs::read(with.join("favicon.png")).unwrap(), source);
        assert_eq!(fs::read_dir(without).unwrap().count(), 0);
    }

    #[test]
    fn saved_composer_keeps_unsent_text_in_its_own_thread() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let folder = tmp.0.join("Project");
        fs::create_dir(&folder).unwrap();
        let project = LocalProjects::new(&profile).open(&folder).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let store = ThreadStore::new(&profile);
        let make = |title: &str| Conversation {
            id: uuid::Uuid::new_v4(),
            workspace: Workspace::Local(project.root.clone()),
            prompt: title.into(),
            reply: "A saved reply.".into(),
            follow_ups: Vec::new(),
        };
        let first = make("First thread");
        let second = make("Second thread");
        store.save(&first).unwrap();
        store.save(&second).unwrap();
        let mut app =
            application(Some(profile), Some(tmp.0.clone()), "Local fixture".into()).unwrap();

        let read_editor = |app: &mut Application, label: &'static str| {
            let observed = std::sync::Arc::new(std::sync::Mutex::new(None));
            let output = observed.clone();
            app.dispatch(move |runtime| {
                let semantics = runtime.tree().semantics();
                let mut pending = semantics.roots().to_vec();
                while let Some(id) = pending.pop() {
                    let Some(node) = semantics.node(id) else {
                        continue;
                    };
                    if node.label.as_deref() == Some(label) {
                        *output.lock().unwrap() = Some(node.value.clone().unwrap_or_default());
                        return;
                    }
                    pending.extend(node.children.iter().copied());
                }
            });
            app.process_runtime_work();
            observed.lock().unwrap().take().expect("saved editor")
        };
        interact(&mut app, |sim| {
            sim.click("Open thread: First thread").unwrap();
            sim.type_into("Message First thread", "Unsent first")
                .unwrap();
            sim.press(Code::Enter).unwrap();
            sim.type_text("second line").unwrap();
            sim.click("Open thread: Second thread").unwrap();
            sim.focus("Message Second thread").unwrap();
        });
        assert_eq!(read_editor(&mut app, "Message Second thread"), "");
        interact(&mut app, |sim| {
            sim.type_into("Message Second thread", "Separate draft")
                .unwrap();
            sim.click("Open thread: First thread").unwrap();
            sim.focus("Message First thread").unwrap();
        });
        assert_eq!(
            read_editor(&mut app, "Message First thread"),
            "Unsent first\nsecond line"
        );
        assert_eq!(store.load(first.id).unwrap(), Some(first));
        assert_eq!(store.load(second.id).unwrap(), Some(second));
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
    }

    #[test]
    fn local_thread_details_move_below_find_and_persist_per_thread() {
        use incular::rendering::PaintCommand;

        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        let folder = tmp.0.join("Project in use");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("untouched.txt"), b"keep me").unwrap();
        let project = LocalProjects::new(&profile).open(&folder).unwrap();
        Profile::new(&profile).complete_onboarding().unwrap();
        let store = ThreadStore::new(&profile);
        for prompt in ["First saved discussion", "Second saved discussion"] {
            store
                .save(&Conversation {
                    id: uuid::Uuid::new_v4(),
                    workspace: Workspace::Local(project.root.clone()),
                    prompt: prompt.into(),
                    reply: "An ordinary saved reply.".into(),
                    follow_ups: Vec::new(),
                })
                .unwrap();
        }
        let open_profile = || {
            application(
                Some(profile.clone()),
                Some(tmp.0.clone()),
                "Local fixture".into(),
            )
            .unwrap()
        };
        let mut app = open_profile();
        let card = |app: &mut Application| {
            let window = app.active_window_ids()[0];
            app.process_runtime_work();
            let (list, _) = app
                .run_window_frame_at(
                    window,
                    Constraints::tight(Size::new(1280.0, 800.0)),
                    std::time::Instant::now(),
                )
                .unwrap()
                .unwrap();
            let mut transforms = vec![AffineTransform::IDENTITY];
            for command in list.commands() {
                match command {
                    PaintCommand::PushTransform { transform } => {
                        transforms.push(transform.then(*transforms.last().unwrap()));
                    }
                    PaintCommand::PopTransform => {
                        transforms.pop();
                    }
                    PaintCommand::RRect {
                        rrect,
                        brush: Brush::Solid(color),
                    } if *color == Color::rgba(255, 255, 255, 213)
                        && rrect.rect.size.width == 280.0
                        && rrect.rect.size.height >= 50.0 =>
                    {
                        assert_eq!(rrect.radii.top_left, 16.0);
                        return Some(
                            transforms
                                .last()
                                .unwrap()
                                .transform_point(rrect.rect.origin),
                        );
                    }
                    _ => {}
                }
            }
            None
        };
        interact(&mut app, |sim| {
            sim.click("Open thread: First saved discussion").unwrap();
            sim.focus(thread_details::TOGGLE).unwrap();
        });
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 52.0)));
        let details_semantics = std::sync::Arc::new(std::sync::Mutex::new((false, false)));
        let observed = details_semantics.clone();
        let expected_path = project.root.to_string_lossy().into_owned();
        app.dispatch(move |runtime| {
            let semantics = runtime.tree().semantics();
            let mut pending = semantics.roots().to_vec();
            while let Some(id) = pending.pop() {
                let Some(node) = semantics.node(id) else {
                    continue;
                };
                let mut observed = observed.lock().unwrap();
                if node.label.as_deref() == Some("Run context: Local fixture, Local workspace") {
                    observed.0 = node.state.tooltip.as_deref() == Some(expected_path.as_str());
                }
                if node.label.as_deref() == Some(thread_details::TOGGLE) {
                    observed.1 = node.state.toggled == Some(true);
                }
                pending.extend(node.children.iter().copied());
            }
        });
        app.process_runtime_work();
        assert_eq!(*details_semantics.lock().unwrap(), (true, true));

        interact(&mut app, |sim| {
            sim.click("Thread actions for First saved discussion")
                .unwrap();
            sim.click("Find in thread").unwrap();
            sim.focus("Find in thread").unwrap();
        });
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 100.0)));
        interact(&mut app, |sim| {
            sim.click("Close thread find").unwrap();
            sim.focus(thread_details::TOGGLE).unwrap();
            let modi = if cfg!(target_os = "macos") {
                Code::MetaLeft
            } else {
                Code::ControlLeft
            };
            sim.key_down(modi).unwrap();
            sim.key_down(Code::ShiftLeft).unwrap();
            sim.press_key(KeyboardKey::Character("B".into()), Code::KeyB)
                .unwrap();
            sim.key_up(Code::ShiftLeft).unwrap();
            sim.key_up(modi).unwrap();
        });
        assert_eq!(card(&mut app), None);
        interact(&mut app, |sim| {
            sim.click("Open thread: Second saved discussion").unwrap()
        });
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 52.0)));
        interact(&mut app, |sim| {
            sim.click("Open thread: First saved discussion").unwrap()
        });
        assert_eq!(card(&mut app), None);
        drop(app);
        let mut app = open_profile();
        interact(&mut app, |sim| {
            sim.click("Open thread: Second saved discussion").unwrap();
            sim.focus("Thread actions for Second saved discussion")
                .unwrap();
        });
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 52.0)));
        interact(&mut app, |sim| {
            sim.click("Open thread: First saved discussion").unwrap();
            sim.focus("Thread actions for First saved discussion")
                .unwrap();
        });
        assert_eq!(card(&mut app), None);
        interact(&mut app, |sim| sim.click(thread_details::TOGGLE).unwrap());
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 52.0)));
        drop(app);
        let mut app = open_profile();
        interact(&mut app, |sim| {
            sim.click("Open thread: First saved discussion").unwrap();
            sim.focus("Thread actions for First saved discussion")
                .unwrap();
        });
        assert_eq!(card(&mut app), Some(Offset::new(988.0, 52.0)));
        assert_eq!(fs::read(folder.join("untouched.txt")).unwrap(), b"keep me");
        assert_eq!(fs::read_dir(folder).unwrap().count(), 1);
    }

    #[test]
    fn active_project_sidebar_can_open_a_second_folder_without_touching_the_first() {
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        let first = tmp.0.join("First Project");
        let second = tmp.0.join("Second Project");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        Profile::new(&root).complete_onboarding().unwrap();
        LocalProjects::new(&root).open(&first).unwrap();
        let second_path = second.to_string_lossy().into_owned();
        let mut app =
            application(Some(root.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, move |sim| {
            sim.focus("Add project").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("Local folder").unwrap();
            sim.focus("Absolute folder path").unwrap();
            sim.type_text(second_path).unwrap();
            sim.click("Open folder").unwrap();
            sim.focus("Add project").unwrap();
        });
        let list = LocalProjects::new(root).list().unwrap();
        assert_eq!(
            list.iter().map(LocalProject::title).collect::<Vec<_>>(),
            ["Second Project", "First Project"]
        );
        assert!(fs::read_dir(first).unwrap().next().is_none());
        assert!(fs::read_dir(second).unwrap().next().is_none());
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
            sim.click("Add project").expect("open source picker");
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
                sim.click("Start without a project").is_err(),
                "reopened application must not show the empty workspace"
            );
        });
    }
    #[test]
    fn opens_settings_from_the_empty_workspace_footer() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        Profile::new(&profile).complete_onboarding().unwrap();
        let mut app = application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.focus("Settings").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Enable Cursor").unwrap();
            sim.click("Back to workspace").unwrap();
            sim.focus("Start without a project").unwrap();
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
            sim.click("Add project").unwrap();
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
    fn saves_provider_display_name_locally_and_reopens_it() {
        use butler::profile::Provider;
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        let profile = Profile::new(&root);
        profile.complete_onboarding().unwrap();
        let mut app =
            application(Some(root.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.focus("Open provider settings").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Provider display name").unwrap();
            sim.key_down("ControlLeft".parse().unwrap()).unwrap();
            sim.press("KeyA".parse().unwrap()).unwrap();
            sim.key_up("ControlLeft".parse().unwrap()).unwrap();
            sim.type_text("Cursor Local").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("Select Cursor Local").unwrap();
        });
        assert_eq!(
            profile.provider_name(Provider::Cursor).unwrap(),
            "Cursor Local"
        );
        assert_eq!(profile.provider_name(Provider::Claude).unwrap(), "Claude");
        drop(app);
        let mut app = application(Some(root), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click("Select Cursor Local").unwrap();
        });
    }

    #[test]
    fn commits_changed_provider_name_on_focus_loss_without_writing_on_type() {
        use butler::profile::Provider;
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        let profile = Profile::new(&root);
        profile.complete_onboarding().unwrap();
        let mut app =
            application(Some(root.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        let check_before_blur = profile.clone();
        interact(&mut app, move |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.focus("Provider display name").unwrap();
            sim.key_down("ControlLeft".parse().unwrap()).unwrap();
            sim.press("KeyA".parse().unwrap()).unwrap();
            sim.key_up("ControlLeft".parse().unwrap()).unwrap();
            sim.type_text("Alternate Cursor").unwrap();
            assert_eq!(
                check_before_blur.provider_name(Provider::Cursor).unwrap(),
                "Cursor"
            );
            sim.click("Select Claude").unwrap();
            sim.click("Select Alternate Cursor").unwrap();
        });
        assert_eq!(
            profile.provider_name(Provider::Cursor).unwrap(),
            "Alternate Cursor"
        );
        assert_eq!(profile.provider_name(Provider::Claude).unwrap(), "Claude");
    }

    #[test]
    fn dismisses_settings_only_after_a_click_on_the_backdrop() {
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        Profile::new(&root).complete_onboarding().unwrap();
        let mut app = application(Some(root), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click_at(Offset::new(1000.0, 685.0)).unwrap();
            assert!(sim.focus("Back to workspace").is_ok());
            sim.mouse().down(Offset::new(41.0, 100.0)).unwrap();
            sim.mouse().up(Offset::new(39.0, 100.0)).unwrap();
            assert!(sim.focus("Back to workspace").is_ok());
            sim.mouse().down(Offset::new(39.0, 100.0)).unwrap();
            sim.mouse().up(Offset::new(41.0, 100.0)).unwrap();
            assert!(sim.focus("Back to workspace").is_ok());
            sim.click_at(Offset::new(25.0, 100.0)).unwrap();
            assert!(sim.focus("Back to workspace").is_err());
            sim.focus("Open provider settings").unwrap();
        });
    }

    #[test]
    fn opens_bundled_licenses_and_returns_to_the_same_provider() {
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        Profile::new(&root).complete_onboarding().unwrap();
        let mut app = application(Some(root), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click("Select Claude").unwrap();
            sim.move_mouse_to(Offset::new(135.0, 600.0)).unwrap();
            sim.scroll(Offset::new(0.0, 320.0)).unwrap();
            sim.focus("About").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("View licenses").unwrap();
            assert!(
                sim.focus("About").is_ok(),
                "sidebar stays at the clicked row"
            );
            sim.focus("Inter").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Inter").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Lucide and Feather").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.focus("Providers").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("Sign-in for Claude isn't available in this build.")
                .unwrap();
            sim.click("Back to workspace").unwrap();
            sim.focus("Open provider settings").unwrap();
        });
    }

    #[test]
    fn filters_bundled_licenses_and_escape_leaves_settings_open() {
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        Profile::new(&root).complete_onboarding().unwrap();
        let mut app = application(Some(root), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.move_mouse_to(Offset::new(135.0, 600.0)).unwrap();
            sim.scroll(Offset::new(0.0, 320.0)).unwrap();
            sim.focus("About").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
            sim.click("View licenses").unwrap();
            sim.click("Search open-source licenses").unwrap();
            sim.focus("Search licenses").unwrap();
            sim.type_text("mit").unwrap();
            assert!(sim.focus("Inter").is_err());
            sim.press("Escape".parse().unwrap()).unwrap();
            sim.focus("Inter").unwrap();
            sim.focus("Back to workspace").unwrap();
        });
    }

    #[test]
    fn toggles_a_provider_preference_without_changing_selection_or_connecting() {
        use butler::profile::Provider;
        let tmp = Fixture::new();
        let root = tmp.0.join("profile");
        let profile = Profile::new(&root);
        profile.complete_onboarding().unwrap();
        assert!(!profile.provider_enabled(Provider::Cursor).unwrap());
        assert!(profile.provider_enabled(Provider::Claude).unwrap());
        let mut app =
            application(Some(root.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click("Enable Claude").unwrap();
            sim.click("Sign-in for Cursor isn't available in this build.")
                .unwrap();
            sim.focus("Enable Cursor").unwrap();
            sim.press("Space".parse().unwrap()).unwrap();
            sim.click("Back to workspace").unwrap();
            sim.click("Enable a provider in Settings to send a message")
                .unwrap();
        });
        assert!(profile.provider_enabled(Provider::Cursor).unwrap());
        assert!(!profile.provider_enabled(Provider::Claude).unwrap());
        drop(app);
        let mut app = application(Some(root), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.click("Enable Cursor").unwrap();
        });
        assert!(!profile.provider_enabled(Provider::Cursor).unwrap());
    }

    #[test]
    fn opens_provider_settings_then_tests_first_send_offline() {
        let tmp = Fixture::new();
        let profile = tmp.0.join("profile");
        Profile::new(&profile).complete_onboarding().unwrap();
        let mut app =
            application(Some(profile.clone()), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut app, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open provider settings").unwrap();
            sim.focus("Select Claude").unwrap();
            sim.press("Enter".parse().unwrap()).unwrap();
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
        interact(&mut app, |sim| {
            sim.click("New thread").unwrap();
            sim.type_into("Ask anything", "Check another draft")
                .unwrap();
            sim.click("Open thread: Check my draft").unwrap();
            sim.click("Check my draft").unwrap();
            sim.click("New thread").unwrap();
        });
        let store = ThreadStore::new(&profile);
        assert_eq!(store.history(&Workspace::Scratch).unwrap().len(), 1);
        assert_eq!(fs::read_dir(profile.join("scratch")).unwrap().count(), 1);
        interact(&mut app, |sim| {
            sim.click("Send").unwrap();
            sim.click("Check another draft").unwrap();
            sim.click("Open thread: Check my draft").unwrap();
            sim.click("Check my draft").unwrap();
            sim.click("Open thread: Check another draft").unwrap();
            sim.click("Check another draft").unwrap();
        });
        let history = store.history(&Workspace::Scratch).unwrap();
        assert_eq!(history.len(), 2);
        assert_ne!(history[0].id, history[1].id);
        assert_eq!(fs::read_dir(profile.join("scratch")).unwrap().count(), 2);
        interact(&mut app, |sim| {
            sim.type_into("Follow up", "More about the second draft")
                .unwrap();
            sim.click("Open thread: Check my draft").unwrap();
            sim.click("Send follow-up").unwrap(); // another thread must not receive unsent text
            sim.click("Open thread: Check another draft").unwrap();
            sim.click("Send follow-up").unwrap();
            sim.click("More about the second draft").unwrap();
            sim.click("Offline follow-up 1 received.").unwrap();
        });
        let threads = history
            .iter()
            .map(|item| store.load(item.id).unwrap().unwrap())
            .collect::<Vec<_>>();
        assert!(
            threads
                .iter()
                .find(|t| t.prompt == "Check my draft")
                .unwrap()
                .follow_ups
                .is_empty()
        );
        assert_eq!(
            threads
                .iter()
                .find(|t| t.prompt == "Check another draft")
                .unwrap()
                .follow_ups
                .len(),
            1
        );
        assert_eq!(fs::read_dir(profile.join("scratch")).unwrap().count(), 2);
        drop(app);
        let mut reopened =
            application(Some(profile), Some(tmp.0.clone()), "Fixture".into()).unwrap();
        interact(&mut reopened, |sim| {
            sim.click("Start without a project").unwrap();
            sim.click("Open thread: Check my draft").unwrap();
            sim.click("Check my draft").unwrap();
            sim.click("Offline test reply received.").unwrap();
            sim.click("Open thread: Check another draft").unwrap();
            sim.click("Check another draft").unwrap();
            sim.click("More about the second draft").unwrap();
            sim.click("Offline follow-up 1 received.").unwrap();
        });
    }
}
