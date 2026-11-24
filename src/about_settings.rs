//! Offline Settings bodies, ported from the pinned T3 Code v0.1.11 source.

use crate::{
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{prelude::*, widgets::internal::ExplicitSemantics};

const INK: Color = Color::rgba(24, 24, 27, 255);
const MUTED: Color = Color::rgba(113, 113, 123, 255);
const RULE: Color = Color::rgba(228, 228, 231, 153);
const NOTICE_INK: Color = Color::rgba(24, 24, 27, 204);
// 76ch at 12px, using Liberation Mono's 0.6em digit advance.
const NOTICE_MAX_WIDTH: f32 = 76.0 * 12.0 * 0.6;

struct Notice {
    name: &'static str,
    version: Option<&'static str>,
    license: &'static str,
    text: &'static str,
}

const NOTICES: [Notice; 3] = [
    Notice {
        name: "Inter",
        version: None,
        license: "OFL-1.1",
        text: include_str!("../LICENSES/Inter-OFL-1.1.txt"),
    },
    Notice {
        name: "Lucide and Feather",
        version: None,
        license: "ISC · MIT",
        text: include_str!("../LICENSES/Lucide-ISC-and-Feather-MIT.txt"),
    },
    Notice {
        name: "T3 Code",
        version: Some("0.1.11"),
        license: "MIT",
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

// OpenSourceLicenses.tsx:78–154,249–312. Bundled files replace the web manifest;
// this is deliberately not represented as a complete dependency inventory.
pub fn licenses(
    compact: bool,
    expanded: Option<usize>,
    on_expanded: impl Fn(Option<usize>) + Clone + 'static,
) -> Widget {
    let x = inset(compact);
    let heading = Container::new()
        .height(28.0)
        .padding(EdgeInsets::symmetric(x, 0.0))
        .child(
            Row::new([
                Expanded::new(label(
                    "Third-party notices",
                    16.0,
                    24.0,
                    FontWeight::W600,
                    INK,
                ))
                .into(),
                label("3 notices", 12.0, 16.0, FontWeight::W400, MUTED),
            ])
            .spacing(16.0)
            .cross_axis_alignment(CrossAxisAlignment::Center),
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
    for (index, notice) in NOTICES.iter().enumerate() {
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
        label(notice.license, 12.0, 16.0, FontWeight::W400, MUTED),
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
