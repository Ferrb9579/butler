//! The local saved-thread Find popover; T3 v0.1.11 ThreadFindBar.tsx:83–151.

use crate::{
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{
    prelude::*,
    widgets::{BackdropFilter, TextInputFormatter, internal::ExplicitSemantics},
};

const FIELD: &str = "Find in thread";
const HEIGHT: f32 = 36.0;
const RADIUS: f32 = 16.0;
const MAX_WIDTH: f32 = 280.0;
const INK: Color = Color::rgba(24, 24, 27, 255);
const MUTED: Color = Color::rgba(113, 113, 123, 255);
const DESTRUCTIVE: Color = Color::rgba(239, 68, 68, 255);
const BORDER: Color = Color::rgba(24, 24, 27, 26);
// The original dropdown-glass mixes 18% of the opaque surface into 80% glass.
const GLASS: Color = Color::rgba(255, 255, 255, 213);

/// Builds a 36px bar at the given available width (up to 280px). The caller
/// positions it in the pane and owns its controller, query, and active hit.
/// True moves forward; false moves backward.
pub fn view(
    controller: TextEditingController,
    count: String,
    can_step: bool,
    width: f32,
    on_query: impl Fn(&str) + 'static,
    on_step: impl Fn(bool) + Clone + 'static,
    on_close: impl Fn() + Clone + 'static,
) -> Widget {
    let step_key = on_step.clone();
    let input_keys = controller.clone();
    let escape_keys = controller.clone();
    let input = Widget::from(
        EditableText::new(controller)
            .size(Size::new(0.0, 26.0))
            .content_padding(EdgeInsets::from_ltrb(9.0, 0.0, 6.0, 0.0))
            .placeholder(FIELD)
            .placeholder_color(MUTED)
            .cursor_color(INK)
            .style(
                typography::interface()
                    .font_size(14.0)
                    .line_height_absolute(20.0)
                    .font_weight(FontWeight::W400)
                    .color(INK),
            )
            .input_formatter(FindLengthLimit)
            // Step is navigation, not form submission that would unfocus the input.
            .on_editing_complete(|| {})
            .on_changed(on_query),
    )
    .semantics(ExplicitSemantics::new(SemanticRole::SearchField).label(FIELD));
    let input = KeyboardListener::new(input)
        .include_semantics(false)
        .on_key(move |event| {
            if !event.state.is_down()
                || event.key != KeyboardKey::Named(NamedKey::Enter)
                || composing(&input_keys)
            {
                return false;
            }
            if can_step {
                step_key(!event.modifiers.shift());
            }
            true
        });

    let count_label = ExplicitSemantics::new(SemanticRole::Text)
        .label(count.clone())
        .state(SemanticState {
            live_region: true,
            ..SemanticState::default()
        });
    let count_color = if count == "0/0" { DESTRUCTIVE } else { MUTED };
    let count = Widget::from(
        Container::new()
            .constraints(Constraints::new(40.0, f32::INFINITY, 0.0, f32::INFINITY))
            .alignment(Alignment::CENTER)
            .child(
                Text::new(count)
                    .style(
                        typography::interface()
                            .font_size(12.0)
                            .line_height_absolute(16.0)
                            .font_features("\"tnum\" 1")
                            .color(count_color),
                    )
                    .max_lines(Some(1)),
            ),
    )
    .semantics(count_label);

    let previous = on_step.clone();
    let close_help = on_close.clone();
    let controls = Row::new(vec![
        count,
        control("Previous match", "chevron-up", can_step, move || {
            previous(false)
        }),
        control("Next match", "chevron-down", can_step, move || {
            on_step(true)
        }),
        control("Close thread find", "x", true, on_close),
    ])
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .spacing(2.0);
    let body = Container::new()
        .width(width.clamp(0.0, MAX_WIDTH))
        .height(HEIGHT)
        .padding(EdgeInsets::from_ltrb(0.0, 0.0, 4.0, 0.0))
        .decoration(
            BoxDecoration::new()
                .color(GLASS)
                .border(Border::new(1.0, BORDER))
                .border_radius(BorderRadius::circular(RADIUS)),
        )
        .child(
            Row::new([Widget::from(Expanded::new(input)), controls.into()])
                .cross_axis_alignment(CrossAxisAlignment::Center),
        );
    let bar = ClipRRect::new(RADIUS, BackdropFilter::blur(12.0, body));
    KeyboardListener::new(bar)
        .include_semantics(false)
        .on_key(move |event| {
            if event.state.is_down()
                && event.key == KeyboardKey::Named(NamedKey::Escape)
                && !composing(&escape_keys)
            {
                close_help();
                return true;
            }
            false
        })
        .into()
}

fn composing(controller: &TextEditingController) -> bool {
    controller.preedit().is_some() || controller.composing().is_some()
}

fn control(
    label: &'static str,
    glyph: &'static str,
    enabled: bool,
    click: impl Fn() + 'static,
) -> Widget {
    Button::with_child(label, icon(glyph, 16.0, MUTED))
        .style(
            ButtonStyle::new()
                .variant(ButtonVariant::Ghost)
                .fixed_size(Size::new(24.0, 24.0))
                .padding(EdgeInsets::ZERO)
                .border_radius(12.0)
                .background(Color::TRANSPARENT),
        )
        .enabled(enabled)
        .on_click(click)
        .into()
}

// HTML's maxlength counts UTF-16 code units, not bytes or grapheme clusters.
struct FindLengthLimit;

impl TextInputFormatter for FindLengthLimit {
    fn format_edit_update(
        &self,
        old_value: &TextEditingValue,
        new_value: &TextEditingValue,
    ) -> TextEditingValue {
        if new_value.composing.is_some() || new_value.text.len() <= 200 {
            return new_value.clone();
        }
        let units = new_value.text.encode_utf16().count();
        if units <= 200 || units < old_value.text.encode_utf16().count() {
            return new_value.clone();
        }

        let replacement = old_value
            .composing
            .unwrap_or_else(|| old_value.selection.range())
            .clamp_to(&old_value.text);
        let before = &old_value.text[..replacement.start];
        let after = &old_value.text[replacement.end..];
        let Some(inserted) = new_value
            .text
            .strip_prefix(before)
            .and_then(|rest| rest.strip_suffix(after))
        else {
            // Reject an overlong edit whose unaffected text cannot be verified.
            return old_value.clone();
        };
        let available =
            200_usize.saturating_sub(before.encode_utf16().count() + after.encode_utf16().count());
        let mut used = 0;
        let mut end = inserted.len();
        for (at, character) in inserted.char_indices() {
            used += character.len_utf16();
            if used > available {
                end = at;
                break;
            }
        }
        if end == 0 {
            return old_value.clone();
        }
        let text = format!("{before}{}{after}", &inserted[..end]);
        TextEditingValue::new(text).with_selection(TextSelection::collapsed(before.len() + end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_in_both_directions_and_leaves_ime_navigation_alone() {
        use incular::{
            core::{ImeEvent, InputEvent, KeyboardEvent, Modifiers, PointerPhase},
            runtime::Runtime,
        };
        use std::{
            cell::{Cell, RefCell},
            rc::Rc,
        };

        let queried = Rc::new(RefCell::new(String::new()));
        let moves = Rc::new(RefCell::new(Vec::new()));
        let closed = Rc::new(Cell::new(0));
        let controller = TextEditingController::new();
        let bar = view(
            controller.clone(),
            "1/2".into(),
            true,
            280.0,
            {
                let queried = queried.clone();
                move |value| {
                    queried.replace(value.to_owned());
                }
            },
            {
                let moves = moves.clone();
                move |direction| moves.borrow_mut().push(direction)
            },
            {
                let closed = closed.clone();
                move || closed.set(closed.get() + 1)
            },
        );
        let mut runtime =
            Runtime::new(Directionality::new(TextDirection::Ltr, bar).into()).unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(280.0, 36.0)))
            .unwrap();
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = runtime.handle_input(InputEvent::Pointer {
                phase,
                position: Offset::new(20.0, 15.0),
            });
        }
        let _ = runtime.handle_input(InputEvent::Text("find me".into()));
        assert_eq!(&*queried.borrow(), "find me");
        let key = |key, code, modifiers| {
            let mut event = KeyboardEvent::key_down(KeyboardKey::Named(key), code);
            event.modifiers = modifiers;
            InputEvent::Key(event)
        };
        let enter = || key(NamedKey::Enter, Code::Enter, Modifiers::empty());
        let escape = || key(NamedKey::Escape, Code::Escape, Modifiers::empty());
        let _ = runtime.handle_input(enter());
        let _ = runtime.handle_input(key(NamedKey::Enter, Code::Enter, Modifiers::SHIFT));
        assert_eq!(&*moves.borrow(), &[true, false]);
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
            text: "世界".into(),
            selection: None,
        }));
        assert!(controller.preedit().is_some());
        let _ = runtime.handle_input(escape());
        assert_eq!(closed.get(), 0);
        let _ = runtime.handle_input(enter());
        assert_eq!(&*moves.borrow(), &[true, false]);
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("世界".into())));
        let _ = runtime.handle_input(escape());
        assert_eq!(closed.get(), 1);
    }

    #[test]
    fn find_input_limits_utf16_without_cutting_a_scalar() {
        let old = TextEditingValue::empty();
        let text = format!("{}🙂z", "a".repeat(199));
        let input = TextEditingValue::new(text);
        let limited = FindLengthLimit.format_edit_update(&old, &input);
        assert_eq!(limited.text, "a".repeat(199));
        assert_eq!(limited.selection, TextSelection::collapsed(199));

        let old = TextEditingValue::new(format!("{}tail", "x".repeat(194)))
            .with_selection(TextSelection::collapsed(194));
        let input = TextEditingValue::new(format!("{}🙂extra tail", "x".repeat(194)));
        // An edit with an exact unchanged suffix preserves it.
        let kept = FindLengthLimit.format_edit_update(&old, &input);
        assert_eq!(kept.text, format!("{}🙂tail", "x".repeat(194)));
        assert_eq!(kept.selection, TextSelection::collapsed(198));

        let text = "🙂".repeat(100);
        let input = TextEditingValue::new(&text);
        assert_eq!(FindLengthLimit.format_edit_update(&old, &input).text, text);
    }
}
