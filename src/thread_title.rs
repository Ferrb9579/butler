//! Title action and inline editor for an already-saved local thread.
use crate::{
    empty_workspace::{INK, MUTED, RULE, WHITE},
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{gestures::GestureCallbacks, prelude::*, widgets::MenuController};
use std::{cell::Cell, rc::Rc};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Header,
    Sidebar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    id: Uuid,
    place: Place,
}

#[derive(Clone)]
pub struct Editor {
    target: Signal<Option<Target>>,
    focused: Rc<Cell<Option<Target>>>,
    input: TextEditingController,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            target: Signal::new(None),
            focused: Rc::new(Cell::new(None)),
            input: TextEditingController::new(),
        }
    }
}

impl Editor {
    pub fn clear(&self) {
        self.focused.set(None);
        self.target.set(None);
    }

    pub fn clear_sidebar(&self) {
        if self
            .target
            .get()
            .is_some_and(|target| target.place == Place::Sidebar)
        {
            self.clear();
        }
    }

    fn start(&self, id: Uuid, title: &str, place: Place) {
        self.focused.set(None);
        self.input.set_text(title);
        self.target.set(Some(Target { id, place }));
    }

    pub fn sidebar_editor(
        &self,
        id: Uuid,
        commit: impl Fn(Uuid, &str) + 'static,
    ) -> Option<Widget> {
        let target = Target {
            id,
            place: Place::Sidebar,
        };
        (self.target.get() == Some(target)).then(|| {
            Widget::from(
                Container::new()
                    .height(32.0)
                    .padding(EdgeInsets::symmetric(12.0, 0.0))
                    .alignment(Alignment::CENTER_LEFT)
                    .child(self.input_view(target, commit)),
            )
        })
    }

    pub fn sidebar_action(&self, id: Uuid, title: &str, row: impl Into<Widget>) -> Widget {
        let editor = self.clone();
        let title = title.to_owned();
        KeyboardListener::new(row)
            .include_semantics(false)
            .on_key(move |event| {
                if event.state.is_down()
                    && !event.repeat
                    && event.key == KeyboardKey::Named(NamedKey::F2)
                {
                    editor.start(id, &title, Place::Sidebar);
                    true
                } else {
                    false
                }
            })
            .into()
    }

    pub fn view(&self, id: Uuid, title: &str, commit: impl Fn(Uuid, &str) + 'static) -> Widget {
        let target = Target {
            id,
            place: Place::Header,
        };
        if self.target.get() == Some(target) {
            return self.input_view(target, commit);
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
                editor.start(id, &original, Place::Header);
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

    fn input_view(&self, target: Target, commit: impl Fn(Uuid, &str) + 'static) -> Widget {
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
        let blur_editor = self.clone();
        let blur_commit = guard.clone();
        let field = GestureDetector::new(field).callbacks(GestureCallbacks {
            on_focus_within: Some(Rc::new(move |focused| {
                if blur_editor.target.get() != Some(target) {
                    return;
                }
                if focused {
                    if blur_editor.focused.replace(Some(target)) != Some(target) {
                        // Select the title on actual focus, not by faking autofocus.
                        blur_editor.input.select_all();
                    }
                } else if blur_editor.focused.replace(None) == Some(target)
                    && target.place == Place::Sidebar
                    && blur_editor.input.preedit().is_none()
                    && blur_editor.input.composing().is_none()
                {
                    let title = blur_editor.input.text();
                    blur_editor.clear();
                    blur_commit(target.id, &title);
                }
            })),
            ..GestureCallbacks::default()
        });
        KeyboardListener::new(field)
            .include_semantics(false)
            .on_key(move |event| {
                if !event.state.is_down()
                    || editor.target.get() != Some(target)
                    || editor.input.preedit().is_some()
                    || editor.input.composing().is_some()
                {
                    return false;
                }
                match event.key {
                    KeyboardKey::Named(NamedKey::Escape) => editor.clear(),
                    KeyboardKey::Named(NamedKey::Enter) => {
                        editor.clear();
                        guard(target.id, &editor.input.text());
                    }
                    _ => return false,
                }
                true
            })
            .into()
    }
}
