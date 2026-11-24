//! Local-project source palette and path field. Visual reference: T3 v0.1.11 desktop.

use crate::settings_icons::icon;
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use incular::{
    gestures::GestureCallbacks,
    prelude::*,
    widgets::{BackdropFilter, GestureDetector, internal::ActionSurface},
};

use butler::{
    clone_runner, managed_projects,
    projects::{FolderListing, LocalProject},
    remote_projects::{self, RemoteProject},
};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, label, space};

// T3's light CommandFooter mixes zinc-900 into its surface at these opacities.
const FOOTER_RULE: Color = Color::rgba(24, 24, 27, 18);
const FOOTER_FILL: Color = Color::rgba(24, 24, 27, 6);
const FOOTER_KEY: Color = Color::rgba(24, 24, 27, 20);
const FOOTER_INK: Color = Color::rgba(24, 24, 27, 255);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Picker {
    Closed,
    Sources,
    Local,
    New,
    RemoteUrl,
    CloneDestination,
    ThreadProjects,
}

pub enum ProjectRequest {
    Open(String),
    StartThread(PathBuf),
    Create(String),
    Clone {
        remote: RemoteProject,
        destination: String,
    },
}

#[derive(Clone)]
pub struct ThreadProjects {
    query: TextEditingController,
    filter: Signal<String>,
    choices: Signal<Vec<LocalProject>>,
}

impl ThreadProjects {
    pub fn new() -> Self {
        Self {
            query: TextEditingController::new(),
            filter: Signal::new(String::new()),
            choices: Signal::new(Vec::new()),
        }
    }

    pub fn begin(&self, choices: Vec<LocalProject>) {
        self.query.clear();
        self.filter.set(String::new());
        self.choices.set(choices);
    }

    fn matching(&self, query: &str) -> Vec<LocalProject> {
        let query = query.to_lowercase();
        self.choices
            .get()
            .into_iter()
            .filter(|project| {
                let text = format!("{} {}", project.title(), project.root.display()).to_lowercase();
                query.split_whitespace().all(|term| text.contains(term))
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct RemoteFlow {
    url: TextEditingController,
    url_text: Signal<String>,
    destination: TextEditingController,
    destination_text: Signal<String>,
    candidate: Signal<Option<RemoteProject>>,
    error: Signal<Option<String>>,
    browse: Signal<Option<FolderListing>>,
    browse_error: Signal<Option<String>>,
}

impl RemoteFlow {
    pub fn new() -> Self {
        Self {
            url: TextEditingController::new(),
            url_text: Signal::new(String::new()),
            destination: TextEditingController::new(),
            destination_text: Signal::new(String::new()),
            candidate: Signal::new(None),
            error: Signal::new(None),
            browse: Signal::new(None),
            browse_error: Signal::new(None),
        }
    }

    fn reset(&self) {
        self.url.clear();
        self.url_text.set(String::new());
        self.destination.clear();
        self.destination_text.set(String::new());
        self.candidate.set(None);
        self.error.set(None);
        self.browse.set(None);
        self.browse_error.set(None);
    }

    fn continue_to_destination(&self, input: &str, step: &Signal<Picker>, home: Option<&Path>) {
        let result = remote_projects::parse(input).and_then(|candidate| {
            clone_runner::validate_url(&candidate.url)?;
            Ok(candidate)
        });
        match result {
            Ok(candidate) => {
                let suggestion = candidate.suggested_destination();
                self.set_destination(suggestion);
                self.candidate.set(Some(candidate));
                self.error.set(None);
                if let Some(home) = home {
                    self.load_parent(home, false);
                }
                step.set(Picker::CloneDestination);
            }
            Err(error) => {
                self.candidate.set(None);
                self.error.set(Some(error.to_string()));
            }
        }
    }

    fn load_parent(&self, directory: &Path, update_destination: bool) {
        match FolderListing::read(directory) {
            Ok(list) => {
                if update_destination && let Some(candidate) = self.candidate.get() {
                    let path = if list.directory.file_name() == Some(candidate.leaf.as_ref()) {
                        list.directory.clone()
                    } else {
                        list.directory.join(&candidate.leaf)
                    };
                    self.set_destination(path.to_string_lossy().into_owned());
                }
                self.browse_error.set(None);
                self.browse.set(Some(list));
            }
            Err(error) => {
                self.browse_error
                    .set(Some(format!("Could not browse folder: {error}")));
            }
        }
    }

    fn browse_action(&self, glyph: &str, name: &str, directory: PathBuf, busy: bool) -> Widget {
        let remote = self.clone();
        let surface = ActionSurface::with_child(row(
            32.0,
            Row::new(vec![
                icon(glyph, 16.0, MUTED),
                label(name, 14.0, FontWeight::W400, INK),
            ])
            .spacing(8.0)
            .cross_axis_alignment(CrossAxisAlignment::Center),
        ))
        .label(name)
        .size(Size::new(574.0, 32.0))
        .hover_color(Color::rgba(241, 241, 244, 255));
        if busy {
            surface.into()
        } else {
            surface
                .on_press(move || remote.load_parent(&directory, true))
                .into()
        }
    }

    fn set_destination(&self, value: String) {
        self.destination.set_text(value.clone());
        self.destination_text.set(value);
    }
}

pub fn overlay(
    workspace: Widget,
    step: Signal<Picker>,
    path: TextEditingController,
    new_name: TextEditingController,
    name_preview: Signal<String>,
    remote: RemoteFlow,
    thread_projects: ThreadProjects,
    device: &str,
    current_project: Option<&Path>,
    project_data: Option<PathBuf>,
    creating: bool,
    home: Option<PathBuf>,
    browse: Signal<Option<FolderListing>>,
    browse_error: Signal<Option<String>>,
    error: Option<String>,
    submit: impl Fn(ProjectRequest) + Clone + 'static,
) -> Widget {
    let current = step.get();
    if current == Picker::Closed {
        return workspace;
    }
    let back = step.clone();
    let close = step.clone();
    let shortcut_step = step.clone();
    let shortcut_edit = new_name.clone();
    let shortcut_url = remote.url.clone();
    let header_create: Widget = if current == Picker::RemoteUrl {
        let enabled = !remote.url_text.get().trim().is_empty();
        let remote = remote.clone();
        let section = step.clone();
        let browse_home = home.clone();
        palette_submit("Continue", "Continue (Enter)", true, enabled, move || {
            remote.continue_to_destination(&remote.url.text(), &section, browse_home.as_deref());
        })
    } else if current == Picker::CloneDestination {
        let pending = remote.candidate.get();
        let enabled =
            !creating && pending.is_some() && !remote.destination_text.get().trim().is_empty();
        let field = remote.destination.clone();
        let submit = submit.clone();
        palette_submit(
            if creating {
                "Cloning"
            } else {
                "Create & Clone"
            },
            "Create & Clone",
            false,
            enabled,
            move || {
                if let Some(remote) = pending.clone() {
                    submit(ProjectRequest::Clone {
                        remote,
                        destination: field.text(),
                    });
                }
            },
        )
    } else if current == Picker::New {
        let name = name_preview.get();
        let allow_create = !creating
            && project_data
                .as_ref()
                .is_some_and(|base| managed_projects::preview_root(base, &name).is_ok());
        let controller = new_name.clone();
        let submit = submit.clone();
        let text = if creating { "Creating" } else { "Create" };
        Button::with_child(
            "Create (Enter)",
            Row::new(vec![
                label(text, 12.0, FontWeight::W500, INK),
                Container::new()
                    .height(16.0)
                    .padding(EdgeInsets::symmetric(4.0, 0.0))
                    .alignment(Alignment::CENTER)
                    .decoration(
                        BoxDecoration::new()
                            .color(Color::rgba(24, 24, 27, 18))
                            .border_radius(BorderRadius::all(Radius::circular(4.0))),
                    )
                    .child(label("Enter", 10.0, FontWeight::W400, MUTED))
                    .into(),
            ])
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .spacing(6.0)
            .into(),
        )
        .style(
            ButtonStyle::new()
                .variant(ButtonVariant::Outline)
                .height(24.0)
                .padding(EdgeInsets::symmetric(6.0, 0.0)),
        )
        .enabled(allow_create)
        .on_click(move || submit(ProjectRequest::Create(controller.text())))
        .into()
    } else {
        SizedBox::new().width(0.0).into()
    };
    let mut rows = vec![row(
        48.0,
        Row::new(vec![
            Button::with_child("Back", icon("arrow-left", 16.0, MUTED))
                .style(
                    ButtonStyle::new()
                        .variant(ButtonVariant::Ghost)
                        .fixed_size(Size::new(16.0, 24.0))
                        .padding(EdgeInsets::ZERO),
                )
                .enabled(!creating)
                .on_click(move || {
                    if creating {
                        return;
                    }
                    back.set(
                        if matches!(
                            current,
                            Picker::Local
                                | Picker::New
                                | Picker::RemoteUrl
                                | Picker::CloneDestination
                        ) {
                            Picker::Sources
                        } else {
                            Picker::Closed
                        },
                    );
                })
                .into(),
            Widget::from(SizedBox::new().width(8.0)),
            if current == Picker::ThreadProjects {
                let edit = thread_projects.clone();
                let search = thread_projects.clone();
                let submit = submit.clone();
                Expanded::new(
                    EditableText::new(thread_projects.query.clone())
                        .placeholder("New thread in...")
                        .placeholder_color(MUTED)
                        .style(crate::typography::interface().font_size(15.0).color(INK))
                        .on_changed(move |value| {
                            edit.filter.set(value.to_owned());
                        })
                        .on_submit(move |value| {
                            if let Some(project) = search.matching(&value).into_iter().next() {
                                submit(ProjectRequest::StartThread(project.root));
                            }
                        }),
                )
                .into()
            } else if current == Picker::RemoteUrl {
                let edit = remote.clone();
                let review = remote.clone();
                let step = step.clone();
                let browse_home = home.clone();
                Expanded::new(
                    EditableText::new(remote.url.clone())
                        .placeholder("Enter Git clone URL")
                        .placeholder_color(MUTED)
                        .style(crate::typography::interface().font_size(15.0).color(INK))
                        .on_changed(move |value| {
                            edit.url_text.set(value.to_owned());
                            edit.error.set(None);
                        })
                        .on_submit(move |value| {
                            if !value.trim().is_empty() {
                                review.continue_to_destination(
                                    &value,
                                    &step,
                                    browse_home.as_deref(),
                                );
                            }
                        }),
                )
                .into()
            } else if current == Picker::CloneDestination {
                let edit = remote.clone();
                Expanded::new(
                    EditableText::new(remote.destination.clone())
                        .placeholder("Enter path (e.g. ~/projects/my-app)")
                        .placeholder_color(MUTED)
                        .read_only(creating)
                        .style(crate::typography::interface().font_size(15.0).color(INK))
                        .on_changed(move |value| {
                            edit.destination_text.set(value.to_owned());
                        }),
                )
                .into()
            } else if current == Picker::New {
                let preview = name_preview.clone();
                let submit = submit.clone();
                Expanded::new(
                    EditableText::new(new_name.clone())
                        .placeholder("Project name")
                        .read_only(creating)
                        .placeholder_color(MUTED)
                        .style(crate::typography::interface().font_size(15.0).color(INK))
                        .on_changed(move |value| {
                            preview.set(value.to_owned());
                        })
                        .on_submit(move |value| {
                            if !creating && !value.trim().is_empty() {
                                submit(ProjectRequest::Create(value));
                            }
                        }),
                )
                .into()
            } else if current == Picker::Local {
                Expanded::new(
                    EditableText::new(path.clone())
                        .placeholder("Absolute folder path")
                        .placeholder_color(MUTED)
                        .style(
                            TextStyle::new()
                                .font_family("Liberation Sans")
                                .font_size(14.0)
                                .color(INK),
                        ),
                )
                .into()
            } else {
                label("Search...", 14.0, FontWeight::W400, MUTED)
            },
            header_create,
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center),
    )];
    match current {
        Picker::Sources => {
            rows.push(row(28.0, label("Sources", 12.0, FontWeight::W500, MUTED)));
            let create_source = row(
                48.0,
                source(
                    "palette-folder-git",
                    "New project",
                    "Start a new Git repository from a name",
                    project_data.is_some(),
                ),
            );
            if project_data.is_some() {
                let step = step.clone();
                let name = new_name.clone();
                let preview = name_preview.clone();
                rows.push(
                    ActionSurface::with_child(create_source)
                        .label("New project")
                        .size(Size::new(574.0, 48.0))
                        .hover_color(Color::rgba(241, 241, 244, 255))
                        .on_press(move || {
                            name.clear();
                            preview.set(String::new());
                            step.set(Picker::New);
                        })
                        .into(),
                );
            } else {
                rows.push(create_source);
            }
            rows.push(
                ActionSurface::with_child(row(
                    48.0,
                    source(
                        "palette-folder-plus",
                        "Local folder",
                        "Browse a folder on disk",
                        true,
                    ),
                ))
                .label("Local folder")
                .size(Size::new(574.0, 48.0))
                .hover_color(Color::rgba(241, 241, 244, 255))
                .on_press({
                    let browse = browse.clone();
                    let browse_error = browse_error.clone();
                    let step = step.clone();
                    move || {
                        browse.set(None);
                        browse_error.set(None);
                        if let Some(home) = &home {
                            navigate(home.clone(), &browse, &browse_error);
                        }
                        step.set(Picker::Local);
                    }
                })
                .into(),
            );
            let git_source = row(
                48.0,
                source(
                    "palette-link",
                    "Git URL",
                    "Clone from a remote URL",
                    project_data.is_some(),
                ),
            );
            if project_data.is_some() && cfg!(any(target_os = "linux", target_os = "macos")) {
                let remote = remote.clone();
                let git_step = step.clone();
                rows.push(
                    ActionSurface::with_child(git_source)
                        .label("Git URL")
                        .size(Size::new(574.0, 48.0))
                        .hover_color(Color::rgba(241, 241, 244, 255))
                        .on_press(move || {
                            remote.reset();
                            git_step.set(Picker::RemoteUrl);
                        })
                        .into(),
                );
            } else {
                rows.push(git_source);
            }
            rows.push(row(
                48.0,
                source("◉", "GitHub repository", "Clone GitHub owner/repo", false),
            ));
        }
        Picker::Local => {
            let listing = browse.get();
            let current = listing.as_ref().map(|value| value.directory.clone());
            rows.push(row(
                28.0,
                label("Directories", 12.0, FontWeight::W500, MUTED),
            ));
            rows.push(row(
                44.0,
                short_action("Open folder", {
                    let path = path.clone();
                    let current = current.clone();
                    move || {
                        let text = path.text();
                        if text.is_empty() {
                            if let Some(dir) = current.as_ref().and_then(|d| d.to_str()) {
                                submit(ProjectRequest::Open(dir.to_owned()));
                                return;
                            }
                        }
                        submit(ProjectRequest::Open(text));
                    }
                }),
            ));
            if let Some(listing) = listing {
                let mut dirs = Vec::new();
                if let Some(parent) = listing.directory.parent().map(PathBuf::from) {
                    dirs.push(browse_action(
                        "corner-left-up",
                        "Parent directory",
                        parent,
                        browse.clone(),
                        browse_error.clone(),
                        path.clone(),
                    ));
                }
                for (name, target) in listing.children {
                    dirs.push(browse_action(
                        "folder",
                        &name,
                        target,
                        browse.clone(),
                        browse_error.clone(),
                        path.clone(),
                    ));
                }
                if !dirs.is_empty() {
                    let list_height = (dirs.len() as f32 * 32.0).min(224.0);
                    rows.push(
                        Container::new()
                            .height(list_height)
                            .child(SingleChildScrollView::new(
                                Column::new(dirs)
                                    .main_axis_size(MainAxisSize::Min)
                                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                            ))
                            .into(),
                    );
                }
            }
            for message in [browse_error.get(), error].into_iter().flatten() {
                rows.push(row(
                    30.0,
                    label(
                        &message,
                        13.0,
                        FontWeight::W400,
                        Color::rgba(161, 38, 38, 255),
                    ),
                ));
            }
        }
        Picker::New => {
            let name = name_preview.get();
            let destination = project_data.as_ref().map(|base| {
                if name.trim().is_empty() {
                    base.join("projects")
                } else {
                    managed_projects::preview_root(base, &name)
                        .unwrap_or_else(|_| base.join("projects"))
                }
            });
            let location = destination
                .as_ref()
                .map(|path| {
                    format!(
                        "{} {}",
                        if name.trim().is_empty() {
                            "Goes in"
                        } else {
                            "Creates"
                        },
                        path.display()
                    )
                })
                .unwrap_or_else(|| "No local profile directory".into());
            rows.push(row(
                52.0,
                source(
                    "palette-folder-git",
                    if name.trim().is_empty() {
                        "New project"
                    } else {
                        name.trim()
                    },
                    &location,
                    true,
                ),
            ));
            let existing = step.clone();
            rows.push(
                ActionSurface::with_child(row(
                    48.0,
                    source(
                        "palette-folder-plus",
                        "Add existing project",
                        "Open a folder or clone a repository",
                        true,
                    ),
                ))
                .label("Add existing project")
                .size(Size::new(574.0, 48.0))
                .hover_color(Color::rgba(241, 241, 244, 255))
                .on_press(move || {
                    if !creating {
                        existing.set(Picker::Sources);
                    }
                })
                .into(),
            );
            if let Some(message) = error {
                rows.push(row(
                    30.0,
                    label(
                        &message,
                        13.0,
                        FontWeight::W400,
                        Color::rgba(161, 38, 38, 255),
                    ),
                ));
            }
        }
        Picker::RemoteUrl => {
            rows.push(
                Container::new()
                    .height(96.0)
                    .alignment(Alignment::CENTER)
                    .child(label(
                        "Enter a Git clone URL and press Enter to continue.",
                        14.0,
                        FontWeight::W400,
                        MUTED,
                    ))
                    .into(),
            );
            if let Some(error) = remote.error.get() {
                rows.push(row(
                    30.0,
                    label(
                        &error,
                        13.0,
                        FontWeight::W400,
                        Color::rgba(161, 38, 38, 255),
                    ),
                ));
            }
        }
        Picker::CloneDestination => {
            if let Some(candidate) = remote.candidate.get() {
                rows.push(row(
                    24.0,
                    label("Repository", 12.0, FontWeight::W500, MUTED),
                ));
                rows.push(row(
                    48.0,
                    source("palette-link", &candidate.input, &candidate.url, true),
                ));
            }
            rows.push(row(
                28.0,
                label("Select where to clone", 12.0, FontWeight::W500, MUTED),
            ));
            let mut has_choices = false;
            if let Some(listing) = remote.browse.get() {
                let at_home = home
                    .as_ref()
                    .and_then(|path| path.canonicalize().ok())
                    .is_some_and(|root| root == listing.directory);
                let mut choices = Vec::new();
                if !at_home && let Some(parent) = listing.directory.parent() {
                    choices.push(remote.browse_action(
                        "corner-left-up",
                        "Parent directory",
                        parent.to_owned(),
                        creating,
                    ));
                }
                for (name, path) in listing.children {
                    choices.push(remote.browse_action("folder", &name, path, creating));
                }
                if !choices.is_empty() {
                    has_choices = true;
                    rows.push(
                        Container::new()
                            .height((choices.len() as f32 * 32.0).min(160.0))
                            .child(SingleChildScrollView::new(
                                Column::new(choices)
                                    .main_axis_size(MainAxisSize::Min)
                                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                            ))
                            .into(),
                    );
                }
            }
            if let Some(message) = remote.browse_error.get() {
                rows.push(row(30.0, label(&message, 13.0, FontWeight::W400, MUTED)));
            }
            if !has_choices {
                rows.push(row(
                    48.0,
                    label(
                        "Choose a destination path and select Create & Clone.",
                        14.0,
                        FontWeight::W400,
                        MUTED,
                    ),
                ));
            }
            if let Some(error) = error {
                rows.push(row(
                    30.0,
                    label(
                        &error,
                        13.0,
                        FontWeight::W400,
                        Color::rgba(161, 38, 38, 255),
                    ),
                ));
            }
        }
        Picker::ThreadProjects => {
            rows.push(row(28.0, label("Projects", 12.0, FontWeight::W500, MUTED)));
            let matches = thread_projects.matching(&thread_projects.filter.get());
            if matches.is_empty() {
                rows.push(row(
                    48.0,
                    label("No projects found", 14.0, FontWeight::W400, MUTED),
                ));
            } else {
                let options = matches
                    .into_iter()
                    .map(|project| {
                        let title = project.title();
                        let location = format!("{device} · {}", project.root.display());
                        let is_current = current_project == Some(project.root.as_path());
                        let title_with_status = if is_current {
                            format!("{title}  ·  Current")
                        } else {
                            title.clone()
                        };
                        let submit = submit.clone();
                        ActionSurface::with_child(row(
                            48.0,
                            source("palette-folder-git", &title_with_status, &location, true),
                        ))
                        .label(format!("New thread in {title}"))
                        .size(Size::new(574.0, 48.0))
                        .hover_color(Color::rgba(241, 241, 244, 255))
                        .on_press(move || submit(ProjectRequest::StartThread(project.root.clone())))
                        .into()
                    })
                    .collect::<Vec<Widget>>();
                rows.push(
                    Container::new()
                        .height((options.len() as f32 * 48.0).min(240.0))
                        .child(SingleChildScrollView::new(
                            Column::new(options)
                                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                                .main_axis_size(MainAxisSize::Min),
                        ))
                        .into(),
                );
            }
            if let Some(message) = error {
                rows.push(row(
                    30.0,
                    label(
                        &message,
                        13.0,
                        FontWeight::W400,
                        Color::rgba(161, 38, 38, 255),
                    ),
                ));
            }
        }
        Picker::Closed => unreachable!(),
    }
    let footer = if current == Picker::RemoteUrl {
        remote_url_footer(move || {
            close.set(Picker::Closed);
        })
    } else if current == Picker::New {
        let hint = |key, sentence| {
            Widget::from(
                Row::new(vec![
                    shortcut_key(key),
                    label(sentence, 14.0, FontWeight::W400, MUTED),
                ])
                .main_axis_size(MainAxisSize::Min)
                .spacing(4.0)
                .cross_axis_alignment(CrossAxisAlignment::Center),
            )
        };
        row(
            40.0,
            Row::new(vec![
                hint("Enter", "Create"),
                hint("Backspace", "Back"),
                Button::with_child(
                    "Close",
                    Row::new(vec![
                        shortcut_key("Esc"),
                        label("Close", 14.0, FontWeight::W400, MUTED),
                    ])
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .spacing(4.0)
                    .into(),
                )
                .style(
                    ButtonStyle::new()
                        .variant(ButtonVariant::Ghost)
                        .height(30.0)
                        .padding(EdgeInsets::ZERO),
                )
                .enabled(!creating)
                .on_click(move || {
                    close.set(Picker::Closed);
                })
                .into(),
            ])
            .main_axis_size(MainAxisSize::Min)
            .spacing(12.0)
            .cross_axis_alignment(CrossAxisAlignment::Center),
        )
    } else {
        row(
            40.0,
            short_action("Close", move || {
                close.set(Picker::Closed);
            }),
        )
    };
    rows.push(space(if current == Picker::New { 10.0 } else { 16.0 }));
    if current != Picker::RemoteUrl {
        rows.push(Container::new().height(1.0).color(RULE).into());
    }
    rows.push(footer);

    let popup = Container::new()
        .width(576.0)
        .margin(EdgeInsets::only(0.0, 80.0, 0.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(PAGE)
                .border(Border::new(1.0, RULE))
                .box_shadow([BoxShadow::new(
                    Color::rgba(18, 18, 24, 79),
                    Offset::new(0.0, 18.0),
                    32.0,
                    0.0,
                )])
                .border_radius(BorderRadius::all(Radius::circular(16.0))),
        )
        .child(
            Column::new(rows)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        );
    Stack::new(vec![
        workspace,
        BackdropFilter::new(
            GaussianBlur::uniform(4.0),
            Container::new().color(Color::rgba(252, 252, 252, 84)),
        )
        .into(),
        GestureDetector::new(popup)
            .callbacks(GestureCallbacks {
                on_key: Some(Rc::new(move |event| {
                    let active_editor = match current {
                        Picker::New => Some(&shortcut_edit),
                        Picker::RemoteUrl => Some(&shortcut_url),
                        _ => None,
                    };
                    if creating
                        || !event.state.is_down()
                        || active_editor.is_some_and(|edit| {
                            edit.preedit().is_some() || edit.composing().is_some()
                        })
                    {
                        return false;
                    }
                    if event.code == Code::Escape {
                        shortcut_step.set(Picker::Closed);
                        true
                    } else if event.code == Code::Backspace
                        && match current {
                            Picker::New => name_preview.get().is_empty(),
                            Picker::RemoteUrl => shortcut_url.text().is_empty(),
                            _ => false,
                        }
                    {
                        shortcut_step.set(Picker::Sources);
                        true
                    } else {
                        false
                    }
                })),
                ..GestureCallbacks::default()
            })
            .into(),
    ])
    .alignment(Alignment::TOP_CENTER)
    .into()
}

/// CommandPaletteContent.tsx, ui/command.tsx and ui/kbd.tsx.
/// Only hint at keys Butler actually handles. Keep the existing pointer Close
/// action: the stock palette does not yet match T3's modal focus handling.
fn remote_url_footer(close: impl Fn() + 'static) -> Widget {
    let hint = |key: &'static str, title: &'static str| {
        let children: Vec<Widget> = vec![
            Container::new()
                .height(20.0)
                .constraints(Constraints::new(20.0, f32::INFINITY, 20.0, 20.0))
                .padding(EdgeInsets::symmetric(4.0, 0.0))
                .alignment(Alignment::CENTER)
                .decoration(
                    BoxDecoration::new()
                        .color(FOOTER_KEY)
                        .border_radius(BorderRadius::all(Radius::circular(4.0))),
                )
                .child(
                    Text::new(key).style(
                        crate::typography::interface()
                            .font_size(12.0)
                            .line_height_absolute(16.0)
                            .font_weight(FontWeight::W500)
                            .color(FOOTER_INK),
                    ),
                )
                .into(),
            Text::new(title)
                .style(
                    crate::typography::interface()
                        .font_size(14.0)
                        .line_height_absolute(20.0)
                        .font_weight(FontWeight::W500)
                        .color(MUTED),
                )
                .into(),
        ];
        Row::new(children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .spacing(4.0)
    };
    let items: Vec<Widget> = vec![
        hint("Enter", "Continue").into(),
        hint("Backspace", "Back").into(),
        ActionSurface::with_child(hint("Esc", "Close"))
            .label("Close")
            .on_press(close)
            .into(),
    ];
    let items = Row::new(items)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .spacing(12.0);
    Container::new()
        .decoration(
            BoxDecoration::new()
                .color(FOOTER_FILL)
                .border_radius(BorderRadius::vertical(Radius::ZERO, Radius::circular(15.0))),
        )
        .child(
            Column::new(vec![
                Container::new().height(1.0).color(FOOTER_RULE).into(),
                row(40.0, items),
            ])
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .into()
}

fn source(icon_text: &str, title: &str, description: &str, ready: bool) -> Widget {
    let line = |text, size, ink| {
        Widget::from(
            Text::new(text)
                .style(crate::typography::interface().font_size(size).color(ink))
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        )
    };
    let mut items = vec![
        if icon_text.starts_with("palette-") {
            icon(icon_text, 16.0, MUTED)
        } else {
            label(icon_text, 16.0, FontWeight::W400, MUTED)
        },
        SizedBox::new().width(8.0).into(),
        Expanded::new(
            Column::new(vec![
                line(title, 14.0, if ready { INK } else { MUTED }),
                line(description, 12.0, MUTED),
            ])
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Start),
        )
        .into(),
    ];
    if title == "Add existing project" {
        items.push(icon("chevron-right", 16.0, MUTED));
    }
    Row::new(items)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .into()
}

fn palette_submit(
    text: &str,
    accessible: &str,
    with_enter: bool,
    enabled: bool,
    click: impl Fn() + 'static,
) -> Widget {
    let mut children = vec![label(text, 12.0, FontWeight::W500, INK)];
    if with_enter {
        children.push(shortcut_key("Enter"));
    }
    Button::with_child(
        accessible,
        Row::new(children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .spacing(4.0)
            .into(),
    )
    .style(
        ButtonStyle::new()
            .variant(ButtonVariant::Outline)
            .height(24.0)
            .padding(EdgeInsets::symmetric(7.0, 0.0)),
    )
    .enabled(enabled)
    .on_click(click)
    .into()
}

fn shortcut_key(text: &str) -> Widget {
    Container::new()
        .height(20.0)
        .padding(EdgeInsets::symmetric(4.0, 0.0))
        .alignment(Alignment::CENTER)
        .decoration(
            BoxDecoration::new()
                .color(Color::rgba(24, 24, 27, 18))
                .border_radius(BorderRadius::all(Radius::circular(4.0))),
        )
        .child(label(text, 12.0, FontWeight::W400, INK))
        .into()
}

fn row(height: f32, child: impl Into<Widget>) -> Widget {
    Container::new()
        .height(height)
        .padding(EdgeInsets::symmetric(16.0, 0.0))
        .alignment(Alignment::CENTER_LEFT)
        .child(child)
        .into()
}

fn short_action(text: &str, click: impl Fn() + 'static) -> Widget {
    Button::new(text)
        .style(
            ButtonStyle::new()
                .height(26.0)
                .background(PAGE)
                .foreground(INK)
                .variant(ButtonVariant::Ghost)
                .text_style(
                    TextStyle::new()
                        .font_family("Liberation Sans")
                        .font_size(14.0),
                ),
        )
        .on_click(click)
        .into()
}

fn navigate(
    target: PathBuf,
    browse: &Signal<Option<FolderListing>>,
    error: &Signal<Option<String>>,
) -> bool {
    match FolderListing::read(&target) {
        Ok(listing) => {
            error.set(None);
            browse.set(Some(listing));
            true
        }
        Err(problem) => {
            error.set(Some(format!("Could not browse folder: {problem}")));
            false
        }
    }
}

fn browse_action(
    glyph: &str,
    name: &str,
    target: PathBuf,
    browse: Signal<Option<FolderListing>>,
    error: Signal<Option<String>>,
    path: TextEditingController,
) -> Widget {
    ActionSurface::with_child(row(
        32.0,
        Row::new(vec![
            icon(glyph, 16.0, MUTED),
            label(name, 14.0, FontWeight::W400, INK),
        ])
        .spacing(8.0)
        .cross_axis_alignment(CrossAxisAlignment::Center),
    ))
    .size(Size::new(574.0, 32.0))
    .label(name)
    .hover_color(Color::rgba(241, 241, 244, 255))
    .on_press(move || {
        if navigate(target.clone(), &browse, &error) {
            path.set_text(target.to_string_lossy().into_owned());
        }
    })
    .into()
}
