//! Title action and inline editor for an already-saved local thread.
use crate::{
    empty_workspace::{INK, MUTED, RULE, WHITE},
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{prelude::*, widgets::MenuController};
use std::rc::Rc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Editor {
    thread: Signal<Option<Uuid>>,
    input: TextEditingController,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            thread: Signal::new(None),
            input: TextEditingController::new(),
        }
    }
}

impl Editor {
    pub fn clear(&self) {
        self.thread.set(None);
    }

    pub fn view(&self, id: Uuid, title: &str, commit: impl Fn(Uuid, &str) + 'static) -> Widget {
        if self.thread.get() == Some(id) {
            return self.input_view(id, commit);
        }
        let menu = MenuController::new();
        let menu_to_open = menu.clone();
        let menu_to_close = menu.clone();
        let editor = self.clone();
        let original = title.to_owned();
        let action = incular::widgets::internal::ActionSurface::with_child(
            Text::new(title)
                .style(
                    typography::interface()
                        .font_size(14.0)
                        .font_weight(FontWeight::W500)
                        .color(INK),
                )
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        )
        .label(format!("Thread actions for {title}"))
        .size(Size::new(0.0, 28.0))
        .mouse_cursor(incular::widgets::MouseCursor::Click)
        .on_press(move || menu_to_open.open(None));
        let anchor = RawMenuAnchor::new(menu, move |_, info| {
            let editor = editor.clone();
            let original = original.clone();
            let close = menu_to_close.clone();
            let menu_search = menu_to_close.clone();
            let row = Row::new(vec![
                icon("pencil", 16.0, MUTED),
                Text::new("Rename thread")
                    .style(typography::interface().font_size(14.0).color(INK))
                    .into(),
            ])
            .spacing(8.0)
            .cross_axis_alignment(CrossAxisAlignment::Center);
            let button = Button::with_child(
                "Rename thread",
                Container::new()
                    .padding(EdgeInsets::symmetric(8.0, 0.0))
                    .child(row)
                    .into(),
            )
            .style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .background(WHITE)
                    .border_radius(6.0)
                    .alignment(Alignment::CENTER_LEFT)
                    .fixed_size(Size::new(160.0, 28.0))
                    .padding(EdgeInsets::ZERO),
            )
            .on_click(move || {
                close.close();
                editor.input.set_text(original.clone());
                editor.thread.set(Some(id));
            });
            let width = 168.0;
            let panel = Container::new()
                .width(width)
                .height(36.0)
                .padding(EdgeInsets::all(4.0))
                .decoration(
                    BoxDecoration::new()
                        .color(WHITE)
                        .border(Border::new(1.0, RULE))
                        .border_radius(BorderRadius::circular(8.0)),
                )
                .child(button);
            let x = info
                .anchor_rect
                .origin
                .x
                .clamp(0.0, (info.overlay_size.width - width).max(0.0));
            let y = info.anchor_rect.origin.y + info.anchor_rect.size.height + 4.0;
            Stack::new(vec![
                Positioned::new(
                    TapRegion::new(panel)
                        .group_id(info.tap_region_group_id.clone())
                        .on_tap_outside(move |_| menu_search.close()),
                )
                .left(x)
                .top(y),
            ])
            .into()
        })
        .child(action);
        Align::new(
            Alignment::CENTER_LEFT,
            Widget::from(anchor).with_key(format!("thread-title-{id}").as_str()),
        )
        .into()
    }

    fn input_view(&self, id: Uuid, commit: impl Fn(Uuid, &str) + 'static) -> Widget {
        let editor = self.clone();
        let guard = Rc::new(commit);
        let input = EditableText::new(self.input.clone())
            .size(Size::new(0.0, 26.0))
            .placeholder("Thread title")
            .content_padding(EdgeInsets::symmetric(4.0, 0.0))
            .style(
                typography::interface()
                    .font_size(14.0)
                    .font_weight(FontWeight::W500)
                    .color(INK),
            )
            .focused_border(Border::new(1.0, Color::rgba(66, 102, 206, 255)), 4.0)
            .on_editing_complete(|| {});
        let field = Widget::from(input).semantics(
            incular::widgets::internal::ExplicitSemantics::new(SemanticRole::TextField)
                .label("Thread title"),
        );
        KeyboardListener::new(field)
            .include_semantics(false)
            .on_key(move |event| {
                if !event.state.is_down()
                    || editor.thread.get() != Some(id)
                    || editor.input.preedit().is_some()
                    || editor.input.composing().is_some()
                {
                    return false;
                }
                match event.key {
                    KeyboardKey::Named(NamedKey::Escape) => editor.clear(),
                    KeyboardKey::Named(NamedKey::Enter) => {
                        editor.clear();
                        guard(id, &editor.input.text());
                    }
                    _ => return false,
                }
                true
            })
            .into()
    }
}
