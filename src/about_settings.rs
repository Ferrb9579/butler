//! Offline Settings bodies, ported from the pinned T3 Code v0.1.11 source.

use crate::{
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{
    gestures::GestureCallbacks,
    prelude::*,
    widgets::{RawTooltip, RawTooltipTriggerMode, internal::ExplicitSemantics},
};
use std::{cell::Cell, rc::Rc};

const INK: Color = Color::rgba(24, 24, 27, 255);
const MUTED: Color = Color::rgba(113, 113, 123, 255);
const RULE: Color = Color::rgba(228, 228, 231, 153);
const NOTICE_INK: Color = Color::rgba(24, 24, 27, 204);
const PAGE: Color = Color::rgba(252, 252, 253, 255);
const INPUT: Color = Color::rgba(212, 212, 216, 255);
const RING: Color = Color::rgba(27, 78, 216, 255);
const SEARCH_LABEL: &str = "Search open-source licenses";
// 76ch at 12px, using Liberation Mono's 0.6em digit advance.
const NOTICE_MAX_WIDTH: f32 = 76.0 * 12.0 * 0.6;

struct Notice {
    name: &'static str,
    version: Option<&'static str>,
    license: &'static str,
    bundle: &'static str,
    text: &'static str,
}

const NOTICES: [Notice; 3] = [
    Notice {
        name: "Inter",
        version: None,
        license: "OFL-1.1",
        bundle: "Assets",
        text: include_str!("../LICENSES/Inter-OFL-1.1.txt"),
    },
    Notice {
        name: "Lucide and Feather",
        version: None,
        license: "ISC · MIT",
        bundle: "Assets",
        text: include_str!("../LICENSES/Lucide-ISC-and-Feather-MIT.txt"),
    },
    Notice {
        name: "T3 Code",
        version: Some("0.1.11"),
        license: "MIT",
        bundle: "Desktop",
        text: include_str!("../LICENSES/T3-Code-MIT.txt"),
    },
];

// SettingsPanels.tsx:2449–2539, AboutVersionTitle.tsx:28–51.
pub fn about(compact: bool, open_licenses: impl Fn() + 'static) -> Widget {
    let x = inset(compact);
    let version_title = Row::new([
        label("Version", 14.0, 20.0, FontWeight::W500, INK),
        Text::new(env!("CARGO_PKG_VERSION"))
            .style(mono(11.0, 16.0, FontWeight::W500, MUTED))
            .into(),
    ])
    .spacing(8.0)
    .cross_axis_alignment(CrossAxisAlignment::Baseline);
    let app = setting_row(
        x,
        version_title.into(),
        "Current version of the application.",
        None,
    );
    let action = Button::new("View licenses")
        .style(
            ButtonStyle::new()
                .variant(ButtonVariant::Outline)
                .height(if compact { 32.0 } else { 28.0 })
                .padding(EdgeInsets::symmetric(9.0, 0.0))
                .text_style(interface(
                    if compact { 16.0 } else { 14.0 },
                    if compact { 24.0 } else { 20.0 },
                    FontWeight::W500,
                    INK,
                )),
        )
        .on_click(open_licenses);
    let legal = setting_row(
        x,
        label("Open source licenses", 14.0, 20.0, FontWeight::W500, INK),
        "Notices for third-party code and assets bundled with Butler.",
        Some(action.into()),
    );
    Column::new([
        section("App", x, grouped(app)),
        section("Legal", x, grouped(legal)),
    ])
    .spacing(32.0)
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

#[derive(Clone)]
pub struct LicenseSearch {
    open: Rc<Cell<bool>>,
    controller: TextEditingController,
    revision: Rc<Cell<u64>>,
}

impl LicenseSearch {
    pub fn new() -> Self {
        Self {
            open: Rc::new(Cell::new(false)),
            controller: TextEditingController::new(),
            revision: Rc::new(Cell::new(0)),
        }
    }

    pub fn reset(&self) {
        self.open.set(false);
        self.controller.set_text("");
        self.invalidate();
    }

    fn set_open(&self, open: bool) {
        if self.open.replace(open) != open {
            self.invalidate();
        }
    }

    fn invalidate(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }
}

// OpenSourceLicenses.tsx:78–229,249–312. Bundled files replace the web manifest;
// this is deliberately not represented as a complete dependency inventory.
pub fn licenses(
    compact: bool,
    expanded: Option<usize>,
    on_expanded: impl Fn(Option<usize>) + Clone + 'static,
    search: LicenseSearch,
) -> Widget {
    Widget::stateful_builder(search.revision.clone(), move |_| {
        let open = search.open.get();
        let filtered = matching_notices(&search.controller.text());
        license_contents(
            compact,
            expanded,
            on_expanded.clone(),
            &search,
            open,
            filtered,
        )
    })
}

fn license_contents(
    compact: bool,
    expanded: Option<usize>,
    on_expanded: impl Fn(Option<usize>) + Clone + 'static,
    search: &LicenseSearch,
    search_open: bool,
    filtered: Vec<(usize, &'static Notice)>,
) -> Widget {
    let x = inset(compact);
    let count = if filtered.len() == NOTICES.len() {
        format!("{} notices", NOTICES.len())
    } else {
        format!("{} of {}", filtered.len(), NOTICES.len())
    };
    let heading = Container::new()
        .padding(EdgeInsets::symmetric(x, 0.0))
        .child(
            Wrap::new([
                header_item(
                    label("Third-party notices", 16.0, 24.0, FontWeight::W600, INK),
                    28.0,
                ),
                search_action(search.clone(), search_open, compact, &count),
            ])
            .spacing(16.0)
            .run_spacing(4.0)
            .alignment(WrapAlignment::SpaceBetween),
        );
    let summary: Widget = Container::new()
        .padding(EdgeInsets::from_ltrb(x, 12.0, x, 12.0))
        .child(label(
            "These notices are bundled with Butler. This is not a complete dependency inventory.",
            12.0,
            18.0,
            FontWeight::W400,
            MUTED,
        ))
        .into();
    let mut rows = vec![summary];
    if filtered.is_empty() {
        rows.push(divider());
        rows.push(
            Container::new()
                .padding(EdgeInsets::symmetric(x, 32.0))
                .alignment(Alignment::CENTER)
                .child(label(
                    "No licenses match that search.",
                    14.0,
                    24.0,
                    FontWeight::W400,
                    MUTED,
                ))
                .into(),
        );
    }
    for (index, notice) in filtered {
        let open = expanded == Some(index);
        let set_expanded = on_expanded.clone();
        rows.push(divider());
        rows.push(notice_row(notice, compact, open, move || {
            set_expanded((!open).then_some(index));
        }));
        if open {
            rows.push(
                Container::new()
                    .padding(EdgeInsets::from_ltrb(x + 24.0, 4.0, x + 24.0, 16.0))
                    .alignment(Alignment::CENTER_LEFT)
                    .child(ConstrainedBox::new(
                        Constraints::new(0.0, NOTICE_MAX_WIDTH, 0.0, f32::INFINITY),
                        Text::new(notice.text)
                            .style(mono(12.0, 20.0, FontWeight::W400, NOTICE_INK))
                            .soft_wrap(true),
                    ))
                    .into(),
            );
        }
    }
    Column::new([
        heading.into(),
        grouped(
            Column::new(rows)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .into(),
        ),
    ])
    .spacing(10.0)
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

fn matching_notices(query: &str) -> Vec<(usize, &'static Notice)> {
    // ECMAScript's whitespace expression includes the byte-order mark but
    // excludes NEXT LINE. Otherwise it agrees with Rust's is_whitespace.
    let lower = query.to_lowercase();
    let terms: Vec<_> = lower
        .split(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{0085}'))
        .filter(|term| !term.is_empty())
        .collect();
    NOTICES
        .iter()
        .enumerate()
        .filter(|(_, notice)| {
            if terms.is_empty() {
                return true;
            }
            let metadata = format!(
                "{} {} {} {}",
                notice.name,
                notice.version.unwrap_or_default(),
                notice.license,
                notice.bundle,
            )
            .to_lowercase();
            terms.iter().all(|term| metadata.contains(term))
        })
        .collect()
}

fn header_item(child: Widget, height: f32) -> Widget {
    SizedBox::new()
        .height(height)
        .child(
            Row::new([child])
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Center),
        )
        .into()
}

fn search_action(search: LicenseSearch, open: bool, compact: bool, count: &str) -> Widget {
    let mut children = Vec::new();
    if !open || !compact {
        children.push(label(count, 12.0, 16.0, FontWeight::W400, MUTED));
    }
    let height = if open && compact { 32.0 } else { 28.0 };
    children.push(if open {
        search_field(search, compact, height)
    } else {
        RawTooltip::new(
            "Search licenses",
            Button::with_child(SEARCH_LABEL, icon("search", 12.0, MUTED))
                .style(
                    ButtonStyle::new()
                        .variant(ButtonVariant::Ghost)
                        .fixed_size(Size::new(20.0, 20.0))
                        .padding(EdgeInsets::ZERO)
                        .border_radius(4.0)
                        .background(Color::TRANSPARENT),
                )
                .on_click(move || search.set_open(true)),
        )
        .trigger_mode(RawTooltipTriggerMode::Manual)
        .enable_tap_to_dismiss(false)
        .prefer_below(false)
        .into()
    });
    header_item(
        Row::new(children)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .spacing(if open { 8.0 } else { 6.0 })
            .into(),
        height,
    )
}

fn search_field(search: LicenseSearch, compact: bool, height: f32) -> Widget {
    let width = if compact { 144.0 } else { 176.0 };
    let on_change = search.clone();
    let on_escape = search.clone();
    // A raw editor cannot acquire focus on reveal at this framework pin (IG-001).
    // The prefix is inside the editor's bounds, so clicking it still focuses the input.
    let editor: Widget = EditableText::new(search.controller.clone())
        .size(Size::new(width, height))
        .content_padding(EdgeInsets::from_ltrb(25.0, 0.0, 9.0, 0.0))
        .placeholder("Search licenses")
        .placeholder_color(MUTED)
        .cursor_color(INK)
        .style(interface(
            if compact { 16.0 } else { 14.0 },
            if compact { 30.0 } else { 26.0 },
            FontWeight::W400,
            INK,
        ))
        .focused_border(Border::new(1.0, RING), 8.0)
        // HTML search Enter does not vacate the field or submit a form here.
        .on_editing_complete(|| {})
        .on_changed(move |_| on_change.invalidate())
        .into();
    let editor = editor
        .semantics(ExplicitSemantics::new(SemanticRole::GenericContainer).label(SEARCH_LABEL));
    let appearance = Container::new()
        .width(width)
        .height(height)
        .decoration(
            BoxDecoration::new()
                .color(PAGE)
                .border(Border::new(1.0, INPUT))
                .border_radius(BorderRadius::circular(8.0)),
        )
        .child(Stack::new([
            Positioned::new(icon("search", 12.0, Color::rgba(24, 24, 27, 204)))
                .left(8.0)
                .top((height - 12.0) / 2.0),
            Positioned::fill(editor),
        ]));
    KeyboardListener::new(
        GestureDetector::new(appearance).callbacks(GestureCallbacks {
            on_focus_within: Some(Rc::new(move |focused| {
                if !focused && search.controller.text().is_empty() {
                    search.set_open(false);
                }
            })),
            ..GestureCallbacks::default()
        }),
    )
    .on_key(move |event| {
        if event.state.is_down() && event.key == KeyboardKey::Named(NamedKey::Escape) {
            on_escape.reset();
            return true;
        }
        false
    })
    .include_semantics(false)
    .into()
}

fn notice_row(notice: &Notice, compact: bool, open: bool, click: impl Fn() + 'static) -> Widget {
    let size = if compact { 16.0 } else { 14.0 };
    let height = if compact { 40.0 } else { 36.0 };
    let mut name: Vec<Widget> = vec![
        Expanded::new(
            Text::new(notice.name)
                .style(interface(
                    size,
                    if compact { 24.0 } else { 20.0 },
                    FontWeight::W500,
                    INK,
                ))
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        )
        .into(),
    ];
    if let Some(version) = notice.version {
        name.push(
            Text::new(version)
                .style(mono(12.0, 16.0, FontWeight::W400, MUTED))
                .into(),
        );
    }
    let name = Row::new(name)
        .spacing(8.0)
        .cross_axis_alignment(CrossAxisAlignment::Center);
    let chevron = icon("chevron-down", 14.0, MUTED);
    let chevron = if open {
        chevron
    } else {
        Transform::rotation(-std::f32::consts::FRAC_PI_2, chevron).into()
    };
    let content = Row::new([
        chevron,
        Expanded::new(name).into(),
        Text::new(format!("{} · {}", notice.license, notice.bundle))
            .style(interface(12.0, 16.0, FontWeight::W400, MUTED))
            .max_lines(Some(1))
            .overflow(TextOverflow::Ellipsis)
            .into(),
    ])
    .spacing(10.0)
    .cross_axis_alignment(CrossAxisAlignment::Center);
    Widget::from(
        Button::with_child(notice.name, content.into())
            .style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .height(height)
                    .border_radius(0.0)
                    .background(Color::TRANSPARENT)
                    .padding(EdgeInsets::symmetric(inset(compact), 0.0)),
            )
            .on_click(click),
    )
    .semantics(
        ExplicitSemantics::new(SemanticRole::Button)
            .label(notice.name)
            .state(SemanticState {
                focusable: true,
                enabled: true,
                expanded: Some(open),
                ..SemanticState::default()
            })
            .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
    )
}

fn inset(compact: bool) -> f32 {
    if compact { 12.0 } else { 16.0 }
}

// SettingsGroup.tsx:18–25, settingsLayout.tsx:235–262,526–572.
fn section(title: &str, x: f32, content: Widget) -> Widget {
    let heading = Container::new()
        .height(28.0)
        .padding(EdgeInsets::symmetric(x, 0.0))
        .alignment(Alignment::CENTER_LEFT)
        .child(label(title, 16.0, 24.0, FontWeight::W600, INK));
    Column::new([heading.into(), content])
        .spacing(10.0)
        .main_axis_size(MainAxisSize::Min)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
}

fn grouped(child: Widget) -> Widget {
    let radius = BorderRadius::all(Radius::circular(12.0));
    Container::new()
        .decoration(
            BoxDecoration::new()
                .color(Color::rgba(255, 255, 255, 102))
                .border(Border::new(1.0, RULE))
                .border_radius(radius)
                .box_shadow([BoxShadow::new(
                    Color::rgba(0, 0, 0, 13),
                    Offset::new(0.0, 1.0),
                    2.0,
                    0.0,
                )]),
        )
        .child(child)
        .into()
}

fn setting_row(
    x: f32,
    title: Widget,
    description: &'static str,
    control: Option<Widget>,
) -> Widget {
    let descriptions: Widget = Column::new([
        title,
        label(description, 12.0, 18.0, FontWeight::W400, MUTED),
    ])
    .spacing(4.0)
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .into();
    Container::new()
        .padding(EdgeInsets::symmetric(x, 12.0))
        .child(LayoutBuilder::new(move |_, layout| match &control {
            None => descriptions.clone(),
            Some(control) if layout.max_width() >= 512.0 => {
                Row::new([Expanded::new(descriptions.clone()).into(), control.clone()])
                    .spacing(32.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .into()
            }
            Some(control) => Column::new([descriptions.clone(), control.clone()])
                .spacing(12.0)
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .into(),
        }))
        .into()
}

fn divider() -> Widget {
    Container::new()
        .height(1.0)
        .color(Color::rgba(228, 228, 231, 128))
        .into()
}

fn label(text: &str, size: f32, height: f32, weight: FontWeight, color: Color) -> Widget {
    Text::new(text)
        .style(interface(size, height, weight, color))
        .into()
}

fn interface(size: f32, height: f32, weight: FontWeight, color: Color) -> TextStyle {
    typography::interface()
        .font_size(size)
        .line_height_absolute(height)
        .font_weight(weight)
        .color(color)
}

fn mono(size: f32, height: f32, weight: FontWeight, color: Color) -> TextStyle {
    TextStyle::new()
        .font_family("Liberation Mono")
        .fallback_family(FontFamily::Monospace)
        .font_size(size)
        .line_height_absolute(height)
        .font_weight(weight)
        .color(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indices(query: &str) -> Vec<usize> {
        matching_notices(query)
            .into_iter()
            .map(|(index, _)| index)
            .collect()
    }

    #[test]
    fn filter_uses_all_terms_from_metadata_in_source_order() {
        assert_eq!(indices(""), [0, 1, 2]);
        assert_eq!(indices("  \u{feff}  "), [0, 1, 2]);
        assert_eq!(indices("AsSeTs"), [0, 1]);
        assert_eq!(indices("  MIT "), [1, 2]);
        assert_eq!(indices("ISC\u{feff}feather"), [1]);
        assert_eq!(indices("0.1.11 CODE desktop"), [2]);
        assert!(indices("assets 0.1.11").is_empty());
        assert!(indices("permission is hereby granted").is_empty());
    }
}
