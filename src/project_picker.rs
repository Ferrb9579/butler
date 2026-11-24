use incular::{
    controls::{Button, ButtonStyle},
    prelude::*,
};

use crate::empty_workspace::{INK, MUTED, WHITE, label, space};

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
    let mut controls = match step.get() {
        Picker::Closed => return workspace,
        Picker::Sources => {
            let next = step.clone();
            vec![
                label("Add project", 18.0, FontWeight::W600, INK),
                space(14.0),
                label("New project", 14.0, FontWeight::W500, MUTED),
                space(8.0),
                action("Local folder", move || {
                    next.set(Picker::Local);
                }),
                label("Browse a folder on disk", 13.0, FontWeight::W400, MUTED),
                space(8.0),
                label("Git URL", 14.0, FontWeight::W500, MUTED),
            ]
        }
        Picker::Local => vec![
            label("Local folder", 18.0, FontWeight::W600, INK),
            space(14.0),
            Container::new()
                .height(38.0)
                .padding(EdgeInsets::all(8.0))
                .color(Color::rgba(245, 245, 246, 255))
                .child(
                    EditableText::new(path.clone())
                        .placeholder("Absolute folder path")
                        .style(TextStyle::new().font_size(14.0).color(INK)),
                )
                .into(),
            space(8.0),
            action("Open folder", move || submit(path.text())),
        ],
    };
    if let Some(error) = error {
        controls.extend([
            space(8.0),
            label(
                &error,
                14.0,
                FontWeight::W400,
                Color::rgba(161, 38, 38, 255),
            ),
        ]);
    }
    controls.extend([
        space(8.0),
        action("Cancel", move || {
            step.set(Picker::Closed);
        }),
    ]);
    let popup = Container::new()
        .width(480.0)
        .margin(EdgeInsets::only(0.0, 94.0, 0.0, 0.0))
        .padding(EdgeInsets::all(20.0))
        .decoration(
            BoxDecoration::new()
                .color(WHITE)
                .border_radius(BorderRadius::all(Radius::circular(12.0))),
        )
        .child(
            Column::new(controls)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        );
    Stack::new(vec![workspace, popup.into()])
        .alignment(Alignment::TOP_CENTER)
        .into()
}

fn action(text: &str, click: impl Fn() + 'static) -> Widget {
    Button::new(text)
        .style(
            ButtonStyle::new()
                .height(30.0)
                .background(WHITE)
                .foreground(INK),
        )
        .on_click(click)
        .into()
}
