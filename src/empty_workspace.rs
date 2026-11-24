//! Native empty workspace, matched against the pinned Linux desktop reference.

use incular::{
    controls::{Button, ButtonStyle},
    prelude::*,
};

pub(crate) const PAGE: Color = Color::rgba(252, 252, 252, 255);
const SIDEBAR: Color = Color::rgba(249, 249, 249, 255);
pub(crate) const RULE: Color = Color::rgba(228, 228, 231, 255);
pub(crate) const INK: Color = Color::rgba(38, 38, 38, 255);
pub(crate) const MUTED: Color = Color::rgba(113, 113, 122, 255);
const FAINT: Color = Color::rgba(161, 161, 170, 255);
const BLUE: Color = Color::rgba(27, 78, 216, 255);
pub(crate) const WHITE: Color = Color::rgba(255, 255, 255, 255);

pub fn view(scratch_available: bool, open_scratch: impl Fn() + 'static) -> Widget {
    let mut buttons = vec![project_button("＋ Add project", 116.0, 28.0, BLUE, WHITE)];
    if scratch_available {
        buttons.push(SizedBox::new().width(8.0).into());
        buttons.push(
            Button::new("Start without a project")
                .style(style(28.0, WHITE, INK).fixed_size(Size::new(186.0, 28.0)))
                .on_click(open_scratch)
                .into(),
        );
    }
    let hero = Column::new(vec![
        label("What should we work on?", 30.0, FontWeight::W600, INK),
        space(4.0),
        label(
            if scratch_available {
                "Add a project, or start without one."
            } else {
                "Add a project to start your first thread."
            },
            14.0,
            FontWeight::W400,
            MUTED,
        ),
        space(24.0),
        Row::new(buttons)
            .main_axis_alignment(MainAxisAlignment::Center)
            .into(),
    ])
    .main_axis_size(MainAxisSize::Min);
    Row::new(vec![
        sidebar(false),
        Container::new().width(1.0).color(RULE).into(),
        Expanded::new(
            Container::new().color(PAGE).child(
                Column::new(vec![
                    space(40.0),
                    Expanded::new(Container::new().alignment(Alignment::CENTER).child(hero)).into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ),
        )
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

pub(crate) fn sidebar(scratch: bool) -> Widget {
    let empty_caption = if scratch {
        "No threads yet"
    } else {
        "No projects yet"
    };
    Container::new()
        .width(255.0)
        .color(SIDEBAR)
        .child(Column::new(vec![
            line("◧    Butler   ‹   ›", 40.0, INK),
            line("⌕   Search", 48.0, MUTED),
            space(24.0),
            label(empty_caption, 12.0, FontWeight::W400, FAINT),
            space(8.0),
            if scratch {
                space(26.0)
            } else {
                project_button("＋ Add project", 96.0, 26.0, SIDEBAR, MUTED)
            },
            Spacer::new().into(),
            line("⚙     ⑂     ⊙     ▥", 48.0, MUTED),
        ]))
        .into()
}

fn line(text: &str, height: f32, color: Color) -> Widget {
    Container::new()
        .width(255.0)
        .height(height)
        .padding(EdgeInsets::symmetric(16.0, 0.0))
        .alignment(Alignment::CENTER_LEFT)
        .child(label(text, 14.0, FontWeight::W500, color))
        .into()
}

fn project_button(text: &str, width: f32, height: f32, background: Color, ink: Color) -> Widget {
    // The project picker isn't connected yet. This is not an actionable control.
    let decoration = BoxDecoration::new()
        .color(background)
        .border_radius(BorderRadius::all(Radius::circular(8.0)));
    Container::new()
        .width(width)
        .height(height)
        .alignment(Alignment::CENTER)
        .decoration(decoration)
        .child(label(
            text,
            if width < 100.0 { 11.0 } else { 14.0 },
            FontWeight::W500,
            ink,
        ))
        .into()
}

fn style(height: f32, background: Color, ink: Color) -> ButtonStyle {
    ButtonStyle::new()
        .height(height)
        .background(background)
        .foreground(ink)
        .padding(EdgeInsets::symmetric(8.0, 0.0))
        .text_style(
            TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(14.0),
        )
        .border_radius(8.0)
}

pub(crate) fn label(text: &str, size: f32, weight: FontWeight, color: Color) -> Widget {
    Text::new(text)
        .style(
            TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(size)
                .font_weight(weight)
                .color(color),
        )
        .into()
}

pub(crate) fn space(height: f32) -> Widget {
    SizedBox::new().height(height).into()
}
