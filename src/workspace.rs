//! Draft immediately after "Start without a project" on a profile with no provider.
//! Reference: T3 Code v0.1.11 desktop, native fixture 45329928604.

use crate::sidebar_search::{self, Search};
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use butler::threads::{Conversation, ThreadSummary};
use incular::prelude::*;

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, WHITE, label, sidebar, space};

pub fn draft(
    project: Option<String>,
    history: Vec<ThreadSummary>,
    selected: Option<Conversation>,
    history_error: Option<String>,
    search: Search,
    add_project: Option<impl Fn() + Clone + 'static>,
    open_thread: impl Fn(uuid::Uuid) + Clone + 'static,
    new_thread: impl Fn() + Clone + 'static,
    open_settings: impl Fn() + Clone + 'static,
    #[cfg(test)] offline: Option<OfflineDraft>,
) -> Widget {
    let title = project.clone().unwrap_or_else(|| "No project".into());
    let sidebar = Widget::stateful_builder(search.revision(), move |_| {
        let active = search.active();
        let matching = if active {
            sidebar_search::matching_threads(&history, &search.query())
        } else {
            history.iter().collect::<Vec<_>>()
        };
        let ids = matching.iter().map(|thread| thread.id).collect::<Vec<_>>();
        let highlighted = if active {
            search.highlighted(&ids)
        } else {
            None
        };
        let mut threads = matching
            .into_iter()
            .map(|thread| {
                let id = thread.id;
                let open_thread = open_thread.clone();
                let clear = search.clone();
                let content = if active {
                    Widget::from(
                        Text::new(&thread.title)
                            .style(crate::typography::interface().font_size(14.0).color(INK))
                            .max_lines(Some(1))
                            .overflow(TextOverflow::Ellipsis),
                    )
                } else {
                    label(&thread.title, 13.0, FontWeight::W400, INK)
                };
                let button = Button::with_child(format!("Open thread: {}", thread.title), content)
                    .style(
                        ButtonStyle::new()
                            .variant(ButtonVariant::Ghost)
                            .height(if active { 36.0 } else { 32.0 })
                            .border_radius(6.0)
                            .background(if highlighted == Some(id) {
                                Color::rgba(236, 236, 239, 255)
                            } else {
                                PAGE
                            })
                            .padding(EdgeInsets::symmetric(if active { 10.0 } else { 16.0 }, 0.0))
                            .alignment(Alignment::CENTER_LEFT),
                    )
                    .on_click(move || {
                        clear.reset();
                        open_thread(id);
                    });
                if active {
                    let hover = search.clone();
                    incular::widgets::MouseRegion::new(button)
                        .opaque(false)
                        .on_enter(move |_| hover.highlight(id))
                        .into()
                } else {
                    button.into()
                }
            })
            .collect::<Vec<Widget>>();
        if active && threads.is_empty() {
            threads.push(
                Container::new()
                    .padding(EdgeInsets::symmetric(8.0, 24.0))
                    .alignment(Alignment::CENTER)
                    .child(label("No threads found", 12.0, FontWeight::W400, MUTED))
                    .into(),
            );
        }
        if let Some(error) = &history_error {
            threads.push(label(error, 12.0, FontWeight::W400, MUTED));
        }
        let choose_thread = open_thread.clone();
        sidebar(
            project.as_deref(),
            add_project.clone(),
            Some(new_thread.clone()),
            Some(sidebar_search::field(search.clone(), ids, choose_thread)),
            threads,
        )
    });
    let route_title = selected
        .as_ref()
        .and_then(|turn| turn.prompt.trim().lines().next())
        .unwrap_or("New thread")
        .chars()
        .take(72)
        .collect::<String>();
    Row::new(vec![
        sidebar,
        Container::new().width(1.0).color(RULE).into(),
        Expanded::new(
            Container::new().color(PAGE).child(
                Column::new(vec![
                    Widget::from(
                        Container::new()
                            .height(40.0)
                            .padding(EdgeInsets::symmetric(20.0, 0.0))
                            .alignment(Alignment::CENTER_LEFT)
                            .child(label(
                                &format!("⬚ {title}   /   {route_title}"),
                                14.0,
                                FontWeight::W500,
                                INK,
                            )),
                    ),
                    Expanded::new(LayoutBuilder::new(move |_, constraints| {
                        let width = constraints.max_width();
                        let height = constraints.max_height();
                        let margin = if width >= 720.0 { 48.0 } else { 16.0 };
                        let composer_width = (width - 2.0 * margin).clamp(1.0, 652.0);
                        if let Some(turn) = selected.as_ref() {
                            return Container::new()
                                .padding(EdgeInsets::symmetric(margin, 0.0))
                                .child(Container::new().width(composer_width).child(transcript(
                                    turn,
                                    #[cfg(test)]
                                    offline.clone(),
                                )))
                                .into();
                        }
                        let top = (height * 0.28).max(28.0);
                        Container::new()
                            .padding(EdgeInsets::symmetric(margin, 0.0))
                            .child(
                                Column::new(vec![
                                    space(top),
                                    Container::new()
                                        .width(composer_width)
                                        .child(composer(
                                            &title,
                                            open_settings.clone(),
                                            #[cfg(test)]
                                            offline.clone(),
                                        ))
                                        .into(),
                                ])
                                .cross_axis_alignment(CrossAxisAlignment::Start),
                            )
                            .into()
                    }))
                    .into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ),
        )
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

fn composer(
    title: &str,
    open_settings: impl Fn() + 'static,
    #[cfg(test)] offline: Option<OfflineDraft>,
) -> Widget {
    #[cfg(test)]
    if let Some(offline) = offline {
        return offline_composer(title, offline);
    }
    Column::new(vec![
        centered("What should we work on?", 32.0, FontWeight::W400),
        space(10.0),
        centered(title, 14.0, FontWeight::W500),
        space(20.0),
        Container::new()
            .height(112.0)
            .padding(EdgeInsets::all(16.0))
            .decoration(
                BoxDecoration::new()
                    .color(WHITE)
                    .border_radius(BorderRadius::all(Radius::circular(18.0))),
            )
            .child(
                Column::new(vec![
                    label(
                        "Enable a provider in Settings to send a message",
                        14.0,
                        FontWeight::W400,
                        MUTED,
                    ),
                    Spacer::new().into(),
                    Row::new(vec![
                        label("♧", 15.0, FontWeight::W400, MUTED),
                        Spacer::new().into(),
                        crate::provider_settings::action("Open provider settings", open_settings),
                        label("♩     ↑", 14.0, FontWeight::W400, MUTED),
                    ])
                    .into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Start),
            )
            .into(),
        space(10.0),
        Container::new()
            .alignment(Alignment::CENTER_RIGHT)
            .child(label("⊘ Full access", 12.0, FontWeight::W400, MUTED))
            .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

fn centered(text: &str, size: f32, weight: FontWeight) -> Widget {
    Container::new()
        .alignment(Alignment::CENTER)
        .child(label(text, size, weight, INK))
        .into()
}

#[cfg(test)]
#[derive(Clone)]
pub struct OfflineDraft {
    pub input: TextEditingController,
    pub error: Option<String>,
    pub send: std::rc::Rc<dyn Fn(String)>,
}

#[cfg(test)]
fn offline_composer(title: &str, draft: OfflineDraft) -> Widget {
    let mut rows = vec![
        centered("What should we work on?", 32.0, FontWeight::W400),
        centered(title, 14.0, FontWeight::W500),
        space(20.0),
    ];
    let control = draft.input.clone();
    rows.extend([
        EditableText::new(draft.input)
            .placeholder("Ask anything")
            .style(TextStyle::new().font_size(14.0).color(INK))
            .into(),
        crate::provider_settings::action("Send", move || (draft.send)(control.text())),
    ]);
    if let Some(error) = draft.error {
        rows.push(label(&error, 14.0, FontWeight::W400, MUTED));
    }
    Column::new(rows)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
}

fn transcript(turn: &Conversation, #[cfg(test)] offline: Option<OfflineDraft>) -> Widget {
    let mut messages = Vec::new();
    let pairs = std::iter::once((&turn.prompt, &turn.reply)).chain(
        turn.follow_ups
            .iter()
            .map(|exchange| (&exchange.prompt, &exchange.reply)),
    );
    for (prompt, reply) in pairs {
        messages.extend([
            space(16.0),
            label(prompt, 14.0, FontWeight::W500, INK),
            space(12.0),
            label(reply, 14.0, FontWeight::W400, INK),
        ]);
    }
    let scroll = Expanded::new(SingleChildScrollView::new(
        Column::new(messages)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Start),
    ));
    let rows = vec![Widget::from(scroll)];
    #[cfg(test)]
    let rows = if let Some(draft) = offline {
        let mut rows = rows;
        let control = draft.input.clone();
        if let Some(error) = draft.error {
            rows.push(label(&error, 14.0, FontWeight::W400, MUTED));
        }
        rows.extend([
            EditableText::new(draft.input)
                .placeholder("Follow up")
                .style(TextStyle::new().font_size(14.0).color(INK))
                .into(),
            crate::provider_settings::action("Send follow-up", move || {
                (draft.send)(control.text())
            }),
            space(12.0),
        ]);
        rows
    } else {
        rows
    };
    Column::new(rows)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
}
