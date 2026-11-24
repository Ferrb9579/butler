//! Draft immediately after "Start without a project" on a profile with no provider.
//! Reference: T3 Code v0.1.11 desktop, native fixture 45329928604.

use crate::message_jump::{Jump, Message};
use crate::message_search::{MessageHit, MessageSource, Results};
use crate::settings_icons::icon;
use crate::sidebar_search::{self, Search};
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use butler::threads::{Conversation, ThreadSummary};
use incular::prelude::*;
use std::{cell::RefCell, rc::Rc};

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, WHITE, icon_label, label, sidebar, space};

pub fn draft(
    project: Option<String>,
    project_icon: Option<incular::image::ImageHandle>,
    history: Vec<ThreadSummary>,
    messages: Results,
    selected: Option<Conversation>,
    jump: Option<Jump>,
    jump_state: Signal<Option<Jump>>,
    find_input: TextEditingController,
    title_editor: crate::thread_title::Editor,
    rename: impl Fn(uuid::Uuid, &str) + 'static,
    open_find: impl Fn() + 'static,
    history_error: Option<String>,
    search: Search,
    add_project: Option<impl Fn() + Clone + 'static>,
    open_thread: impl Fn(uuid::Uuid, Option<(MessageHit, String)>) + Clone + 'static,
    new_thread: impl Fn() + Clone + 'static,
    start_here: impl Fn() + 'static,
    open_settings: impl Fn() + Clone + 'static,
    #[cfg(test)] offline: Option<OfflineDraft>,
) -> Widget {
    let title = project.clone().unwrap_or_else(|| "No project".into());
    let project_title = title.clone();
    let has_project = project.is_some();
    let header_icon = project_icon.clone();
    let start_here = Rc::new(start_here);
    let selected = selected.map(Rc::new);
    let rename = Rc::new(rename);
    let header_rename = rename.clone();
    let active_header = if let Some(turn) = &selected {
        let saved_title = history
            .iter()
            .find(|row| row.id == turn.id)
            .map(|row| row.title.clone())
            .unwrap_or_else(|| {
                turn.prompt
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or("New thread")
                    .chars()
                    .take(72)
                    .collect()
            });
        title_editor.view(
            turn.id,
            &saved_title,
            move |id, title| header_rename(id, title),
            open_find,
        )
    } else {
        breadcrumb_text("New thread", INK)
    };
    let rendered_query = search.query();
    let sidebar_settings = open_settings.clone();
    let sidebar = Widget::stateful_builder(search.revision(), move |_| {
        let active = search.active();
        let matching = if active {
            sidebar_search::matching_threads(&history, &search.query())
        } else {
            history.iter().collect::<Vec<_>>()
        };
        let lookup = if rendered_query == search.query() {
            &messages
        } else {
            &Results::Idle
        };
        let mut rows = matching
            .into_iter()
            .map(|row| (row, None))
            .collect::<Vec<_>>();
        let mut ids = rows.iter().map(|(thread, _)| thread.id).collect::<Vec<_>>();
        if active && let Results::Ready(hits) = lookup {
            for hit in hits {
                if !ids.contains(&hit.thread_id)
                    && let Some(row) = history.iter().find(|row| row.id == hit.thread_id)
                {
                    ids.push(row.id);
                    rows.push((row, Some(hit)));
                }
            }
        }
        let highlighted = if active {
            search.highlighted(&ids)
        } else {
            None
        };
        let message_rows = rows
            .iter()
            .filter_map(|(row, hit)| hit.map(|hit| (row.id, hit.clone())))
            .collect::<std::collections::HashMap<_, _>>();
        let mut threads = rows
            .into_iter()
            .map(|(thread, hit)| {
                let id = thread.id;
                if !active {
                    let commit = rename.clone();
                    if let Some(input) =
                        title_editor.sidebar_editor(id, move |id, title| commit(id, title))
                    {
                        return input;
                    }
                }
                let open_thread = open_thread.clone();
                let clear = search.clone();
                let seed = hit.cloned().map(|hit| (hit, search.query()));
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
                let content = if let Some(hit) = hit {
                    let speaker = match hit.source {
                        MessageSource::User => "You",
                        MessageSource::Assistant => "Agent",
                    };
                    Column::new(vec![
                        content,
                        Text::new(format!("{speaker}: {}", hit.snippet))
                            .style(crate::typography::interface().font_size(12.0).color(MUTED))
                            .max_lines(Some(1))
                            .overflow(TextOverflow::Ellipsis)
                            .into(),
                    ])
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .into()
                } else {
                    content
                };
                let button = Button::with_child(format!("Open thread: {}", thread.title), content)
                    .style(
                        ButtonStyle::new()
                            .variant(ButtonVariant::Ghost)
                            .height(if hit.is_some() {
                                48.0
                            } else if active {
                                36.0
                            } else {
                                32.0
                            })
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
                        open_thread(id, seed.clone());
                    });
                if active {
                    let hover = search.clone();
                    incular::widgets::MouseRegion::new(button)
                        .opaque(false)
                        .on_enter(move |_| hover.highlight(id))
                        .into()
                } else {
                    title_editor.sidebar_action(id, &thread.title, button)
                }
            })
            .collect::<Vec<Widget>>();
        if active && threads.is_empty() {
            threads.push(
                Container::new()
                    .padding(EdgeInsets::symmetric(8.0, 24.0))
                    .alignment(Alignment::CENTER)
                    .child(label(
                        match lookup {
                            Results::Pending => "Searching thread messages…",
                            Results::Failed => "Couldn't search thread messages",
                            _ => "No threads found",
                        },
                        12.0,
                        FontWeight::W400,
                        MUTED,
                    ))
                    .into(),
            );
        }
        if let Some(error) = &history_error {
            threads.push(label(error, 12.0, FontWeight::W400, MUTED));
        }
        let choose_thread = open_thread.clone();
        let search_title_editor = title_editor.clone();
        let query = search.query();
        sidebar(
            project.as_deref(),
            project_icon.clone(),
            add_project.clone(),
            Some(new_thread.clone()),
            sidebar_settings.clone(),
            Some(sidebar_search::field(
                search.clone(),
                ids,
                move || search_title_editor.clear_sidebar(),
                move |id| {
                    choose_thread(
                        id,
                        message_rows
                            .get(&id)
                            .cloned()
                            .map(|hit| (hit, query.clone())),
                    );
                },
            )),
            threads,
        )
    });
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
                            .child(LayoutBuilder::new(move |_, constraints| {
                                // Leave room for the divider, both gaps, and the
                                // 40px minimum of the current-thread breadcrumb.
                                let maximum = (constraints.max_width() - 72.0).max(0.0);
                                let width = (20.0 + word_advance(&project_title)).min(maximum);
                                let action = start_here.clone();
                                Row::new(vec![
                                    workspace_project(
                                        &project_title,
                                        header_icon.as_ref(),
                                        has_project,
                                        width,
                                        move || action(),
                                    ),
                                    breadcrumb_text("/", MUTED),
                                    Expanded::new(active_header.clone()).into(),
                                ])
                                .spacing(12.0)
                                .cross_axis_alignment(CrossAxisAlignment::Center)
                                .into()
                            })),
                    ),
                    Expanded::new(LayoutBuilder::new(move |_, constraints| {
                        let width = constraints.max_width();
                        let height = constraints.max_height();
                        let margin = if width >= 720.0 { 48.0 } else { 16.0 };
                        let composer_width = (width - 2.0 * margin).clamp(1.0, 652.0);
                        if let Some(turn) = selected.as_ref() {
                            let find_width = (width - 24.0).clamp(1.0, 280.0);
                            let top = if jump.is_some()
                                && margin + composer_width > width - 12.0 - find_width
                            {
                                52.0_f32.min(height)
                            } else {
                                0.0
                            };
                            // Fix the lane's actual width rather than nesting a
                            // width request inside the tight full-pane Stack child.
                            let mut layers = vec![
                                Positioned::new(transcript(
                                    turn,
                                    jump.as_ref(),
                                    composer_width,
                                    #[cfg(test)]
                                    offline.clone(),
                                ))
                                .left(margin)
                                .top(top)
                                .bottom(0.0)
                                .width(composer_width),
                            ];
                            if let Some(jump) = &jump {
                                layers.push(
                                    Positioned::new(find_bar(
                                        turn,
                                        jump,
                                        jump_state.clone(),
                                        find_input.clone(),
                                        find_width,
                                    ))
                                    .top(12.0)
                                    .right(12.0),
                                );
                            }
                            return Stack::new(layers).into();
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

fn breadcrumb_style(color: Color) -> TextStyle {
    crate::typography::interface()
        .font_size(14.0)
        .font_weight(FontWeight::W500)
        .color(color)
}

fn breadcrumb_text(text: &str, color: Color) -> Widget {
    Text::new(text)
        .style(breadcrumb_style(color))
        .max_lines(Some(1))
        .overflow(TextOverflow::Ellipsis)
        .into()
}

thread_local! {
    static CRUMB_TEXT: RefCell<incular::text::TextEngine> = RefCell::new(incular::text::TextEngine::new());
}

fn word_advance(text: &str) -> f32 {
    CRUMB_TEXT.with_borrow_mut(|engine| {
        engine
            .layout(text, &breadcrumb_style(MUTED), None, TextAlign::Start)
            .lines
            .first()
            .map_or(0.0, |line| line.width.ceil())
    })
}

fn workspace_project(
    title: &str,
    project_icon: Option<&incular::image::ImageHandle>,
    has_project: bool,
    width: f32,
    start_here: impl Fn() + 'static,
) -> Widget {
    let mark = if has_project {
        crate::project_icon_view::project_mark(title, project_icon, 14.0)
    } else {
        icon("message-square-dashed", 14.0, MUTED)
    };
    let contents = Row::new(vec![
        mark,
        Expanded::new(breadcrumb_text(title, MUTED)).into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .spacing(6.0);
    let contents = SizedBox::new().width(width).child(contents);
    incular::widgets::internal::ActionSurface::with_child(contents)
        .label(format!("New thread in {title}"))
        .size(Size::new(width, 28.0))
        .mouse_cursor(incular::widgets::MouseCursor::Click)
        .on_press(start_here)
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
                        icon("paperclip", 16.0, MUTED),
                        Spacer::new().into(),
                        crate::provider_settings::action("Open provider settings", open_settings),
                        icon("mic", 16.0, MUTED),
                        SizedBox::new().width(10.0).into(),
                        crate::settings_icons::compose_send(MUTED),
                    ])
                    .into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Start),
            )
            .into(),
        space(10.0),
        Container::new()
            .alignment(Alignment::CENTER_RIGHT)
            .child(icon_label("shield-off", 14.0, "Full access", 12.0, MUTED))
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

fn transcript(
    turn: &Conversation,
    jump: Option<&Jump>,
    lane_width: f32,
    #[cfg(test)] offline: Option<OfflineDraft>,
) -> Widget {
    let mut messages = Vec::new();
    let pairs = std::iter::once((&turn.prompt, &turn.reply)).chain(
        turn.follow_ups
            .iter()
            .map(|exchange| (&exchange.prompt, &exchange.reply)),
    );
    for (index, (prompt, reply)) in pairs.enumerate() {
        let row = |body: &str, role| {
            let active = jump.filter(|jump| {
                jump.target()
                    == Some(Message {
                        exchange: index,
                        source: role,
                    })
            });
            if role == MessageSource::User {
                user_message(body, lane_width, active)
            } else {
                assistant_message(body, active)
            }
        };
        messages.extend([
            space(16.0),
            row(prompt.as_str(), MessageSource::User),
            space(12.0),
            row(reply.as_str(), MessageSource::Assistant),
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

const USER_MESSAGE_SURFACE: Color = Color::rgba(235, 235, 237, 255);
const ASSISTANT_MESSAGE_INK: Color = Color::rgba(24, 24, 27, 230);

fn message_style(color: Color) -> TextStyle {
    crate::typography::interface()
        .font_size(14.0)
        .line_height_absolute(22.75)
        .font_weight(FontWeight::W400)
        .color(color)
}

fn assistant_message(body: &str, active: Option<&Jump>) -> Widget {
    // TimelineAssistantRows.tsx: px-1 py-0.5. The plain ChatMarkdown root
    // uses the interface font, text-sm / leading-relaxed and 90% foreground
    // at default contrast. No background on the assistant's message.
    let style = message_style(ASSISTANT_MESSAGE_INK);
    let child = match active {
        Some(jump) => marked_message(body, style, jump),
        None => Text::new(body).style(style).soft_wrap(true).into(),
    };
    let row: Widget = Container::new()
        .padding(EdgeInsets::symmetric(4.0, 2.0))
        .alignment(Alignment::CENTER_LEFT)
        .child(child)
        .into();
    match active {
        Some(jump) => row.with_key(jump.key),
        None => row,
    }
}

fn user_message(body: &str, lane_width: f32, active: Option<&Jump>) -> Widget {
    // Pinned TimelineUserMessageRow.tsx plain text path: max-w-[80%],
    // rounded-2xl, p-3 and bg-message. Folded/attachment variants are separate.
    let style = message_style(Color::rgba(24, 24, 27, 255));
    let child = match active {
        Some(jump) => marked_message(body, style, jump),
        None => Text::new(body).style(style).soft_wrap(true).into(),
    };
    let bubble: Widget = Container::new()
        .padding(EdgeInsets::all(12.0))
        .decoration(
            BoxDecoration::new()
                .color(USER_MESSAGE_SURFACE)
                .border_radius(BorderRadius::circular(16.0)),
        )
        .child(child)
        .into();
    let bubble = match active {
        Some(jump) => bubble.with_key(jump.key),
        None => bubble,
    };
    Align::new(
        Alignment::CENTER_RIGHT,
        ConstrainedBox::new(
            Constraints::new(0.0, lane_width * 0.8, 0.0, f32::INFINITY),
            bubble,
        ),
    )
    .into()
}

fn marked_message(body: &str, base: TextStyle, jump: &Jump) -> Widget {
    let Some((range, matched)) = jump
        .occurrence()
        .map(|m| &m.range)
        .and_then(|range| body.get(range.clone()).map(|matched| (range, matched)))
    else {
        return Text::new(body).style(base).into();
    };
    let active = base
        .clone()
        .color(Color::rgba(33, 24, 10, 255))
        .background_color(Color::rgba(238, 165, 43, 255));
    RichText::new(TextSpan::empty().style(base).children([
        TextSpan::new(&body[..range.start]),
        TextSpan::new(matched).style(active),
        TextSpan::new(&body[range.end..]),
    ]))
    .into()
}

#[cfg(test)]
#[test]
fn saved_user_bubbles_fit_content_and_wrap_at_the_source_width() {
    use incular::core::Transform;
    use incular::rendering::{Brush, PaintCommand};
    use incular::runtime::Runtime;

    let lane = 400.0;
    let widget = Directionality::new(
        TextDirection::Ltr,
        Column::new([
            user_message("Short prompt", lane, None),
            user_message(&"A saved prompt with spaces. ".repeat(8), lane, None),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Stretch),
    );
    let mut runtime = Runtime::new(widget.into()).unwrap();
    let (display, _) = runtime
        .run_frame(Constraints::tight(Size::new(lane, 320.0)))
        .unwrap();
    let mut transforms = vec![Transform::IDENTITY];
    let mut boxes = Vec::new();
    for command in display.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(transforms.last().unwrap().then(*transform));
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::RRect {
                rrect,
                brush: Brush::Solid(color),
            } if *color == USER_MESSAGE_SURFACE => {
                let pos = transforms
                    .last()
                    .unwrap()
                    .transform_point(rrect.rect.origin);
                assert_eq!(rrect.radii.top_right, 16.0);
                boxes.push((pos, rrect.rect.size));
            }
            _ => {}
        }
    }
    assert_eq!(boxes.len(), 2);
    let [(short_position, short), (long_position, long)] = boxes[..] else {
        unreachable!()
    };
    assert!(short.width < 160.0 && short.height >= 46.0, "{short:?}");
    assert!((310.0..=320.0).contains(&long.width), "{long:?}");
    assert!(long.height > short.height + 20.0, "{long:?}");
    assert!((short_position.x + short.width - lane).abs() < 1.0);
    assert!((long_position.x + long.width - lane).abs() < 1.0);
}

#[cfg(test)]
#[test]
fn saved_assistant_reply_paints_left_inset_and_relaxed_lines_without_a_bubble() {
    use incular::core::Transform;
    use incular::rendering::PaintCommand;
    use incular::runtime::Runtime;

    let row = assistant_message(&"Plain saved assistant response. ".repeat(5), None);
    let root = Directionality::new(TextDirection::Ltr, Column::new([row]));
    let mut runtime = Runtime::new(root.into()).unwrap();
    let (display, _) = runtime
        .run_frame(Constraints::tight(Size::new(150.0, 280.0)))
        .unwrap();
    let mut transforms = vec![Transform::IDENTITY];
    let mut baselines = Vec::<f32>::new();
    for command in display.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                transforms.push(transforms.last().unwrap().then(*transform));
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::RRect { .. } => panic!("assistant text must not paint a bubble"),
            PaintCommand::GlyphRun { run, color } if *color == ASSISTANT_MESSAGE_INK => {
                let origin = transforms.last().unwrap().transform_point(run.origin);
                assert!((origin.x - 4.0).abs() < 1.0);
                for glyph in run.glyphs.iter() {
                    let y = transforms
                        .last()
                        .unwrap()
                        .transform_point(run.origin + glyph.offset)
                        .y;
                    if !baselines.iter().any(|last| (*last - y).abs() < 0.1) {
                        baselines.push(y);
                    }
                }
            }
            _ => {}
        }
    }
    assert!(baselines.len() > 3, "{baselines:?}");
    baselines.sort_by(f32::total_cmp);
    for pair in baselines.windows(2) {
        assert!((pair[1] - pair[0] - 22.75).abs() < 0.4, "{pair:?}");
    }
}

fn find_bar(
    turn: &Rc<Conversation>,
    jump: &Jump,
    state: Signal<Option<Jump>>,
    input: TextEditingController,
    width: f32,
) -> Widget {
    let turn = turn.clone();
    let thread = turn.id;
    let on_query = state.clone();
    let on_step = state.clone();
    crate::thread_find_bar::view(
        input,
        jump.count(),
        jump.occurrence().is_some(),
        width,
        move |text| {
            if let Some(open) = on_query
                .get()
                .filter(|open| open.thread == thread && open.query != text)
            {
                on_query.set(Some(open.edited(&turn, text)));
            }
        },
        move |forward| {
            if let Some(next) = on_step
                .get()
                .filter(|open| open.thread == thread)
                .and_then(|open| open.stepped(forward))
            {
                on_step.set(Some(next));
            }
        },
        move || {
            if state.get().is_some_and(|open| open.thread == thread) {
                state.set(None);
            }
        },
    )
}
