//! Local-project source palette and path field. Visual reference: T3 v0.1.11 desktop.

use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
    widgets::{BackdropFilter, internal::ActionSurface},
};

use butler::projects::FolderListing;
use std::path::PathBuf;

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, label, space};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Picker {
    Closed,
    Sources,
    Local,
}

pub fn overlay(
    workspace: Widget,
    step: Signal<Picker>,
    path: TextEditingController,
    home: Option<PathBuf>,
    browse: Signal<Option<FolderListing>>,
    browse_error: Signal<Option<String>>,
    error: Option<String>,
    submit: impl Fn(String) + 'static,
) -> Widget {
    let current = step.get();
    if current == Picker::Closed {
        return workspace;
    }
    let back = step.clone();
    let close = step.clone();
    let mut rows = vec![row(
        48.0,
        Row::new(vec![
            short_action("‹", move || {
                back.set(if current == Picker::Local {
                    Picker::Sources
                } else {
                    Picker::Closed
                });
            }),
            Widget::from(SizedBox::new().width(8.0)),
            if current == Picker::Local {
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
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center),
    )];
    match current {
        Picker::Sources => {
            rows.push(row(28.0, label("Sources", 12.0, FontWeight::W500, MUTED)));
            rows.push(row(
                48.0,
                source(
                    "＋",
                    "New project",
                    "Start a new Git repository from a name",
                    false,
                ),
            ));
            rows.push(
                ActionSurface::with_child(row(
                    48.0,
                    source("⊞", "Local folder", "Browse a folder on disk", true),
                ))
                .label("Local folder")
                .size(Size::new(574.0, 48.0))
                .hover_color(Color::rgba(241, 241, 244, 255))
                .on_press({
                    let browse = browse.clone();
                    let browse_error = browse_error.clone();
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
            rows.push(row(
                48.0,
                source("↗", "Git URL", "Clone from a remote URL", false),
            ));
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
                                submit(dir.to_owned());
                                return;
                            }
                        }
                        submit(text);
                    }
                }),
            ));
            if let Some(listing) = listing {
                let mut dirs = Vec::new();
                if let Some(parent) = listing.directory.parent().map(PathBuf::from) {
                    dirs.push(browse_action(
                        "Parent directory",
                        parent,
                        browse.clone(),
                        browse_error.clone(),
                        path.clone(),
                    ));
                }
                for (name, target) in listing.children {
                    dirs.push(browse_action(
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
        Picker::Closed => unreachable!(),
    }
    rows.extend([
        space(16.0),
        Container::new().height(1.0).color(RULE).into(),
        row(
            40.0,
            short_action("Close", move || {
                close.set(Picker::Closed);
            }),
        ),
    ]);

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
        popup.into(),
    ])
    .alignment(Alignment::TOP_CENTER)
    .into()
}

fn source(icon: &str, title: &str, description: &str, ready: bool) -> Widget {
    Row::new(vec![
        label(icon, 16.0, FontWeight::W400, MUTED),
        SizedBox::new().width(8.0).into(),
        Column::new(vec![
            label(
                title,
                15.0,
                FontWeight::W400,
                if ready { INK } else { MUTED },
            ),
            label(description, 13.0, FontWeight::W400, MUTED),
        ])
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
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
    name: &str,
    target: PathBuf,
    browse: Signal<Option<FolderListing>>,
    error: Signal<Option<String>>,
    path: TextEditingController,
) -> Widget {
    ActionSurface::with_child(row(32.0, label(name, 14.0, FontWeight::W400, INK)))
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
