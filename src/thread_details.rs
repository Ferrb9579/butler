//! Read-only desktop inline workspace row from T3 Code v0.1.11.
use crate::{
    empty_workspace::MUTED,
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{
    prelude::*,
    widgets::{BackdropFilter, RawTooltip, RawTooltipTriggerMode, internal::ExplicitSemantics},
};
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

const WIDTH: f32 = 280.0;
const EDGE: f32 = 12.0;
const CLEARANCE: f32 = 32.0;
const MINIMUM_CHAT: f32 = 640.0;
const MINIMUM_HEIGHT: f32 = 160.0;
const FIND_INSET: f32 = 48.0;
const GLASS: Color = Color::rgba(255, 255, 255, 213);
const BORDER: Color = Color::rgba(24, 24, 27, 26);
pub const TOGGLE: &str = "Toggle thread details panel";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub right: f32,
    pub top: f32,
    pub lane_right: f32,
}

pub fn inline_layout(width: f32, height: f32, padding: f32, find_open: bool) -> Option<Layout> {
    let top = EDGE + if find_open { FIND_INSET } else { 0.0 };
    let lane_right = width - WIDTH - EDGE - CLEARANCE;
    (lane_right - padding >= MINIMUM_CHAT && height - top - EDGE >= MINIMUM_HEIGHT).then_some(
        Layout {
            width: WIDTH,
            right: EDGE,
            top,
            lane_right,
        },
    )
}

#[derive(Clone)]
pub struct Visibility {
    closed: Signal<BTreeSet<Uuid>>,
    details_focus: FocusNode,
    content_focus: FocusNode,
}

impl Default for Visibility {
    fn default() -> Self {
        let content_focus = FocusNode::new();
        content_focus.set_skip_traversal(true);
        Self {
            closed: Signal::new(BTreeSet::new()),
            details_focus: FocusNode::new(),
            content_focus,
        }
    }
}

impl Visibility {
    pub fn is_open(&self, thread: Uuid) -> bool {
        !self.closed.get().contains(&thread)
    }

    fn toggle(&self, thread: Uuid) {
        let mut closed = self.closed.get();
        if closed.remove(&thread) {
            self.details_focus.request_focus();
        } else {
            closed.insert(thread);
            if self.details_focus.has_focus() {
                self.content_focus.request_focus();
            }
        }
        self.closed.set(closed);
    }

    pub fn content(&self, child: impl Into<Widget>) -> Widget {
        Focus::new(child).node(self.content_focus.clone()).into()
    }
}

#[derive(Clone)]
pub struct LocalDetails {
    pub thread: Uuid,
    pub machine: String,
    pub path: PathBuf,
    pub visibility: Visibility,
    pub canvas_height: f32,
    pub inline_open: bool,
}

impl LocalDetails {
    pub fn toggle(&self) {
        self.visibility.toggle(self.thread);
    }

    pub fn toggle_button(&self) -> Widget {
        let open = self.inline_open;
        let next = self.clone();
        let control = Button::with_child(
            TOGGLE,
            icon("square-menu", 16.0, Color::rgba(24, 24, 27, 204)),
        )
        .style(
            ButtonStyle::new()
                .variant(ButtonVariant::Ghost)
                .fixed_size(Size::new(28.0, 28.0))
                .padding(EdgeInsets::ZERO)
                .background(if open {
                    Color::rgba(244, 244, 245, 255)
                } else {
                    Color::TRANSPARENT
                }),
        )
        .on_click(move || next.toggle());
        RawTooltip::new(
            "Toggle thread details",
            Widget::from(control).semantics(
                ExplicitSemantics::new(SemanticRole::Button)
                    .label(TOGGLE)
                    .state(SemanticState {
                        toggled: Some(open),
                        ..SemanticState::default()
                    }),
            ),
        )
        .trigger_mode(RawTooltipTriggerMode::Manual)
        .enable_tap_to_dismiss(false)
        .into()
    }

    pub fn card(&self) -> Widget {
        let path = self.path.to_string_lossy().into_owned();
        let row_text = Widget::from(
            Text::new(&self.machine)
                .style(
                    typography::interface()
                        .font_size(13.0)
                        .font_weight(FontWeight::W500)
                        .line_height_absolute(18.0)
                        .color(Color::rgba(24, 24, 27, 204)),
                )
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        );
        let row = Container::new()
            .height(32.0)
            .padding(EdgeInsets::symmetric(10.0, 0.0))
            .alignment(Alignment::CENTER_LEFT)
            .child(
                Row::new([
                    icon("monitor", 16.0, MUTED),
                    Expanded::new(row_text).into(),
                    icon("folder", 16.0, MUTED),
                ])
                .spacing(10.0)
                .cross_axis_alignment(CrossAxisAlignment::Center),
            );
        let row = Widget::from(RawTooltip::new(&path, row)).semantics(
            ExplicitSemantics::new(SemanticRole::Text)
                .label(format!("Run context: {}, Local workspace", self.machine))
                .state(SemanticState {
                    tooltip: Some(path),
                    ..SemanticState::default()
                }),
        );
        let body = Container::new()
            .width(WIDTH)
            .padding(EdgeInsets::from_ltrb(8.0, 8.0, 8.0, 10.0))
            .decoration(
                BoxDecoration::new()
                    .color(GLASS)
                    .border(Border::new(1.0, BORDER))
                    .border_radius(BorderRadius::circular(16.0)),
            )
            .child(row);
        Focus::new(
            Widget::from(ClipRRect::new(16.0, BackdropFilter::blur(12.0, body))).semantics(
                ExplicitSemantics::new(SemanticRole::GenericContainer).label("Thread details"),
            ),
        )
        .node(self.visibility.details_focus.clone())
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_layout_reserves_the_lane_and_moves_below_find() {
        let closed_find = inline_layout(1024.0, 760.0, 48.0, false).unwrap();
        assert_eq!(
            (closed_find.width, closed_find.right, closed_find.top),
            (280.0, 12.0, 12.0)
        );
        assert_eq!(closed_find.lane_right - 48.0, 652.0);
        let open_find = inline_layout(1024.0, 760.0, 48.0, true).unwrap();
        assert_eq!(open_find.top - closed_find.top, 48.0);
        assert!(inline_layout(1012.0, 184.0, 48.0, false).is_some());
        assert!(inline_layout(1011.0, 184.0, 48.0, false).is_none());
        assert!(inline_layout(1012.0, 183.0, 48.0, false).is_none());
        assert!(inline_layout(1012.0, 231.0, 48.0, true).is_none());
    }

    #[test]
    fn visibility_is_independent_per_thread() {
        let visibility = Visibility::default();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert!(visibility.is_open(first));
        visibility.toggle(first);
        assert!(!visibility.is_open(first));
        assert!(visibility.is_open(second));
        visibility.toggle(first);
        assert!(visibility.is_open(first));
    }
}
