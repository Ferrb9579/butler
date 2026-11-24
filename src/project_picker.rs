//! Local-project source palette and path field. Visual reference: T3 v0.1.11 desktop.

use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
    widgets::BackdropFilter,
};

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
                Button::with_child(row(
                    48.0,
                    source("⊞", "Local folder", "Browse a folder on disk", true),
                ))
                .style(
                    ButtonStyle::new()
                        .height(48.0)
                        .background(PAGE)
                        .variant(ButtonVariant::Ghost)
                        .padding(EdgeInsets::all(0.0)),
                )
                .on_click(move || {
                    step.set(Picker::Local);
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
            rows.push(row(
                28.0,
                label("Local folder", 12.0, FontWeight::W500, MUTED),
            ));
            rows.push(row(
                44.0,
                short_action("Open folder", move || submit(path.text())),
            ));
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
