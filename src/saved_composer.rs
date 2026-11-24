//! Local, unsent saved-thread composer. T3 v0.1.11 ChatView.tsx,
//! ComposerSurface.tsx, ChatComposer.tsx and ComposerPrimaryActions.tsx.

use crate::{settings_icons::compose_send, typography};
use incular::{
    prelude::*,
    widgets::{BackdropFilter, TextInputFormatter, internal::ActionSurface},
};

pub const BOTTOM_INSET: f32 = 20.0;
const PLACEHOLDER: &str = "Enable a provider in Settings to send a message";
const INK: Color = Color::rgba(24, 24, 27, 255);
const MUTED: Color = Color::rgba(113, 113, 123, 255);
const SURFACE: Color = Color::rgba(255, 255, 255, 204);
const DISABLED_ACTION: Color = Color::rgba(24, 24, 27, 31);
const OUTLINE: Color = Color::rgba(0, 0, 0, 20);
const RADIUS: f32 = 16.0;

/// The caller owns this saved thread's unsent text controller and places the
/// dock in the transcript lane. No submission or provider operation exists.
pub fn view(
    thread_title: &str,
    controller: TextEditingController,
    changed: impl Fn(&str) + 'static,
) -> Widget {
    let editor = Widget::from(
        EditableText::new(controller)
            .on_changed(changed)
            .input_formatter(DraftLength)
            .placeholder(PLACEHOLDER)
            .placeholder_color(MUTED)
            .cursor_color(INK)
            .input_type(TextInputTypeHint::Multiline)
            .input_action(TextInputActionHint::Newline)
            .min_lines(Some(2))
            .max_lines(Some(9))
            .style(
                typography::interface()
                    .font_size(14.0)
                    .line_height_absolute(22.75)
                    .font_weight(FontWeight::W400)
                    .color(INK),
            ),
    )
    .accessibility_label(format!("Message {thread_title}"))
    .accessibility_description("Local draft. Sending is unavailable without a provider.");
    let body = Container::new()
        .padding(EdgeInsets::from_ltrb(16.0, 16.0, 16.0, 8.0))
        // Tiptap's 208px scroll height has 4px negative margins top/bottom.
        .child(
            Container::new()
                .constraints(Constraints::new(0.0, f32::INFINITY, 0.0, 200.0))
                .child(editor),
        );
    let send = Container::new()
        .width(32.0)
        .height(32.0)
        .alignment(Alignment::CENTER)
        .decoration(
            BoxDecoration::new()
                .color(DISABLED_ACTION)
                .border_radius(BorderRadius::circular(16.0)),
        )
        .child(compose_send(MUTED));
    let send: Widget = ActionSurface::with_child(send)
        .label("Send message")
        .size(Size::new(32.0, 32.0))
        .enabled(false)
        .into();
    let footer = Container::new()
        .padding(EdgeInsets::from_ltrb(8.0, 0.0, 8.0, 8.0))
        .child(
            Row::new([
                Spacer::new().into(),
                send.accessibility_description("Unavailable without a provider."),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center),
        );
    let card = Container::new()
        .padding(EdgeInsets::all(1.0))
        .decoration(
            BoxDecoration::new()
                .color(SURFACE)
                .border(Border::new(1.0, OUTLINE))
                .border_radius(BorderRadius::circular(RADIUS)),
        )
        .child(
            Column::new([body, footer])
                .main_axis_size(MainAxisSize::Min)
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
        );
    Container::new()
        .decoration(
            BoxDecoration::new()
                .border_radius(BorderRadius::circular(RADIUS))
                .box_shadow([BoxShadow::new(
                    Color::rgba(0, 0, 0, 13),
                    Offset::new(0.0, 1.0),
                    2.0,
                    0.0,
                )]),
        )
        .child(ClipRRect::new(RADIUS, BackdropFilter::blur(12.0, card)))
        .into()
}

struct DraftLength;

impl TextInputFormatter for DraftLength {
    fn format_edit_update(
        &self,
        old: &TextEditingValue,
        new: &TextEditingValue,
    ) -> TextEditingValue {
        if new.text.len() <= crate::composer_drafts::MAX_TEXT_BYTES {
            new.clone()
        } else {
            old.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular::{
        core::{InputEvent, KeyboardEvent, PointerPhase},
        rendering::PaintCommand,
        runtime::Runtime,
    };

    fn dock(controller: &TextEditingController) -> Runtime {
        Runtime::new(
            Directionality::new(
                TextDirection::Ltr,
                view("Release launch check", controller.clone(), |_| {}),
            )
            .into(),
        )
        .unwrap()
    }

    #[test]
    fn paints_the_rounded_card_and_source_sized_disabled_send() {
        let mut runtime = dock(&TextEditingController::new());
        let (display, _) = runtime
            .run_frame(Constraints::new(652.0, 652.0, 0.0, 360.0))
            .unwrap();
        let card = display.commands().iter().find_map(|command| match command {
            PaintCommand::Border { rrect, border } if border.width == 1.0 => Some(rrect),
            _ => None,
        });
        let card = card.expect("composer draws its 1px outline");
        assert_eq!(card.rect.size.width, 652.0);
        assert!((108.0..=114.0).contains(&card.rect.size.height), "{card:?}");
        assert_eq!(card.radii.top_left, RADIUS);
        assert!(display.commands().iter().any(|command| matches!(
            command,
            PaintCommand::RRect { rrect, .. } if rrect.rect.size == Size::new(32.0, 32.0)
                && rrect.radii.top_left == 16.0
        )));
    }

    #[test]
    fn overlarge_utf8_edit_is_rejected_without_truncating_existing_text() {
        let prior = TextEditingValue::new("Existing draft");
        let limit = crate::composer_drafts::MAX_TEXT_BYTES;
        let at_limit = TextEditingValue::new("é".repeat(limit / 2));
        assert_eq!(DraftLength.format_edit_update(&prior, &at_limit), at_limit);
        let too_large = TextEditingValue::new("é".repeat(limit / 2 + 1));
        assert_eq!(DraftLength.format_edit_update(&prior, &too_large), prior);
    }

    #[test]
    fn enter_edits_the_local_draft_and_send_cannot_consume_it() {
        let input = TextEditingController::new();
        let mut runtime = dock(&input);
        let bounds = Constraints::new(652.0, 652.0, 0.0, 360.0);
        runtime.run_frame(bounds).unwrap();
        let mut click = |position| {
            for phase in [PointerPhase::Down, PointerPhase::Up] {
                let _ = runtime.handle_input(InputEvent::Pointer { phase, position });
            }
        };
        click(Offset::new(25.0, 26.0));
        let _ = runtime.handle_input(InputEvent::Text("First line".into()));
        let _ = runtime.handle_input(InputEvent::Key(KeyboardEvent::key_down(
            KeyboardKey::Named(NamedKey::Enter),
            Code::Enter,
        )));
        let _ = runtime.handle_input(InputEvent::Text("Second line".into()));
        assert_eq!(input.text(), "First line\nSecond line");
        runtime.run_frame(bounds).unwrap();
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = runtime.handle_input(InputEvent::Pointer {
                phase,
                position: Offset::new(627.0, 87.0),
            });
        }
        assert_eq!(input.text(), "First line\nSecond line");
    }
}
