//! Local provider settings route. The canned provider is available to tests only.

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, label, space};
use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
};

pub fn view(go_back: impl Fn() + 'static, #[cfg(test)] fixture_enabled: Signal<bool>) -> Widget {
    let menu = Container::new()
        .width(255.0)
        .padding(EdgeInsets::all(16.0))
        .color(Color::rgba(249, 249, 249, 255))
        .child(
            Column::new(vec![
                action("Back to workspace", go_back),
                space(24.0),
                label("Settings", 18.0, FontWeight::W600, INK),
                space(16.0),
                label("General", 14.0, FontWeight::W400, MUTED),
                space(12.0),
                label("Appearance", 14.0, FontWeight::W400, MUTED),
                space(12.0),
                label("Providers", 14.0, FontWeight::W600, INK),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Start),
        );
    let mut panels = vec![
        label("Providers", 24.0, FontWeight::W600, INK),
        space(8.0),
        label(
            "Manage the providers available on this device.",
            14.0,
            FontWeight::W400,
            MUTED,
        ),
        space(20.0),
        Container::new().height(1.0).color(RULE).into(),
        space(20.0),
    ];
    #[cfg(test)]
    if fixture_enabled.get() {
        panels.push(label(
            "Offline test provider enabled",
            14.0,
            FontWeight::W500,
            INK,
        ));
    } else {
        panels.push(action("Enable offline test provider", move || {
            fixture_enabled.set(true);
        }));
    }
    #[cfg(not(test))]
    panels.push(label(
        "No providers configured.",
        14.0,
        FontWeight::W400,
        MUTED,
    ));

    Row::new(vec![
        Widget::from(menu),
        Container::new().width(1.0).color(RULE).into(),
        Expanded::new(
            Container::new()
                .color(PAGE)
                .padding(EdgeInsets::all(28.0))
                .child(Column::new(panels).cross_axis_alignment(CrossAxisAlignment::Start)),
        )
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

pub fn action(text: &str, click: impl Fn() + 'static) -> Widget {
    Button::new(text)
        .style(
            ButtonStyle::new()
                .height(28.0)
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
