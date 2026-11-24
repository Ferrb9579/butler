use crate::settings_icons::icon;
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use incular::{
    prelude::*,
    widgets::{
        MouseCursor,
        internal::{
            ActionInteractionController, ActionPolicy, ActionSplashPolicy, ActionSurface,
            ExplicitSemantics,
        },
    },
};
use std::{f32::consts::PI, time::Duration};

const BACKGROUND: Color = Color::rgba(252, 252, 252, 255);
const INK: Color = Color::rgba(16, 19, 24, 255);
const MUTED: Color = Color::rgba(105, 112, 128, 255);
const RULE: Color = Color::rgba(238, 238, 240, 255);
const ACCENT: Color = Color::rgba(107, 140, 228, 255);
const AGENT_TRIGGER: &str = "Set up an agent";
const TRIGGER_INK: Color = Color::rgba(24, 24, 27, 255);
const TRIGGER_MUTED: Color = Color::rgba(113, 113, 123, 255);
const TRIGGER_RING: Color = Color::rgba(27, 78, 216, 255);
const AGENTS_LANE_START: f32 = 96.0 + 40.0;

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
                gap(16.0),
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

fn row_icon(mark: Widget) -> Widget {
    Container::new()
        .width(20.0)
        .height(20.0)
        .alignment(Alignment::CENTER)
        .child(mark)
        .into()
}

fn agent_trigger(expanded: bool, agents_open: Signal<bool>) -> Widget {
    // WelcomePage.tsx RowIcon and CollapsibleTrigger: no inset or hover fill
    // in the no-agent state. The only exterior chrome is the keyboard ring.
    let plus = row_icon(icon("plus", 16.0, TRIGGER_MUTED));
    let label = Text::new(AGENT_TRIGGER).style(
        crate::typography::interface()
            .font_size(14.0)
            .line_height_absolute(20.0)
            .font_weight(FontWeight::W400)
            .color(TRIGGER_INK),
    );
    let down = icon("chevron-down", 16.0, TRIGGER_MUTED);
    let chevron = if expanded {
        Widget::from(Transform::rotation(PI, down))
    } else {
        down
    };
    let row = Widget::from(
        Row::new([plus, Expanded::new(label).into(), chevron])
            .spacing(12.0)
            .cross_axis_alignment(CrossAxisAlignment::Center),
    )
    .exclude_semantics();
    let state = ActionInteractionController::new();
    let interaction = state.clone();
    let surface = Widget::stateful_builder(interaction.revision(), move |_| {
        let mut frame = Container::new()
            .constraints(Constraints::new(0.0, f32::INFINITY, 40.0, f32::INFINITY))
            .child(row.clone());
        if interaction.state().focused {
            // Paint over the row rather than adding decoration padding.
            frame = frame.foreground_decoration(
                BoxDecoration::new()
                    .border(Border::new(2.0, TRIGGER_RING))
                    .border_radius(BorderRadius::circular(6.0)),
            );
        }
        frame.into()
    });
    Widget::from(
        ActionSurface::with_child(surface)
            .label(AGENT_TRIGGER)
            .interaction_controller(state)
            .mouse_cursor(MouseCursor::Click)
            .policy(ActionPolicy {
                feedback_enabled: false,
                splash: ActionSplashPolicy::None,
                transition_duration: Duration::ZERO,
            })
            .on_press(move || agents_open.update(|open| *open = !*open)),
    )
    .semantics(
        ExplicitSemantics::new(SemanticRole::Button)
            .label(AGENT_TRIGGER)
            .state(SemanticState {
                focusable: true,
                expanded: Some(expanded),
                ..SemanticState::default()
            })
            .actions([SemanticActionKind::Focus, SemanticActionKind::Activate]),
    )
}

fn agents_section(narrow: bool, expanded: bool, agents_open: Signal<bool>) -> Widget {
    let mut right = vec![agent_trigger(expanded, agents_open)];
    if expanded {
        // The reference panel's provider grid touches the 40px trigger.
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
        right.push(registry_row());
    }
    section(
        "Agents",
        narrow,
        AGENTS_LANE_START,
        Column::new(right)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .into(),
    )
}

fn registry_row() -> Widget {
    Container::new()
        .height(36.0)
        .child(
            Row::new([
                row_icon(crate::settings_icons::acp_registry_mark(16.0)),
                Expanded::new(text(
                    "More agents in the ACP Registry",
                    14.0,
                    FontWeight::W400,
                    INK,
                ))
                .into(),
                Button::new("Browse")
                    .style(button_style(24.0, Color::WHITE, INK))
                    .enabled(false)
                    .into(),
            ])
            .spacing(12.0)
            .cross_axis_alignment(CrossAxisAlignment::Center),
        )
        .into()
}

fn phone_section(narrow: bool) -> Widget {
    section(
        "Phone",
        narrow,
        138.0,
        setup_row(
            Container::new()
                .width(30.0)
                .alignment(Alignment::CENTER_LEFT)
                .child(icon("smartphone", 16.0, MUTED))
                .into(),
            "Use Butler on your phone",
            "Set up",
            0.0,
        ),
    )
}

fn section(label: &str, narrow: bool, lane_start: f32, body: Widget) -> Widget {
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
            Widget::from(SizedBox::new().width(lane_start).child(label)),
            Expanded::new(body).into(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .into()
    }
}

fn agent((name, action): (&str, &str)) -> Widget {
    setup_row(
        row_icon(crate::settings_icons::provider_brand(
            name, 16.0, INK, false,
        )),
        name,
        action,
        12.0,
    )
}

fn setup_row(leading: Widget, name: &str, action: &str, spacing: f32) -> Widget {
    Container::new()
        .height(36.0)
        .child(
            Row::new([
                leading,
                Expanded::new(text(name, 14.0, FontWeight::W400, INK)).into(),
                Button::new(action)
                    .style(button_style(24.0, Color::rgba(255, 255, 255, 255), INK))
                    .enabled(false)
                    .into(),
            ])
            .spacing(spacing),
        )
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
            .style(
                button_style(32.0, ACCENT, Color::rgba(255, 255, 255, 255))
                    .variant(ButtonVariant::Primary),
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular::{
        core::Transform as Matrix,
        rendering::{Brush, PaintCommand},
        runtime::Runtime,
    };

    #[test]
    fn registry_mark_paints_black_centered_in_its_existing_agent_lane() {
        let root = Directionality::new(
            TextDirection::Ltr,
            Align::new(
                Alignment::TOP_LEFT,
                section("Agents", false, AGENTS_LANE_START, registry_row()),
            ),
        );
        let mut runtime = Runtime::new(root.into()).unwrap();
        let (display, _) = runtime
            .run_frame(Constraints::tight(Size::new(720.0, 120.0)))
            .unwrap();
        let mut matrices = vec![Matrix::IDENTITY];
        let mut marks = Vec::new();
        for command in display.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    matrices.push(matrices.last().unwrap().then(*transform));
                }
                PaintCommand::PopTransform => {
                    matrices.pop();
                }
                PaintCommand::FillPath {
                    path,
                    brush: Brush::Solid(color),
                    fill_rule,
                } if *color == Color::BLACK => {
                    assert_eq!(*fill_rule, FillRule::NonZero);
                    marks.push(
                        matrices
                            .last()
                            .unwrap()
                            .transform_rect_bbox(path.bounds().unwrap()),
                    );
                }
                _ => {}
            }
        }
        let [mark] = marks.as_slice() else {
            panic!("expected one ACP mark, got {marks:?}");
        };
        // The Agents section's content starts at x136. The original wide
        // mark aspect-fits 16px with 2px on both sides of its 20px slot.
        assert!((mark.origin.x - 138.0).abs() < 0.15, "{mark:?}");
        assert!((mark.size.width - 16.0).abs() < 0.15, "{mark:?}");
        assert!(
            (mark.origin.y + mark.size.height / 2.0 - 18.0).abs() < 0.15,
            "{mark:?}"
        );
        assert!(
            (mark.size.height - 16.0 * 220.0 / 576.0).abs() < 0.15,
            "{mark:?}"
        );
    }

    #[test]
    fn desktop_agent_grid_uses_the_reference_lanes_and_shared_vertical_spacing() {
        crate::typography::register();
        let root = Directionality::new(
            TextDirection::Ltr,
            view("Local device", Signal::new(true), None, || {}),
        );
        let mut runtime = Runtime::new(root.into()).unwrap();
        let (display, _) = runtime
            .run_frame(Constraints::tight(Size::new(1280.0, 800.0)))
            .unwrap();

        let semantics = runtime.tree().semantics();
        let mut pending = semantics.roots().to_vec();
        let mut bounds = std::collections::HashMap::new();
        while let Some(id) = pending.pop() {
            let Some(node) = semantics.node(id) else {
                continue;
            };
            if let Some(label) = node.label.as_deref() {
                bounds.insert(label, node.bounds);
            }
            pending.extend(node.children.iter().copied());
        }
        let trigger = bounds[AGENT_TRIGGER];
        for (index, (left, right)) in [
            ("Antigravity", "Claude"),
            ("Codex", "Cursor"),
            ("fx", "Grok"),
            ("Kiro", "Muse Code"),
            ("OpenCode", "Pi"),
        ]
        .into_iter()
        .enumerate()
        {
            let top = trigger.origin.y + trigger.size.height + 36.0 * index as f32;
            let left = bounds[left];
            let right = bounds[right];
            assert_eq!(left.origin.x, 448.0);
            assert_eq!(right.origin.x, 756.0);
            assert_eq!(left.origin.y, right.origin.y);
            assert!(left.origin.y >= top && left.origin.y + left.size.height <= top + 36.0);
        }
        assert_eq!(bounds["More agents in the ACP Registry"].origin.x, 448.0);
        assert_eq!(bounds["Use Butler on your phone"].origin.x, 448.0);

        let mut matrices = vec![Matrix::IDENTITY];
        let mut divider_rows = Vec::new();
        let mut acp = None;
        let registry_top = trigger.origin.y + trigger.size.height + 5.0 * 36.0;
        for command in display.commands() {
            let divider = match command {
                PaintCommand::PushTransform { transform } => {
                    matrices.push(matrices.last().unwrap().then(*transform));
                    continue;
                }
                PaintCommand::PopTransform => {
                    matrices.pop();
                    continue;
                }
                PaintCommand::Rect { rect, color } if *color == RULE => rect,
                PaintCommand::RRect {
                    rrect,
                    brush: Brush::Solid(color),
                } if *color == RULE => &rrect.rect,
                PaintCommand::FillPath {
                    path,
                    brush: Brush::Solid(color),
                    ..
                } if *color == Color::BLACK => {
                    let rect = matrices
                        .last()
                        .unwrap()
                        .transform_rect_bbox(path.bounds().unwrap());
                    if rect.origin.y >= registry_top && rect.origin.y < registry_top + 36.0 {
                        acp = Some(rect);
                    }
                    continue;
                }
                _ => continue,
            };
            if divider.size == Size::new(720.0, 1.0) {
                divider_rows.push(matrices.last().unwrap().transform_point(divider.origin));
            }
        }
        // The default-font heading above the section can vary by platform.
        // Pin the actual offsets between its first rule, trigger and grid.
        assert_eq!(divider_rows[0].x, 280.0);
        assert_eq!(trigger.origin.y - divider_rows[0].y, 17.0);
        assert_eq!(
            divider_rows[1],
            Offset::new(280.0, registry_top + 36.0 + 16.0)
        );
        let mark = acp.expect("ACP Registry mark is painted in the agent section");
        assert!((mark.origin.x - 418.0).abs() < 0.15, "{mark:?}");
        assert!(
            (mark.origin.y + mark.size.height / 2.0 - (registry_top + 18.0)).abs() < 0.15,
            "{mark:?}"
        );
    }

    #[test]
    fn agent_trigger_uses_the_whole_lane_with_original_rotated_icons() {
        crate::typography::register();
        let near = |actual: f32, expected: f32| {
            assert!(
                (actual - expected).abs() < 0.1,
                "expected {expected}, got {actual}"
            );
        };
        for (width, narrow, expanded) in [(720.0, false, false), (402.0, true, true)] {
            let left = if narrow { 0.0 } else { AGENTS_LANE_START };
            let y = if narrow { 32.0 } else { 0.0 };
            let widget = section(
                "Agents",
                narrow,
                AGENTS_LANE_START,
                agent_trigger(expanded, Signal::new(expanded)),
            );
            let root =
                Directionality::new(TextDirection::Ltr, Align::new(Alignment::TOP_LEFT, widget));
            let mut runtime = Runtime::new(root.into()).unwrap();
            let (display, _) = runtime
                .run_frame(Constraints::tight(Size::new(width, 120.0)))
                .unwrap();

            let semantics = runtime.tree().semantics();
            let mut pending = semantics.roots().to_vec();
            let mut buttons = Vec::new();
            while let Some(id) = pending.pop() {
                let Some(node) = semantics.node(id) else {
                    continue;
                };
                if node.label.as_deref() == Some(AGENT_TRIGGER) {
                    buttons.push(node);
                }
                pending.extend(node.children.iter().copied());
            }
            assert_eq!(buttons.len(), 1);
            let button = buttons[0];
            assert_eq!(button.role, SemanticRole::Button);
            assert_eq!(button.state.expanded, Some(expanded));
            assert!(button.actions.contains(&SemanticActionKind::Activate));
            assert_eq!(
                button.bounds,
                Rect::from_origin_size(Offset::new(left, y), Size::new(width - left, 40.0))
            );

            let mut matrices = vec![Matrix::IDENTITY];
            let mut icons = Vec::new();
            let mut label = None;
            for command in display.commands() {
                match command {
                    PaintCommand::PushTransform { transform } => {
                        matrices.push(matrices.last().unwrap().then(*transform));
                    }
                    PaintCommand::PopTransform => {
                        matrices.pop();
                    }
                    PaintCommand::StrokePath {
                        path,
                        brush: Brush::Solid(color),
                        ..
                    } if *color == TRIGGER_MUTED => {
                        let transform = *matrices.last().unwrap();
                        let local = path.bounds().unwrap();
                        let path_box = transform.transform_rect_bbox(local);
                        let upper = transform.transform_point(local.origin);
                        let lower = transform
                            .transform_point(local.origin + Offset::new(0.0, local.size.height));
                        icons.push((path_box, upper, lower));
                    }
                    PaintCommand::GlyphRun { run, color } if *color == TRIGGER_INK => {
                        near(run.font_size, 14.0);
                        let at = matrices
                            .last()
                            .unwrap()
                            .transform_point(run.origin + run.glyphs[0].offset);
                        label.get_or_insert(at.x);
                    }
                    PaintCommand::Border { .. } => {
                        panic!("the untouched trigger has no border or ring")
                    }
                    _ => {}
                }
            }
            near(label.unwrap(), left + 32.0);
            assert_eq!(icons.len(), 2);
            let (plus, _, _) = icons[0];
            near(plus.origin.x, left + 2.0 + 5.0 * 16.0 / 24.0);
            near(plus.size.width, 14.0 * 16.0 / 24.0);
            near(plus.origin.y + plus.size.height / 2.0, y + 20.0);
            let (chevron, start, middle) = icons[1];
            near(chevron.origin.x, width - 12.0);
            near(chevron.size.width, 8.0);
            near(chevron.origin.y + chevron.size.height / 2.0, y + 20.0);
            assert_eq!(middle.y < start.y, expanded);
        }
    }
}
