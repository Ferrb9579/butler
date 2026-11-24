//! Local provider settings overlay. The canned provider is available to tests only.

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, WHITE, label, space};
use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
    widgets::{BackdropFilter, StackFit, internal::ActionSurface},
};

const SIDEBAR: Color = Color::rgba(249, 249, 249, 255);
const SELECTED: Color = Color::rgba(240, 240, 243, 255);
const PROVIDERS: [&str; 3] = ["Codex", "Claude", "Cursor"];

pub fn overlay(
    workspace: Widget,
    device: &str,
    provider_selection: Signal<usize>,
    go_back: impl Fn() + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let selected = provider_selection.get();
    let first = Container::new()
        .width(239.0)
        .color(SIDEBAR)
        .child(SingleChildScrollView::new(sidebar()));
    let detail = details(
        device,
        selected,
        provider_selection,
        go_back,
        #[cfg(test)]
        fixture_enabled,
    );
    let modal = Container::new()
        .margin(EdgeInsets::all(40.0))
        .decoration(
            BoxDecoration::new()
                .color(PAGE)
                .border(Border::new(1.0, RULE))
                .border_radius(BorderRadius::all(Radius::circular(16.0)))
                .box_shadow([BoxShadow::new(
                    Color::rgba(18, 18, 24, 69),
                    Offset::new(0.0, 18.0),
                    34.0,
                    0.0,
                )]),
        )
        .child(ClipRRect::new(
            BorderRadius::all(Radius::circular(16.0)),
            Row::new(vec![
                Widget::from(first),
                Container::new().width(1.0).color(RULE).into(),
                Expanded::new(detail).into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        ));
    Stack::new(vec![
        workspace,
        BackdropFilter::new(
            GaussianBlur::uniform(4.0),
            Container::new().color(Color::rgba(252, 252, 252, 72)),
        )
        .into(),
        modal.into(),
    ])
    .fit(StackFit::Expand)
    .into()
}

fn sidebar() -> Widget {
    let mut rows = vec![nav("Search", false), space(4.0)];
    for section in [
        &[
            "General",
            "Appearance",
            "Chat",
            "Diffs & pull requests",
            "Keybindings",
            "Voice",
            "SnapShots",
        ][..],
        &[
            "Agents",
            "Providers",
            "Threads",
            "Browser",
            "Devices",
            "Scheduled tasks",
        ][..],
        &["Code", "Projects", "Source control & issues", "Worktrees"][..],
    ] {
        for item in section {
            rows.push(nav(item, *item == "Providers"));
        }
    }
    Container::new()
        .padding(EdgeInsets::symmetric(8.0, 8.0))
        .child(
            Column::new(rows)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        )
        .into()
}

fn nav(text: &str, selected: bool) -> Widget {
    Container::new()
        .height(35.0)
        .padding(EdgeInsets::symmetric(12.0, 0.0))
        .alignment(Alignment::CENTER_LEFT)
        .decoration(
            BoxDecoration::new()
                .color(if selected { WHITE } else { SIDEBAR })
                .border_radius(BorderRadius::all(Radius::circular(8.0))),
        )
        .child(label(
            text,
            14.0,
            FontWeight::W400,
            if selected { INK } else { MUTED },
        ))
        .into()
}

fn details(
    device: &str,
    selected: usize,
    selection: Signal<usize>,
    go_back: impl Fn() + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let title = PROVIDERS[selected];
    let heading = Container::new()
        .height(48.0)
        .padding(EdgeInsets::symmetric(20.0, 0.0))
        .child(
            Row::new(vec![
                label("Settings  /  Providers", 14.0, FontWeight::W500, INK),
                Spacer::new().into(),
                ActionSurface::with_child(label("×", 20.0, FontWeight::W400, MUTED))
                    .label("Back to workspace")
                    .size(Size::new(28.0, 28.0))
                    .on_press(go_back)
                    .into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center),
        );

    let mut choices: Vec<Widget> = PROVIDERS
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let selection = selection.clone();
            ActionSurface::with_child(
                Container::new()
                    .padding(EdgeInsets::symmetric(16.0, 6.0))
                    .alignment(Alignment::CENTER_LEFT)
                    .child(
                        Column::new(vec![
                            label(name, 15.0, FontWeight::W500, INK),
                            space(5.0),
                            label("Not configured", 12.0, FontWeight::W400, MUTED),
                        ])
                        .main_axis_size(MainAxisSize::Min)
                        .cross_axis_alignment(CrossAxisAlignment::Start),
                    ),
            )
            .label(*name)
            .size(Size::new(272.0, 78.0))
            .color(if selected == index { SELECTED } else { PAGE })
            .hover_color(SELECTED)
            .on_press(move || {
                selection.set(index);
            })
            .into()
        })
        .collect();
    choices.extend([space(12.0), nav("Add provider", false)]);
    let properties = Column::new(vec![
        label(title, 17.0, FontWeight::W600, INK),
        space(28.0),
        label("Status", 14.0, FontWeight::W500, INK),
        space(8.0),
        label("No providers configured.", 13.0, FontWeight::W400, MUTED),
        space(16.0),
        Container::new().height(1.0).color(RULE).into(),
        space(16.0),
        label("Display name", 14.0, FontWeight::W500, INK),
        space(8.0),
        label(title, 14.0, FontWeight::W400, MUTED),
        space(24.0),
        label("Setup", 16.0, FontWeight::W600, INK),
        space(12.0),
        label("Account", 14.0, FontWeight::W500, INK),
        space(8.0),
        label(
            &format!("Sign-in for {title} isn't available in this build."),
            13.0,
            FontWeight::W400,
            MUTED,
        ),
        space(24.0),
        label("Environment", 16.0, FontWeight::W600, INK),
    ])
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Start);
    let provider_box: Widget = Container::new()
        .decoration(
            BoxDecoration::new()
                .color(PAGE)
                .border(Border::new(1.0, RULE))
                .border_radius(BorderRadius::all(Radius::circular(12.0))),
        )
        .child(
            Row::new(vec![
                Widget::from(
                    Container::new().width(272.0).child(
                        Column::new(choices)
                            .main_axis_size(MainAxisSize::Min)
                            .cross_axis_alignment(CrossAxisAlignment::Stretch),
                    ),
                ),
                Container::new().width(1.0).height(405.0).color(RULE).into(),
                Expanded::new(
                    Container::new()
                        .padding(EdgeInsets::all(16.0))
                        .child(properties),
                )
                .into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Start),
        )
        .into();
    let mut content = vec![
        label(
            &format!("Applying settings for   All projects   on   {device}"),
            14.0,
            FontWeight::W400,
            MUTED,
        ),
        space(34.0),
        label("Providers", 16.0, FontWeight::W600, INK),
        space(14.0),
    ];
    #[cfg(test)]
    if fixture_enabled.get() {
        content.push(label(
            "Offline test provider enabled",
            14.0,
            FontWeight::W500,
            INK,
        ));
    } else {
        content.push(action("Enable offline test provider", move || {
            fixture_enabled.set(true);
        }));
    }
    content.extend([provider_box, space(20.0)]);
    Column::new(vec![
        Widget::from(heading),
        Container::new().height(1.0).color(RULE).into(),
        Expanded::new(SingleChildScrollView::new(
            Container::new().padding(EdgeInsets::all(32.0)).child(
                Column::new(content)
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ),
        ))
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
