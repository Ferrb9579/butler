//! A sidebar message hit seeds an exact, route-scoped transcript occurrence.

use crate::message_search::{MessageHit, MessageSource};
use butler::threads::Conversation;
use incular::{
    prelude::*,
    widgets::{EnsureVisible, GlobalKey, Scrollable},
};
use std::{cell::Cell, ops::Range};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Message {
    pub exchange: usize,
    pub source: MessageSource,
}

impl Message {
    fn order(self) -> (usize, bool) {
        (self.exchange, self.source == MessageSource::Assistant)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub message: Message,
    pub range: Range<usize>,
}

#[derive(Clone, PartialEq)]
pub struct Jump {
    pub thread: Uuid,
    pub query: String,
    matches: std::rc::Rc<Vec<Occurrence>>,
    active: Option<usize>,
    anchor: Option<Message>,
    pub key: GlobalKey,
}

impl Jump {
    pub fn from_result(conversation: &Conversation, hit: MessageHit, query: &str) -> Option<Self> {
        if conversation.id != hit.thread_id || hit.exchange_index > conversation.follow_ups.len() {
            return None;
        }
        Some(Self::new(
            conversation,
            crate::thread_find::seed_query(query, &hit.snippet),
            Some(Message {
                exchange: hit.exchange_index,
                source: hit.source,
            }),
        ))
    }

    pub fn new(conversation: &Conversation, query: String, anchor: Option<Message>) -> Self {
        let needle = search_text(&query);
        let mut matches = Vec::new();
        if !needle.is_empty() {
            let pairs = std::iter::once((&conversation.prompt, &conversation.reply)).chain(
                conversation
                    .follow_ups
                    .iter()
                    .map(|exchange| (&exchange.prompt, &exchange.reply)),
            );
            for (exchange, (prompt, reply)) in pairs.enumerate() {
                for (source, body) in [
                    (MessageSource::User, prompt),
                    (MessageSource::Assistant, reply),
                ] {
                    let message = Message { exchange, source };
                    matches.extend(
                        crate::thread_find::find_ranges(body, needle)
                            .into_iter()
                            .map(|range| Occurrence { message, range }),
                    );
                }
            }
        }
        let active = (!matches.is_empty()).then(|| {
            anchor.map_or(0, |from| {
                matches.partition_point(|m| m.message.order() < from.order())
            }) % matches.len()
        });
        Self {
            thread: conversation.id,
            query,
            matches: matches.into(),
            active,
            // Sidebar entry is still a useful explicit destination if its seeded
            // spelling no longer occurs. Clearing a typed query does not scroll.
            anchor,
            key: GlobalKey::new(),
        }
    }

    pub fn edited(&self, conversation: &Conversation, text: &str) -> Self {
        let mut next = Self::new(conversation, text.into(), self.target());
        next.anchor = None;
        next
    }

    pub fn stepped(&self, forward: bool) -> Option<Self> {
        let active = self.active?;
        let total = self.matches.len();
        let index = if forward {
            (active + 1) % total
        } else if active == 0 {
            total - 1
        } else {
            active - 1
        };
        let mut next = self.clone();
        next.active = Some(index);
        next.key = GlobalKey::new();
        Some(next)
    }

    pub fn count(&self) -> String {
        if search_text(&self.query).is_empty() {
            return String::new();
        }
        let Some(active) = self.active else {
            return "0/0".into();
        };
        format!("{}/{}", active + 1, self.matches.len())
    }

    pub fn occurrence(&self) -> Option<&Occurrence> {
        self.active.and_then(|i| self.matches.get(i))
    }

    pub fn target(&self) -> Option<Message> {
        self.occurrence().map(|m| m.message).or(self.anchor)
    }
    /// One measured reveal after layout, never a guessed text or pixel offset.
    /// A superseded key is absent from the new tree and cannot scroll that view.
    pub fn reveal(&self, cx: &incular::runtime::BuildContext, posted: &Cell<Option<GlobalKey>>) {
        if self.target().is_none() || posted.replace(Some(self.key)) == Some(self.key) {
            return;
        }
        let key = self.key.into();

        cx.dispatch(move |runtime| {
            let owner = runtime.dispatcher();

            add_post_frame_callback(move |_| {
                owner.dispatch(move |runtime| {
                    if let Some(element) = runtime.tree().element_with_key(&key)
                        && Scrollable::ensure_visible(
                            runtime.tree(),
                            element,
                            EnsureVisible::new().alignment(0.5),
                        ) > 0
                    {
                        runtime.request_frame();
                    }
                });
            });

            runtime.request_frame();
        });
    }
}

// The reference passes String.prototype.trim() to Find. Unlike Rust's
// White_Space set this includes BOM (U+FEFF) and excludes NEL (U+0085).
fn search_text(raw: &str) -> &str {
    raw.trim_matches(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use butler::threads::{Exchange, Workspace};

    fn saved() -> Conversation {
        Conversation {
            id: Uuid::new_v4(),
            workspace: Workspace::Scratch,
            prompt: "Café first. cafe only.".into(),
            reply: "Café response; Café end.".into(),
            follow_ups: vec![Exchange {
                prompt: "Café follow-up.".into(),
                reply: "Plain prose.".into(),
            }],
        }
    }

    #[test]
    fn anchored_occurrence_uses_message_order_and_wraps() {
        let turn = saved();
        let reply = Message {
            exchange: 0,
            source: MessageSource::Assistant,
        };
        let find = Jump::new(&turn, "  café  ".into(), Some(reply));
        assert_eq!(find.query, "  café  ");
        assert_eq!(find.count(), "2/4");
        assert_eq!(find.occurrence().unwrap().range, 0..5);
        let next = find.stepped(true).unwrap();
        assert_eq!(next.count(), "3/4");
        assert!(next.target() == Some(reply));
        assert_eq!(
            &turn.reply[next.occurrence().unwrap().range.clone()],
            "Café"
        );
        assert_eq!(
            find.stepped(false).unwrap().stepped(false).unwrap().count(),
            "4/4"
        );
        let changed = next.edited(&turn, "cafe");
        assert_eq!(changed.count(), "1/1");
        assert_eq!(
            &turn.prompt[changed.occurrence().unwrap().range.clone()],
            "cafe"
        );
    }

    #[test]
    fn blank_and_missing_queries_do_not_scroll_after_edit() {
        let turn = saved();
        let opened = Jump::new(&turn, "Café".into(), None);
        assert_eq!(opened.count(), "1/4");
        let missing = opened.edited(&turn, "missing");
        assert_eq!(missing.count(), "0/0");
        assert!(missing.stepped(true).is_none());
        assert!(missing.target().is_none());
        let blank = missing.edited(&turn, " \t ");
        assert_eq!(blank.count(), "");
        assert_eq!(missing.edited(&turn, "\u{feff}Café\u{a0}").count(), "1/4");
        assert_eq!(missing.edited(&turn, "\u{85}").count(), "0/0");
        assert!(blank.stepped(false).is_none());
        assert!(blank.target().is_none());
    }
}
