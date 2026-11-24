//! A sidebar message hit seeds an exact, route-scoped transcript occurrence.

use crate::message_search::{MessageHit, MessageSource};
use butler::threads::Conversation;
use incular::{
    prelude::*,
    widgets::{EnsureVisible, GlobalKey, Scrollable},
};
use std::{cell::Cell, ops::Range};
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub struct Jump {
    pub thread: Uuid,
    pub exchange: usize,
    pub source: MessageSource,
    pub query: String,
    pub range: Option<Range<usize>>,
    pub key: GlobalKey,
}

impl Jump {
    pub fn from_result(conversation: &Conversation, hit: MessageHit, query: &str) -> Option<Self> {
        if conversation.id != hit.thread_id {
            return None;
        }
        let text = match (hit.exchange_index, hit.source) {
            (0, MessageSource::User) => &conversation.prompt,
            (0, MessageSource::Assistant) => &conversation.reply,
            (index, source) => {
                let exchange = conversation.follow_ups.get(index - 1)?;
                match source {
                    MessageSource::User => &exchange.prompt,
                    MessageSource::Assistant => &exchange.reply,
                }
            }
        };
        let query = crate::thread_find::seed_query(query, &hit.snippet);
        let range = crate::thread_find::find_ranges(text, &query)
            .into_iter()
            .next();
        Some(Self {
            thread: hit.thread_id,
            exchange: hit.exchange_index,
            source: hit.source,
            query,
            range,
            key: GlobalKey::new(),
        })
    }

    /// One measured reveal after layout, never a guessed text or pixel offset.
    /// A superseded key is absent from the new tree and cannot scroll that view.
    pub fn reveal(&self, cx: &incular::runtime::BuildContext, posted: &Cell<Option<GlobalKey>>) {
        if posted.replace(Some(self.key)) == Some(self.key) {
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
