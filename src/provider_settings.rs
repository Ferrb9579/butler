//! Local provider settings overlay. The canned provider is available to tests only.

use crate::about_settings;
use crate::empty_workspace::{WHITE, space};
use crate::settings_icons::{icon, provider_brand};
use crate::typography;
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use butler::profile::{Profile, Provider};
use incular::{
    gestures::GestureCallbacks,
    prelude::*,
    widgets::{
        BackdropFilter, GestureDetector, HitTestBehavior, StackFit,
        internal::{ActionSurface, ExplicitSemantics},
    },
};
use std::{cell::RefCell, rc::Rc};

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
const PRIMARY: Color = Color::rgba(27, 78, 216, 255);
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
    enabled: Signal<[bool; 3]>,
    inputs: [TextEditingController; 3],
    names_on_focus: Rc<RefCell<[Option<String>; 3]>>,
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
        let enabled = Provider::ALL.map(|provider| match &profile {
            Some(profile) => match profile.provider_enabled(provider) {
                Ok(value) => value,
                Err(e) => {
                    error = Some(format!("Could not load provider enablement: {e}"));
                    false
                }
            },
            None => provider.enabled_by_default(),
        });
        Self {
            profile,
            committed: Signal::new(committed),
            enabled: Signal::new(enabled),
            inputs,
            names_on_focus: Rc::default(),
            error: Signal::new(error),
        }
    }

    fn toggle(&self, index: usize) {
        let mut enabled = self.enabled.get();
        let next = !enabled[index];
        let result = self
            .profile
            .as_ref()
            .ok_or_else(|| std::io::Error::other("No local profile."))
            .and_then(|profile| profile.save_provider_enabled(Provider::ALL[index], next));
        match result {
            Ok(()) => {
                enabled[index] = next;
                self.enabled.set(enabled);
                self.error.set(None);
            }
            Err(error) => {
                self.error
                    .set(Some(format!("Could not change provider settings: {error}")));
            }
        }
    }

    // Mirrors DraftInput/useCommitOnBlur: a draft is private to the focused
    // field, and only a changed value is committed at focus loss. A
    // single-line EditableText already releases focus on non-IME Enter.
    fn editing_focus(&self, index: usize, focused: bool) {
        let saved = self.committed.get()[index].clone();
        if focused {
            self.names_on_focus.borrow_mut()[index] = Some(saved.clone());
            if self.inputs[index].text() != saved {
                self.inputs[index].set_text(saved);
            }
            return;
        }
        let Some(at_focus) = self.names_on_focus.borrow_mut()[index].take() else {
            return;
        };
        let next = self.inputs[index].text();
        if next != at_focus && next != saved {
            self.save(index, &next);
        } else if next != saved {
            self.inputs[index].set_text(saved);
        }
    }

    fn save(&self, index: usize, next: &str) {
        let result = self
            .profile
            .as_ref()
            .ok_or_else(|| std::io::Error::other("No local profile."))
            .and_then(|profile| profile.save_provider_name(Provider::ALL[index], next));
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Providers,
    About,
    Licenses,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::Providers => "Providers",
            Self::About => "About",
            Self::Licenses => "Open source licenses",
        }
    }

    fn sidebar(self) -> Self {
        match self {
            Self::Licenses => Self::About,
            other => other,
        }
    }
}

#[derive(Clone)]
pub struct Navigation {
    section: Signal<Section>,
    expanded: Signal<Option<usize>>,
    sidebar_scroll: ScrollController,
}

impl Navigation {
    pub fn new() -> Self {
        Self {
            section: Signal::new(Section::Providers),
            expanded: Signal::new(None),
            sidebar_scroll: ScrollController::default(),
        }
    }

    pub fn open_providers(&self) {
        self.sidebar_scroll.jump_to(0.0);
        self.navigate(Section::Providers);
    }

    fn navigate(&self, section: Section) {
        if self.section.get() != section {
            self.expanded.set(None);
            self.section.set(section);
        }
    }
}

pub fn overlay(
    workspace: Widget,
    device: &str,
    navigation: Navigation,
    provider_selection: Signal<usize>,
    names: Names,
    go_back: impl Fn() + Clone + 'static,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let section = navigation.section.get();
    let expanded = navigation.expanded.get();
    let selected = provider_selection.get();
    let saved = names.committed.get();
    let enabled = names.enabled.get();
    let name_error = names.error.get();
    let device = device.to_owned();
    let modal = LayoutBuilder::new(move |_, constraints| {
        let width = constraints.max_width();
        let height = constraints.max_height();
        let g = modal_geometry(width, height);
        let layout = provider_geometry(width, height, g.width);
        let compact = width < 640.0;
        let first = Container::new()
            .width(layout.sidebar - 1.0)
            .color(SIDEBAR)
            .child(
                SingleChildScrollView::new(sidebar(
                    layout.sidebar == 56.0,
                    section,
                    navigation.clone(),
                ))
                .controller(navigation.sidebar_scroll.clone()),
            );
        let body = match section {
            Section::Providers => details(
                &device,
                &layout,
                compact,
                selected,
                provider_selection.clone(),
                names.clone(),
                &saved,
                enabled,
                name_error.as_deref(),
                #[cfg(test)]
                fixture_enabled.clone(),
            ),
            Section::About => {
                let nav = navigation.clone();
                about_settings::about(compact, move || nav.navigate(Section::Licenses))
            }
            Section::Licenses => {
                let nav = navigation.clone();
                about_settings::licenses(compact, expanded, move |value| {
                    nav.expanded.set(value);
                })
            }
        };
        let detail = settings_page(&layout, section, compact, body, go_back.clone());
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
        let box_position = Rect::from_origin_size(
            Offset::new((width - g.width) / 2.0, (height - g.height) / 2.0),
            Size::new(g.width, g.height),
        );
        let close_from_backdrop = go_back.clone();
        let backdrop = GestureDetector::new(BackdropFilter::new(
            GaussianBlur::uniform(4.0),
            // index.css dialog-backdrop: background at 60% opacity, blur 4px.
            Container::new().color(Color::rgba(252, 252, 252, 153)),
        ))
        .behavior(HitTestBehavior::Opaque)
        // A tap has to start on the backdrop. A release over the card does not close it.
        .on_tap_up(move |tap| {
            if !box_position.contains(tap.global_position) {
                close_from_backdrop();
            }
        });
        let panel = GestureDetector::new(
            Container::new()
                .width(g.width)
                .height(g.height)
                .decoration(decoration)
                .child(ClipRRect::new(radius, row)),
        )
        .behavior(HitTestBehavior::Opaque);
        Stack::new(vec![
            workspace.clone(),
            backdrop.into(),
            Container::new()
                .alignment(Alignment::CENTER)
                .child(panel)
                .into(),
        ])
        .fit(StackFit::Expand)
        .into()
    });
    modal.into()
}

fn sidebar(compact: bool, active: Section, navigation: Navigation) -> Widget {
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
        ("More", &["Maintenance", "Labs", "About", "Archive"][..]),
    ] {
        if !heading.is_empty() {
            rows.push(space(16.0));
            if compact {
                rows.extend([Container::new().height(1.0).color(RULE).into(), space(16.0)]);
            } else if heading != "More" {
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
            let destination = match *item {
                "Providers" => Some(Section::Providers),
                "About" => Some(Section::About),
                _ => None,
            };
            let selected = destination.is_some_and(|section| section == active.sidebar());
            let content = nav(item, selected, compact);
            if let Some(destination) = destination {
                let nav = navigation.clone();
                rows.push(
                    Widget::from(
                        ActionSurface::with_child(content)
                            .label(*item)
                            .size(Size::new(0.0, 32.0))
                            .mouse_cursor(incular::widgets::MouseCursor::Click)
                            .hover_color(if selected {
                                Color::TRANSPARENT
                            } else {
                                LIST_HOVER
                            })
                            .on_press(move || nav.navigate(destination)),
                    )
                    .semantics(
                        ExplicitSemantics::new(SemanticRole::Button)
                            .label(*item)
                            .state(SemanticState {
                                enabled: true,
                                focusable: true,
                                selected,
                                ..SemanticState::default()
                            })
                            .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
                    ),
                );
            } else {
                rows.push(content);
            }
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
                .color(if selected { WHITE } else { Color::TRANSPARENT })
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

fn provider_mark(provider: Provider) -> Widget {
    Container::new()
        .width(20.0)
        .height(20.0)
        .alignment(Alignment::CENTER)
        .child(provider_brand(
            provider.label(),
            16.0,
            Color::rgba(24, 24, 27, 204),
            false,
        ))
        .into()
}

fn provider_name_label(name: &str, size: f32, height: f32, weight: FontWeight) -> Widget {
    Text::new(name)
        .style(
            typography::interface()
                .font_size(size)
                .line_height_absolute(height)
                .font_weight(weight)
                .color(INK),
        )
        .max_lines(Some(1))
        .overflow(TextOverflow::Ellipsis)
        .into()
}

// Setting rows use their own CSS container's content box, not the window
// width or the breakpoint for splitting the provider list from its editor.
fn display_name_control_width(content_width: f32) -> Option<f32> {
    (content_width >= 512.0).then_some(224.0)
}

fn display_name_row(compact: bool, selected: usize, names: Names) -> Widget {
    let layout = LayoutBuilder::new(move |_, constraints| {
        let width = display_name_control_width(constraints.max_width());
        let rename = names.clone();
        let mut field = Container::new()
            .height(if compact { 32.0 } else { 28.0 })
            .decoration(
                BoxDecoration::new()
                    .border(Border::new(1.0, INPUT))
                    .border_radius(BorderRadius::all(Radius::circular(8.0))),
            )
            .child(
                GestureDetector::new(
                    EditableText::new(names.inputs[selected].clone())
                        .placeholder("Provider display name")
                        .content_padding(EdgeInsets::symmetric(9.0, 0.0))
                        .style(
                            typography::interface()
                                .font_size(if compact { 16.0 } else { 14.0 })
                                .color(INK),
                        ),
                )
                .callbacks(GestureCallbacks {
                    on_focus_within: Some(Rc::new(move |focused| {
                        rename.editing_focus(selected, focused);
                    })),
                    ..GestureCallbacks::default()
                }),
            );
        let title = styled("Display name", 14.0, 20.0, FontWeight::W500, INK);
        if let Some(width) = width {
            field = field.width(width);
            Row::new(vec![Widget::from(Expanded::new(title)), field.into()])
                .spacing(32.0)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .into()
        } else {
            Column::new(vec![title, field.into()])
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .spacing(12.0)
                .into()
        }
    });
    Container::new()
        .padding(EdgeInsets::symmetric(
            if compact { 12.0 } else { 16.0 },
            12.0,
        ))
        .child(layout)
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
    enabled: [bool; 3],
    name_error: Option<&str>,
    #[cfg(test)] fixture_enabled: Signal<bool>,
) -> Widget {
    let title = Provider::ALL[selected].label();
    let choice_buttons: Vec<Widget> = saved
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let selection = selection.clone();
            let checked = enabled[index];
            let row_status = if checked {
                "Not configured"
            } else {
                "Disabled"
            };
            let select_label = format!("Select {name}");
            let row_selected = selected == index;
            let text = Column::new(vec![
                provider_name_label(name, 14.0, 20.0, FontWeight::W500),
                space(2.0),
                styled(row_status, 12.0, 18.0, FontWeight::W400, SIDEBAR_MUTED),
            ])
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Start);
            let inner = Widget::from(
                ActionSurface::with_child(
                    Container::new()
                        .padding(EdgeInsets::symmetric(
                            if compact { 12.0 } else { 16.0 },
                            12.0,
                        ))
                        .alignment(Alignment::CENTER_LEFT)
                        .child(
                            Row::new(vec![
                                provider_mark(Provider::ALL[index]),
                                Expanded::new(text).into(),
                            ])
                            .spacing(12.0)
                            .cross_axis_alignment(CrossAxisAlignment::Start),
                        ),
                )
                .label(select_label.clone())
                .size(Size::new(0.0, 71.0))
                .mouse_cursor(incular::widgets::MouseCursor::Click)
                .hover_color(if row_selected {
                    Color::TRANSPARENT
                } else {
                    LIST_HOVER
                })
                .on_press(move || {
                    selection.set(index);
                }),
            )
            .semantics(
                ExplicitSemantics::new(SemanticRole::Button)
                    .label(select_label)
                    .state(SemanticState {
                        enabled: true,
                        focusable: true,
                        toggled: Some(row_selected),
                        ..SemanticState::default()
                    })
                    .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
            );
            let toggle = names.clone();
            Container::new()
                .height(71.0)
                .padding(EdgeInsets::from_ltrb(
                    0.0,
                    0.0,
                    if compact { 12.0 } else { 16.0 },
                    0.0,
                ))
                .color(if selected == index {
                    SELECTED
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    Row::new(vec![
                        Widget::from(Expanded::new(inner)),
                        provider_switch(format!("Enable {name}"), checked, compact, move || {
                            toggle.toggle(index)
                        }),
                    ])
                    .spacing(12.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center),
                )
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
    let detail_state = if enabled[selected] {
        "Not configured"
    } else {
        "Disabled"
    };
    let inset = if compact { 12.0 } else { 16.0 };
    let inset_content = |child: Widget| {
        Widget::from(
            Container::new()
                .padding(EdgeInsets::symmetric(inset, 0.0))
                .child(child),
        )
    };
    let section_title = |text| {
        inset_content(
            Container::new()
                .height(28.0)
                .alignment(Alignment::CENTER_LEFT)
                .child(styled(text, 16.0, 24.0, FontWeight::W600, INK))
                .into(),
        )
    };
    let mut property_rows = vec![
        inset_content(
            Container::new()
                .height(28.0)
                .child(
                    Row::new(vec![
                        provider_mark(Provider::ALL[selected]),
                        Expanded::new(provider_name_label(
                            &saved[selected],
                            16.0,
                            24.0,
                            FontWeight::W600,
                        ))
                        .into(),
                    ])
                    .spacing(8.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center),
                )
                .into(),
        ),
        space(10.0),
        Container::new()
            .padding(EdgeInsets::symmetric(inset, 12.0))
            .child(
                Column::new(vec![
                    styled("Status", 14.0, 20.0, FontWeight::W500, INK),
                    space(6.0),
                    styled(detail_state, 12.0, 16.0, FontWeight::W400, MUTED),
                ])
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Start),
            )
            .into(),
        Container::new().height(1.0).color(ROW_RULE).into(),
        display_name_row(compact, selected, names.clone()),
    ];
    if let Some(error) = name_error {
        property_rows.push(inset_content(label(error, 13.0, FontWeight::W400, MUTED)));
    }
    property_rows.extend([
        space(24.0),
        section_title("Setup"),
        space(10.0),
        inset_content(label("Account", 14.0, FontWeight::W500, INK)),
        space(4.0),
        inset_content(label(
            &format!("Sign-in for {title} isn't available in this build."),
            13.0,
            FontWeight::W400,
            MUTED,
        )),
        space(24.0),
        section_title("Environment"),
        space(10.0),
        inset_content(label("Variables", 14.0, FontWeight::W500, INK)),
        space(4.0),
        inset_content(label(
            "Per-provider environment variables aren't available in this build.",
            13.0,
            FontWeight::W400,
            MUTED,
        )),
    ]);
    let properties = Column::new(property_rows)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let choices = Column::new(choices)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch);
    let properties = Container::new()
        .padding(EdgeInsets::symmetric(0.0, 16.0))
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
    Column::new(content)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
}

fn settings_page(
    layout: &ProviderGeometry,
    section: Section,
    compact: bool,
    content: Widget,
    go_back: impl Fn() + 'static,
) -> Widget {
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
                breadcrumb_label(section.title(), INK),
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
                    .child(content),
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

// T3 components/ui/switch.tsx default size and light theme, using a low-level action.
fn provider_switch(
    label: String,
    checked: bool,
    compact: bool,
    click: impl Fn() + 'static,
) -> Widget {
    let unit = if compact { 20.0 } else { 16.0 };
    let size = Size::new(unit * 2.0 - 2.0, unit + 2.0);
    let thumb = unit - 2.0;
    let surface = Container::new()
        .width(size.width)
        .height(size.height)
        .padding(EdgeInsets::all(2.0))
        .alignment(if checked {
            Alignment::CENTER_RIGHT
        } else {
            Alignment::CENTER_LEFT
        })
        .decoration(
            BoxDecoration::new()
                .color(if checked { PRIMARY } else { INPUT })
                .border_radius(BorderRadius::all(Radius::circular(size.height / 2.0))),
        )
        .child(
            Container::new().width(thumb).height(thumb).decoration(
                BoxDecoration::new()
                    .color(PAGE)
                    .border_radius(BorderRadius::all(Radius::circular(thumb / 2.0))),
            ),
        );
    Widget::from(
        ActionSurface::with_child(surface)
            .label(label.clone())
            .size(size)
            .mouse_cursor(incular::widgets::MouseCursor::Click)
            .on_press(click),
    )
    .semantics(
        ExplicitSemantics::new(SemanticRole::Switch)
            .label(label)
            .state(SemanticState {
                enabled: true,
                focusable: true,
                checked: Some(checked.into()),
                ..SemanticState::default()
            })
            .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
    )
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
    fn provider_display_name_stacks_below_its_own_content_breakpoint() {
        assert_eq!(display_name_control_width(0.0), None);
        assert_eq!(display_name_control_width(511.99), None);
        assert_eq!(display_name_control_width(512.0), Some(224.0));
        assert_eq!(display_name_control_width(620.0), Some(224.0));
    }

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
