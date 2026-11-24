use incular::{
    controls::{Button, ButtonStyle, ButtonVariant},
    prelude::*,
};

const BACKGROUND: Color = Color::rgba(252, 252, 252, 255);
const INK: Color = Color::rgba(16, 19, 24, 255);
const MUTED: Color = Color::rgba(105, 112, 128, 255);
const RULE: Color = Color::rgba(238, 238, 240, 255);
const ACCENT: Color = Color::rgba(107, 140, 228, 255);

pub fn view(
    device: &str,
    agents_open: Signal<bool>,
    error: Option<String>,
    complete: impl Fn() + Clone + 'static,
) -> Widget {
    let device = device.to_owned();
    let expanded = agents_open.get();
    Container::new()
        .color(BACKGROUND)
        .child(LayoutBuilder::new(move |_, constraints| {
            let content_width = (constraints.max_width() - 48.0).clamp(1.0, 720.0);
            let narrow = content_width < 480.0;
            let top = if narrow { 80.0 } else { 120.0 };
            let natural_height = top + if expanded && narrow { 780.0 } else { 600.0 };
            let height = constraints.max_height().max(natural_height);
            let mut sections = vec![
                gap(top),
                text("Welcome to Butler", 24.0, FontWeight::W600, INK),
                gap(6.0),
                text(
                    format!("Setting up {device}."),
                    14.0,
                    FontWeight::W400,
                    MUTED,
                ),
                gap(48.0),
                divider(),
                gap(18.0),
                agents_section(narrow, expanded, agents_open.clone()),
                gap(16.0),
                divider(),
                gap(18.0),
                phone_section(narrow),
                gap(18.0),
                divider(),
                gap(24.0),
                actions(narrow, complete.clone()),
            ];
            if let Some(error) = &error {
                sections.extend([
                    gap(10.0),
                    text(error, 13.0, FontWeight::W400, Color::rgba(160, 33, 33, 255)),
                ]);
            }
            sections.push(Spacer::new().into());
            sections.push(gap(32.0));
            let body = Container::new()
                .width(content_width)
                .height(height)
                .child(Column::new(sections).cross_axis_alignment(CrossAxisAlignment::Stretch));
            SingleChildScrollView::new(
                Container::new()
                    .alignment(Alignment::TOP_CENTER)
                    .child(body),
            )
            .into()
        }))
        .into()
}

fn agents_section(narrow: bool, expanded: bool, agents_open: Signal<bool>) -> Widget {
    let title = Button::new(if expanded {
        "+   Set up an agent   ▴"
    } else {
        "+   Set up an agent   ▾"
    })
    .style(button_style(32.0, BACKGROUND, INK).variant(ButtonVariant::Ghost))
    .on_click(move || agents_open.update(|open| *open = !*open));
    let mut right: Vec<Widget> = vec![
        Container::new()
            .alignment(Alignment::CENTER_LEFT)
            .child(title)
            .into(),
    ];
    if expanded {
        right.push(gap(6.0));
        let left = [
            ("Antigravity", "Turn on"),
            ("Codex", "Set up"),
            ("fx", "Turn on"),
            ("Kiro", "Turn on"),
            ("OpenCode", "Turn on"),
        ];
        let other = [
            ("Claude", "Install"),
            ("Cursor", "Turn on"),
            ("Grok", "Turn on"),
            ("Muse Code", "Turn on"),
            ("Pi", "Turn on"),
        ];
        if narrow {
            right.extend(left.into_iter().chain(other).map(agent));
        } else {
            for (left, right_entry) in left.into_iter().zip(other) {
                right.push(
                    Row::new(vec![
                        Widget::from(Expanded::new(agent(left))),
                        SizedBox::new().width(32.0).into(),
                        Expanded::new(agent(right_entry)).into(),
                    ])
                    .into(),
                );
            }
        }
        right.push(agent(("More agents in the ACP Registry", "Browse")));
    }
    section(
        "Agents",
        narrow,
        Column::new(right)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into(),
    )
}

fn phone_section(narrow: bool) -> Widget {
    section(
        "Phone",
        narrow,
        agent(("Use Butler on your phone", "Set up")),
    )
}

fn section(label: &str, narrow: bool, body: Widget) -> Widget {
    let label = Container::new()
        .height(32.0)
        .alignment(Alignment::CENTER_LEFT)
        .child(text(label, 14.0, FontWeight::W400, MUTED));
    if narrow {
        Column::new(vec![label.into(), body])
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into()
    } else {
        Row::new(vec![
            Widget::from(SizedBox::new().width(138.0).child(label)),
            Expanded::new(body).into(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .into()
    }
}

fn agent((name, action): (&str, &str)) -> Widget {
    Container::new()
        .height(36.0)
        .child(Row::new(vec![
            Widget::from(SizedBox::new().width(30.0)),
            Expanded::new(text(name, 14.0, FontWeight::W400, INK)).into(),
            Button::new(action)
                .style(button_style(24.0, Color::rgba(255, 255, 255, 255), INK))
                .enabled(false)
                .into(),
        ]))
        .into()
}

fn actions(narrow: bool, complete: impl Fn() + 'static) -> Widget {
    let bypass: Widget = Button::new("Start without an agent")
        .style(button_style(32.0, BACKGROUND, MUTED).variant(ButtonVariant::Ghost))
        .on_click(complete)
        .into();
    let start: Widget = Row::new(vec![
        text("Set up one agent to start.", 12.0, FontWeight::W400, MUTED),
        SizedBox::new().width(12.0).into(),
        Button::new("Start")
            .style(button_style(32.0, ACCENT, Color::rgba(255, 255, 255, 255)))
            .enabled(false)
            .into(),
    ])
    .main_axis_size(MainAxisSize::Min)
    .into();
    if narrow {
        Column::new(vec![bypass, gap(8.0), start])
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .into()
    } else {
        Row::new(vec![bypass, Spacer::new().into(), start]).into()
    }
}

fn text(value: impl Into<String>, size: f32, weight: FontWeight, color: Color) -> Widget {
    Text::new(value.into())
        .style(
            TextStyle::new()
                .font_size(size)
                .font_weight(weight)
                .color(color),
        )
        .into()
}

fn button_style(height: f32, background: Color, foreground: Color) -> ButtonStyle {
    ButtonStyle::new()
        .height(height)
        .background(background)
        .foreground(foreground)
        .padding(EdgeInsets::symmetric(
            if height <= 24.0 { 7.0 } else { 12.0 },
            0.0,
        ))
        .text_style(TextStyle::new().font_size(if height <= 24.0 { 12.0 } else { 14.0 }))
        .border_radius(8.0)
}

fn gap(size: f32) -> Widget {
    SizedBox::new().height(size).into()
}

fn divider() -> Widget {
    Container::new().height(1.0).color(RULE).into()
}
