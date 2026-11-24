//! Draft immediately after "Start without a project" on a profile with no provider.
//! Reference: T3 Code v0.1.11 desktop, native fixture 45329928604.

use incular::prelude::*;

use crate::empty_workspace::{INK, MUTED, PAGE, RULE, WHITE, label, sidebar, space};

pub fn draft(
    project: Option<String>,
    open_settings: impl Fn() + Clone + 'static,
    #[cfg(test)] offline: Option<OfflineDraft>,
) -> Widget {
    let title = project.clone().unwrap_or_else(|| "No project".into());
    Row::new(vec![
        sidebar(project.as_deref(), None::<fn()>),
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
                                &format!("⬚ {title}   /   New thread"),
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
    pub sent: Option<String>,
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
    if let Some(sent) = draft.sent {
        rows.extend([
            label(&sent, 14.0, FontWeight::W500, INK),
            space(8.0),
            label("Offline test reply received.", 14.0, FontWeight::W400, INK),
        ]);
    } else {
        let control = draft.input.clone();
        rows.extend([
            EditableText::new(draft.input)
                .placeholder("Ask anything")
                .style(TextStyle::new().font_size(14.0).color(INK))
                .into(),
            crate::provider_settings::action("Send", move || (draft.send)(control.text())),
        ]);
    }
    if let Some(error) = draft.error {
        rows.push(label(&error, 14.0, FontWeight::W400, MUTED));
    }
    Column::new(rows)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .into()
}
