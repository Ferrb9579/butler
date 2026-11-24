//! Local provider settings overlay. The canned provider is available to tests only.

use crate::empty_workspace::{WHITE, label, space};
use butler::profile::{Profile, Provider};
use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
    widgets::{BackdropFilter, StackFit, internal::ActionSurface},
};

// 8-bit sRGB conversions of the light OKLCH tokens in the pinned release CSS.
const PAGE: Color = Color::rgba(252, 252, 252, 255); // zinc-25
const SIDEBAR: Color = Color::rgba(250, 250, 250, 255); // zinc-50
const RULE: Color = Color::rgba(228, 228, 231, 255); // zinc-200
const MUTED: Color = Color::rgba(113, 113, 123, 255); // zinc-500
const INK: Color = Color::rgba(24, 24, 27, 255); // zinc-900
const SELECTED: Color = Color::rgba(240, 240, 243, 255);

// T3 Code v0.1.11 AppRouteModal.tsx:68–104. Current native window exposes
// no caption/safe-area insets to this layout; these dimensions are for zero inset.
#[derive(Debug, PartialEq)]
struct ModalGeometry {
    width: f32,
    height: f32,
    radius: f32,
    framed: bool,
}

fn modal_geometry(width: f32, height: f32) -> ModalGeometry {
    let framed = width >= 640.0;
    let inset = if width >= 1024.0 {
        40.0
    } else if framed {
        24.0
    } else {
        0.0
    };
    ModalGeometry {
        width: if framed {
            (width - 2.0 * inset).min(1280.0)
        } else {
            width
        },
        height: if framed {
            (height - 2.0 * inset).clamp(0.0, 960.0)
        } else {
            height
        },
        radius: if framed { 16.0 } else { 0.0 },
        framed,
    }
}

#[derive(Clone)]
pub struct Names {
    profile: Option<Profile>,
    committed: Signal<[String; 3]>,
    inputs: [TextEditingController; 3],
    error: Signal<Option<String>>,
}

impl Names {
    pub fn new(profile: Option<Profile>) -> Self {
        let mut error = None;
        let committed = Provider::ALL.map(|provider| {
            if let Some(profile) = &profile {
                match profile.provider_name(provider) {
                    Ok(name) => return name,
                    Err(e) => error = Some(format!("Could not load a provider name: {e}")),
                }
            }
            provider.label().into()
        });
        let inputs = std::array::from_fn(|index| {
            let controller = TextEditingController::new();
            controller.set_text(committed[index].clone());
            controller
        });
        Self {
            profile,
            committed: Signal::new(committed),
            inputs,
            error: Signal::new(error),
        }
    }

    fn save(&self, index: usize) {
        let result = self
            .profile
            .as_ref()
            .ok_or_else(|| std::io::Error::other("No local profile."))
            .and_then(|profile| {
                profile.save_provider_name(Provider::ALL[index], &self.inputs[index].text())
            });
        match result {
            Ok(name) => {
                self.inputs[index].set_text(name.clone());
                let mut saved = self.committed.get();
                saved[index] = name;
                self.committed.set(saved);
                self.error.set(None);
            }
            Err(e) => {
                self.error
                    .set(Some(format!("Could not save display name: {e}")));
            }
        }
    }
}

pub fn overlay(
    workspace: Widget,
    device: &str,
    provider_selection: Signal<usize>,
    names: Names,
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
        names,
        go_back,
        #[cfg(test)]
        fixture_enabled,
    );
    let row: Widget = Row::new(vec![
        Widget::from(first),
        Container::new().width(1.0).color(RULE).into(),
        Expanded::new(detail).into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into();
    let modal = LayoutBuilder::new(move |_, constraints| {
        let g = modal_geometry(constraints.max_width(), constraints.max_height());
        let radius = BorderRadius::all(Radius::circular(g.radius));
        let mut decoration = BoxDecoration::new().color(PAGE).border_radius(radius);
        if g.framed {
            // Tailwind shadow-2xl/40: 0px 25px 50px -12px, black at 40%.
            decoration = decoration
                .border(Border::new(1.0, RULE))
                .box_shadow([BoxShadow::new(
                    Color::rgba(0, 0, 0, 102),
                    Offset::new(0.0, 25.0),
                    50.0,
                    -12.0,
                )]);
        }
        Container::new()
            .alignment(Alignment::CENTER)
            .child(
                Container::new()
                    .width(g.width)
                    .height(g.height)
                    .decoration(decoration)
                    .child(ClipRRect::new(radius, row.clone())),
            )
            .into()
    });
    Stack::new(vec![
        workspace,
        BackdropFilter::new(
            GaussianBlur::uniform(4.0),
            // index.css dialog-backdrop: background at 60% opacity, blur 4px.
            Container::new().color(Color::rgba(252, 252, 252, 153)),
        )
        .into(),
        modal.into(),
    ])
    .fit(StackFit::Expand)
    .into()
}

fn sidebar() -> Widget {
    let mut rows = vec![nav("Search", false), space(4.0)];
    for (heading, section) in [
        (
            "",
            &[
                "General",
                "Appearance",
                "Chat",
                "Diffs & pull requests",
                "Keybindings",
                "Voice",
                "SnapShots",
            ][..],
        ),
        (
            "Agents",
            &[
                "Providers",
                "Threads",
                "Browser",
                "Devices",
                "Scheduled tasks",
            ][..],
        ),
        (
            "Code",
            &["Projects", "Source control & issues", "Worktrees"][..],
        ),
    ] {
        if !heading.is_empty() {
            rows.push(space(8.0));
            rows.push(
                Container::new()
                    .padding(EdgeInsets::symmetric(8.0, 4.0))
                    .child(label(heading, 12.0, FontWeight::W500, MUTED))
                    .into(),
            );
        }
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
        .height(36.0)
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
    names: Names,
    go_back: impl Fn() + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let title = Provider::ALL[selected].label();
    let saved = names.committed.get();
    let heading = Container::new()
        .height(48.0)
        .padding(EdgeInsets::from_ltrb(20.0, 0.0, 8.0, 0.0))
        .child(
            Row::new(vec![
                breadcrumb_label("Settings", MUTED),
                SizedBox::new().width(12.0).into(),
                breadcrumb_label("/", MUTED),
                SizedBox::new().width(12.0).into(),
                breadcrumb_label("Providers", INK),
                Spacer::new().into(),
                ActionSurface::with_child(label("×", 16.0, FontWeight::W400, MUTED))
                    .label("Back to workspace")
                    .size(Size::new(28.0, 28.0))
                    .on_press(go_back)
                    .into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center),
        );

    let mut choices: Vec<Widget> = saved
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
            .label(name.as_str())
            .size(Size::new(272.0, if index == 2 { 72.0 } else { 82.0 }))
            .color(if selected == index { SELECTED } else { PAGE })
            .hover_color(SELECTED)
            .on_press(move || {
                selection.set(index);
            })
            .into()
        })
        .collect();
    choices.extend([space(12.0), nav("Add provider", false)]);
    let rename = names.clone();
    let mut property_rows = vec![
        label(&saved[selected], 17.0, FontWeight::W600, INK),
        space(28.0),
        label("Status", 14.0, FontWeight::W500, INK),
        space(8.0),
        label("No providers configured.", 13.0, FontWeight::W400, MUTED),
        space(16.0),
        Container::new().height(1.0).color(RULE).into(),
        space(16.0),
        Row::new(vec![
            label("Display name", 14.0, FontWeight::W500, INK),
            Spacer::new().into(),
            Container::new()
                .width(200.0)
                .height(28.0)
                .padding(EdgeInsets::symmetric(8.0, 0.0))
                .alignment(Alignment::CENTER_LEFT)
                .decoration(
                    BoxDecoration::new()
                        .border(Border::new(1.0, RULE))
                        .border_radius(BorderRadius::all(Radius::circular(8.0))),
                )
                .child(
                    EditableText::new(names.inputs[selected].clone())
                        .placeholder("Provider display name")
                        .style(TextStyle::new().font_size(14.0).color(INK))
                        .on_submit(move |_| rename.save(selected)),
                )
                .into(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .into(),
    ];
    if let Some(error) = names.error.get() {
        property_rows.push(label(&error, 13.0, FontWeight::W400, MUTED));
    }
    property_rows.extend([
        space(34.0),
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
        space(18.0),
        label("Variables", 14.0, FontWeight::W500, INK),
        space(8.0),
        label(
            "Per-provider environment variables aren't available in this build.",
            13.0,
            FontWeight::W400,
            MUTED,
        ),
    ]);
    let properties = Column::new(property_rows)
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
                Expanded::new(
                    Container::new()
                        .decoration(BoxDecoration::new().border(Border::new(1.0, RULE)))
                        .padding(EdgeInsets::all(16.0))
                        .child(properties),
                )
                .into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Start),
        )
        .into();
    let mut content = vec![
        Row::new(vec![
            label("Applying settings for", 14.0, FontWeight::W400, MUTED),
            SizedBox::new().width(8.0).into(),
            chip("All projects"),
            SizedBox::new().width(8.0).into(),
            label("on", 14.0, FontWeight::W400, MUTED),
            SizedBox::new().width(8.0).into(),
            chip(device),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .into(),
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
            Container::new()
                .padding(EdgeInsets::symmetric(56.0, 24.0))
                .child(
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

fn chip(text: &str) -> Widget {
    Container::new()
        .padding(EdgeInsets::symmetric(10.0, 5.0))
        .decoration(
            BoxDecoration::new()
                .color(WHITE)
                .border(Border::new(1.0, RULE))
                .border_radius(BorderRadius::all(Radius::circular(8.0))),
        )
        .child(label(text, 14.0, FontWeight::W400, INK))
        .into()
}

fn breadcrumb_label(text: &str, color: Color) -> Widget {
    Text::new(text)
        .style(
            TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(14.0)
                .line_height_absolute(20.0)
                .font_weight(FontWeight::W500)
                .color(color),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t3_modal_breakpoints_and_caps_at_zero_native_inset() {
        for (width, height, expected) in [
            (639.0, 800.0, (639.0, 800.0, 0.0, false)),
            (640.0, 800.0, (592.0, 752.0, 16.0, true)),
            (1023.0, 800.0, (975.0, 752.0, 16.0, true)),
            (1024.0, 800.0, (944.0, 720.0, 16.0, true)),
            (1280.0, 800.0, (1200.0, 720.0, 16.0, true)),
            (1920.0, 1200.0, (1280.0, 960.0, 16.0, true)),
        ] {
            let g = modal_geometry(width, height);
            assert_eq!((g.width, g.height, g.radius, g.framed), expected);
        }
    }
}
