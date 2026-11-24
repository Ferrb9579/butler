//! Local provider settings overlay. The canned provider is available to tests only.

use crate::empty_workspace::{WHITE, space};
use crate::settings_icons::icon;
use crate::typography;
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use butler::profile::{Profile, Provider};
use incular::{
    prelude::*,
    widgets::{BackdropFilter, StackFit, internal::ActionSurface},
};

// 8-bit sRGB conversions of the light OKLCH tokens in the pinned release CSS.
const PAGE: Color = Color::rgba(252, 252, 252, 255); // zinc-25
const SIDEBAR: Color = Color::rgba(250, 250, 250, 255); // zinc-50
const RULE: Color = Color::rgba(228, 228, 231, 255); // zinc-200
const MUTED: Color = Color::rgba(113, 113, 123, 255); // zinc-500
const INK: Color = Color::rgba(24, 24, 27, 255); // zinc-900
const SELECTED: Color = Color::rgba(24, 24, 27, 15); // foreground/6
const LIST_HOVER: Color = Color::rgba(250, 250, 250, 64); // muted/25
const ROW_RULE: Color = Color::rgba(228, 228, 231, 128); // border/50
const INPUT: Color = Color::rgba(212, 212, 216, 255); // zinc-300
const SIDEBAR_ICON: Color = Color::rgba(113, 113, 123, 217);
const SIDEBAR_MUTED: Color = Color::rgba(113, 113, 123, 204);

#[derive(Debug, PartialEq)]
struct ProviderGeometry {
    sidebar: f32,
    page: f32,
    gutter: f32,
    card_height: Option<f32>,
}

// WorkspacePageContainer, SettingsSidebarNav, and @container/providers.
fn provider_geometry(
    viewport_width: f32,
    viewport_height: f32,
    modal_width: f32,
) -> ProviderGeometry {
    let sidebar = if viewport_width < 768.0 { 56.0 } else { 240.0 };
    let page = (modal_width - sidebar).clamp(0.0, 896.0);
    let gutter = if viewport_width < 640.0 { 20.0 } else { 24.0 };
    let card_height =
        (page - 2.0 * gutter >= 768.0).then(|| (viewport_height - 176.0).clamp(512.0, 704.0));
    ProviderGeometry {
        sidebar,
        page,
        gutter,
        card_height,
    }
}

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
    go_back: impl Fn() + Clone + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let selected = provider_selection.get();
    let saved = names.committed.get();
    let name_error = names.error.get();
    let device = device.to_owned();
    let modal = LayoutBuilder::new(move |_, constraints| {
        let width = constraints.max_width();
        let height = constraints.max_height();
        let g = modal_geometry(width, height);
        let layout = provider_geometry(width, height, g.width);
        let first = Container::new()
            .width(layout.sidebar - 1.0)
            .color(SIDEBAR)
            .child(SingleChildScrollView::new(sidebar(layout.sidebar == 56.0)));
        let detail = details(
            &device,
            &layout,
            width < 640.0,
            selected,
            provider_selection.clone(),
            names.clone(),
            &saved,
            name_error.as_deref(),
            go_back.clone(),
            #[cfg(test)]
            fixture_enabled.clone(),
        );
        let row = Row::new(vec![
            Widget::from(first),
            Container::new().width(1.0).color(RULE).into(),
            Expanded::new(detail).into(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
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
                    .child(ClipRRect::new(radius, row)),
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

fn sidebar(compact: bool) -> Widget {
    let mut rows = vec![nav("Search", false, compact), space(8.0)];
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
        ("Connections", &["Environments", "Sharing & access"][..]),
    ] {
        if !heading.is_empty() {
            rows.push(space(16.0));
            if compact {
                rows.extend([Container::new().height(1.0).color(RULE).into(), space(16.0)]);
            } else {
                rows.push(
                    Container::new()
                        .height(16.0)
                        .padding(EdgeInsets::symmetric(8.0, 0.0))
                        .alignment(Alignment::CENTER_LEFT)
                        .child(styled(heading, 12.0, 16.0, FontWeight::W500, SIDEBAR_MUTED))
                        .into(),
                );
                rows.push(space(4.0));
            }
        }
        for (index, item) in section.iter().enumerate() {
            if index > 0 {
                rows.push(space(4.0));
            }
            rows.push(nav(item, *item == "Providers", compact));
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

fn nav(text: &str, selected: bool, compact: bool) -> Widget {
    let search = text == "Search";
    let mut items = vec![icon(text, 16.0, if selected { INK } else { SIDEBAR_ICON })];
    if !compact {
        items.push(styled(
            text,
            14.0,
            20.0,
            FontWeight::W500,
            if selected { INK } else { SIDEBAR_MUTED },
        ));
        if search {
            items.extend([
                Widget::from(Spacer::new()),
                Container::new()
                    .width(20.0)
                    .height(20.0)
                    .alignment(Alignment::CENTER)
                    .decoration(
                        BoxDecoration::new()
                            .color(Color::rgba(24, 24, 27, 18))
                            .border_radius(BorderRadius::all(Radius::circular(4.0))),
                    )
                    .child(styled("/", 12.0, 16.0, FontWeight::W400, MUTED))
                    .into(),
            ]);
        }
    }
    Container::new()
        .height(32.0)
        .padding(EdgeInsets::symmetric(if search { 8.0 } else { 10.0 }, 0.0))
        .alignment(if compact {
            Alignment::CENTER
        } else {
            Alignment::CENTER_LEFT
        })
        .decoration(
            BoxDecoration::new()
                .color(if selected { WHITE } else { SIDEBAR })
                .border_radius(BorderRadius::all(Radius::circular(8.0))),
        )
        .child(
            Row::new(items)
                .spacing(8.0)
                .main_axis_size(if search && !compact {
                    MainAxisSize::Max
                } else {
                    MainAxisSize::Min
                })
                .cross_axis_alignment(CrossAxisAlignment::Center),
        )
        .into()
}

fn details(
    device: &str,
    layout: &ProviderGeometry,
    compact: bool,
    selected: usize,
    selection: Signal<usize>,
    names: Names,
    saved: &[String; 3],
    name_error: Option<&str>,
    go_back: impl Fn() + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let title = Provider::ALL[selected].label();
    let gap = if compact { 8.0 } else { 12.0 };
    let close_size = if compact { 32.0 } else { 28.0 };
    let heading = Container::new()
        .height(48.0)
        .padding(EdgeInsets::from_ltrb(20.0, 0.0, 8.0, 0.0))
        .child(
            Row::new(vec![
                breadcrumb_label("Settings", MUTED),
                SizedBox::new().width(gap).into(),
                breadcrumb_label("/", MUTED),
                SizedBox::new().width(gap).into(),
                breadcrumb_label("Providers", INK),
                Spacer::new().into(),
                Button::with_child(
                    "Back to workspace",
                    icon("close", if compact { 18.0 } else { 16.0 }, MUTED),
                )
                .style(
                    ButtonStyle::new()
                        .fixed_size(Size::new(close_size, close_size))
                        .variant(ButtonVariant::Ghost)
                        .background(Color::TRANSPARENT)
                        .padding(EdgeInsets::ZERO),
                )
                .on_click(go_back)
                .into(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center),
        );

    let choice_buttons: Vec<Widget> = saved
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let selection = selection.clone();
            ActionSurface::with_child(
                Container::new()
                    .padding(EdgeInsets::symmetric(
                        if compact { 12.0 } else { 16.0 },
                        12.0,
                    ))
                    .alignment(Alignment::CENTER_LEFT)
                    .child(
                        Column::new(vec![
                            styled(name, 14.0, 20.0, FontWeight::W500, INK),
                            styled(
                                "Not configured",
                                12.0,
                                16.0,
                                FontWeight::W400,
                                SIDEBAR_MUTED,
                            ),
                        ])
                        .main_axis_size(MainAxisSize::Min)
                        .cross_axis_alignment(CrossAxisAlignment::Start),
                    ),
            )
            .label(name.as_str())
            .size(Size::new(0.0, 71.0))
            .color(if selected == index {
                SELECTED
            } else {
                Color::TRANSPARENT
            })
            .hover_color(if selected == index {
                SELECTED
            } else {
                LIST_HOVER
            })
            .on_press(move || {
                selection.set(index);
            })
            .into()
        })
        .collect();
    let mut choices: Vec<Widget> = choice_buttons
        .into_iter()
        .flat_map(|row| [row, Container::new().height(1.0).color(ROW_RULE).into()])
        .collect();
    choices.push(
        Container::new()
            .height(44.0)
            .padding(EdgeInsets::symmetric(
                if compact { 12.0 } else { 16.0 },
                0.0,
            ))
            .alignment(Alignment::CENTER_LEFT)
            .child(
                Row::new(vec![
                    icon("plus", 16.0, MUTED),
                    styled("Add provider", 14.0, 20.0, FontWeight::W400, MUTED),
                ])
                .spacing(12.0)
                .cross_axis_alignment(CrossAxisAlignment::Center),
            )
            .into(),
    );
    let rename = names.clone();
    let mut property_rows = vec![
        styled(&saved[selected], 16.0, 24.0, FontWeight::W600, INK),
        space(20.0),
        styled("Status", 14.0, 20.0, FontWeight::W500, INK),
        space(4.0),
        styled(
            "No providers configured.",
            12.0,
            16.0,
            FontWeight::W400,
            MUTED,
        ),
        space(12.0),
        Container::new().height(1.0).color(ROW_RULE).into(),
        space(12.0),
        Row::new(vec![
            styled("Display name", 14.0, 20.0, FontWeight::W500, INK),
            Spacer::new().into(),
            Container::new()
                .width(224.0)
                .height(28.0)
                .padding(EdgeInsets::symmetric(9.0, 0.0))
                .alignment(Alignment::CENTER_LEFT)
                .decoration(
                    BoxDecoration::new()
                        .border(Border::new(1.0, INPUT))
                        .border_radius(BorderRadius::all(Radius::circular(8.0))),
                )
                .child(
                    EditableText::new(names.inputs[selected].clone())
                        .placeholder("Provider display name")
                        .style(typography::interface().font_size(14.0).color(INK))
                        .on_submit(move |_| rename.save(selected)),
                )
                .into(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .into(),
    ];
    if let Some(error) = name_error {
        property_rows.push(label(error, 13.0, FontWeight::W400, MUTED));
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
    let choices = Column::new(choices)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let properties = Container::new()
        .padding(EdgeInsets::all(16.0))
        .child(properties);
    let card: Widget = if let Some(height) = layout.card_height {
        Container::new()
            .height(height)
            .child(
                Row::new(vec![
                    Widget::from(
                        Container::new()
                            .width(271.0)
                            .child(SingleChildScrollView::new(choices)),
                    ),
                    Container::new()
                        .width(1.0)
                        .color(Color::rgba(228, 228, 231, 153))
                        .into(),
                    Expanded::new(SingleChildScrollView::new(properties)).into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
            )
            .into()
    } else {
        Column::new(vec![
            Widget::from(choices),
            Container::new()
                .height(1.0)
                .color(Color::rgba(228, 228, 231, 153))
                .into(),
            properties.into(),
        ])
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
    };
    let radius = BorderRadius::all(Radius::circular(12.0));
    let provider_box: Widget = Container::new()
        .decoration(
            BoxDecoration::new()
                .color(PAGE)
                .border(Border::new(1.0, Color::rgba(228, 228, 231, 153)))
                .border_radius(radius),
        )
        .child(ClipRRect::new(radius, card))
        .into();
    let inset = if compact { 12.0 } else { 16.0 };
    let scope_pair = |prefix, value, device_icon| {
        Row::new(vec![
            styled(prefix, 14.0, 20.0, FontWeight::W400, MUTED),
            chip(value, compact, device_icon),
        ])
        .spacing(6.0)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Center)
    };
    let mut content = vec![
        Widget::from(
            Container::new()
                .padding(EdgeInsets::symmetric(inset, 0.0))
                .child(
                    Wrap::new(vec![
                        Widget::from(scope_pair("Applying settings for", "All projects", false)),
                        scope_pair("on", device, true).into(),
                    ])
                    .spacing(6.0)
                    .run_spacing(4.0),
                ),
        ),
        space(32.0),
        Container::new()
            .height(28.0)
            .padding(EdgeInsets::symmetric(inset, 0.0))
            .alignment(Alignment::CENTER_LEFT)
            .child(styled("Providers", 16.0, 24.0, FontWeight::W600, INK))
            .into(),
        space(10.0),
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
    content.push(provider_box);
    Column::new(vec![
        Widget::from(heading),
        Container::new().height(1.0).color(RULE).into(),
        Expanded::new(SingleChildScrollView::new(
            Container::new().alignment(Alignment::TOP_CENTER).child(
                Container::new()
                    .width(layout.page)
                    .padding(EdgeInsets::from_ltrb(
                        layout.gutter,
                        24.0,
                        layout.gutter,
                        48.0,
                    ))
                    .child(
                        Column::new(content)
                            .main_axis_size(MainAxisSize::Min)
                            .cross_axis_alignment(CrossAxisAlignment::Stretch),
                    ),
            ),
        ))
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

fn chip(text: &str, compact: bool, device_icon: bool) -> Widget {
    let mut items = Vec::new();
    if device_icon {
        items.push(icon("cloud", 14.0, MUTED));
    }
    items.push(styled(
        text,
        if compact { 16.0 } else { 14.0 },
        20.0,
        FontWeight::W500,
        INK,
    ));
    items.push(icon("chevron-down", 14.0, Color::rgba(113, 113, 123, 179)));
    Container::new()
        .height(if compact { 32.0 } else { 28.0 })
        .padding(EdgeInsets::symmetric(10.0, 0.0))
        .alignment(Alignment::CENTER)
        .decoration(
            BoxDecoration::new()
                .color(WHITE)
                .border(Border::new(1.0, INPUT))
                .border_radius(BorderRadius::all(Radius::circular(8.0))),
        )
        .child(
            Row::new(items)
                .spacing(6.0)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Center),
        )
        .into()
}

fn breadcrumb_label(text: &str, color: Color) -> Widget {
    styled(text, 14.0, 20.0, FontWeight::W500, color)
}

fn label(text: &str, size: f32, weight: FontWeight, color: Color) -> Widget {
    let height = match size {
        12.0 | 13.0 => 16.0,
        14.0 => 20.0,
        _ => 24.0,
    };
    styled(text, size, height, weight, color)
}

fn styled(text: &str, size: f32, height: f32, weight: FontWeight, color: Color) -> Widget {
    Text::new(text)
        .style(
            typography::interface()
                .font_size(size)
                .line_height_absolute(height)
                .font_weight(weight)
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
                .text_style(typography::interface().font_size(14.0)),
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

    #[test]
    fn providers_uses_viewport_and_container_breakpoints_separately() {
        let geometry = |w, h| provider_geometry(w, h, modal_geometry(w, h).width);
        assert_eq!(
            geometry(639.0, 800.0),
            ProviderGeometry {
                sidebar: 56.0,
                page: 583.0,
                gutter: 20.0,
                card_height: None
            }
        );
        assert_eq!(geometry(767.0, 800.0).sidebar, 56.0);
        assert_eq!(geometry(768.0, 800.0).sidebar, 240.0);
        assert_eq!(geometry(1135.0, 800.0).card_height, None);
        assert_eq!(geometry(1136.0, 800.0).card_height, Some(624.0));
        assert_eq!(
            geometry(1280.0, 800.0),
            ProviderGeometry {
                sidebar: 240.0,
                page: 896.0,
                gutter: 24.0,
                card_height: Some(624.0)
            }
        );
        assert_eq!(geometry(1280.0, 640.0).card_height, Some(512.0));
        assert_eq!(geometry(1280.0, 1100.0).card_height, Some(704.0));
    }
}
