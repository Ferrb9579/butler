//! Native empty workspace, matched against the pinned Linux desktop reference.

use crate::settings_icons::icon;
use crate::ui::{Button, ButtonStyle, ButtonVariant};
use incular::prelude::*;

pub(crate) const PAGE: Color = Color::rgba(252, 252, 252, 255);
const SIDEBAR: Color = Color::rgba(249, 249, 249, 255);
pub(crate) const RULE: Color = Color::rgba(228, 228, 231, 255);
pub(crate) const INK: Color = Color::rgba(38, 38, 38, 255);
pub(crate) const MUTED: Color = Color::rgba(113, 113, 122, 255);
const FAINT: Color = Color::rgba(161, 161, 170, 255);
const BLUE: Color = Color::rgba(27, 78, 216, 255);
pub(crate) const WHITE: Color = Color::rgba(255, 255, 255, 255);

pub fn view(
    scratch_available: bool,
    open_scratch: impl Fn() + 'static,
    open_project: impl Fn() + Clone + 'static,
    open_settings: impl Fn() + 'static,
) -> Widget {
    let mut buttons = vec![project_button(
        "Add project",
        116.0,
        28.0,
        BLUE,
        WHITE,
        open_project.clone(),
    )];
    if scratch_available {
        buttons.push(SizedBox::new().width(8.0).into());
        buttons.push(
            Button::with_child(
                "Start without a project",
                icon_label(
                    "message-square-dashed",
                    16.0,
                    "Start without a project",
                    14.0,
                    INK,
                ),
            )
            .style(style(28.0, WHITE, INK).fixed_size(Size::new(186.0, 28.0)))
            .on_click(open_scratch)
            .into(),
        );
    }
    let hero = Column::new(vec![
        label("What should we work on?", 30.0, FontWeight::W600, INK),
        space(4.0),
        label(
            if scratch_available {
                "Add a project, or start without one."
            } else {
                "Add a project to start your first thread."
            },
            14.0,
            FontWeight::W400,
            MUTED,
        ),
        space(24.0),
        Row::new(buttons)
            .main_axis_alignment(MainAxisAlignment::Center)
            .into(),
    ])
    .main_axis_size(MainAxisSize::Min);
    Row::new(vec![
        sidebar(
            None,
            Some(open_project),
            None::<fn()>,
            open_settings,
            None,
            Vec::new(),
        ),
        Container::new().width(1.0).color(RULE).into(),
        Expanded::new(
            Container::new().color(PAGE).child(
                Column::new(vec![
                    space(40.0),
                    Expanded::new(Container::new().alignment(Alignment::CENTER).child(hero)).into(),
                ])
                .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ),
        )
        .into(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .into()
}

pub(crate) fn sidebar(
    project: Option<&str>,
    open_project: Option<impl Fn() + 'static>,
    new_thread: Option<impl Fn() + 'static>,
    open_settings: impl Fn() + 'static,
    search_input: Option<Widget>,
    threads: Vec<Widget>,
) -> Widget {
    let empty_caption = project.unwrap_or(if open_project.is_some() {
        "No projects yet"
    } else {
        if threads.is_empty() {
            "No threads yet"
        } else {
            "Threads"
        }
    });
    let project_caption = if let Some(title) = project {
        let name: Widget = Text::new(title)
            .style(
                crate::typography::interface()
                    .font_size(14.0)
                    .font_weight(FontWeight::W500)
                    .color(INK),
            )
            .max_lines(Some(1))
            .overflow(TextOverflow::Ellipsis)
            .into();
        Container::new()
            .height(32.0)
            .padding(EdgeInsets::symmetric(16.0, 0.0))
            .child(
                Row::new(vec![
                    crate::project_identity::widget(title, 16.0, false),
                    Expanded::new(name).into(),
                ])
                .spacing(10.0)
                .cross_axis_alignment(CrossAxisAlignment::Center),
            )
            .into()
    } else {
        label(empty_caption, 12.0, FontWeight::W400, FAINT)
    };
    let (action, header_add) = match open_project {
        Some(callback) if new_thread.is_some() && project.is_some() => (
            None,
            Some(
                Button::with_child("Add project", icon("folder-plus", 16.0, MUTED))
                    .style(
                        ButtonStyle::new()
                            .variant(ButtonVariant::Ghost)
                            .fixed_size(Size::new(28.0, 28.0))
                            .background(SIDEBAR),
                    )
                    .on_click(callback)
                    .into(),
            ),
        ),
        Some(callback) => (
            Some(project_button(
                "Add project",
                96.0,
                26.0,
                SIDEBAR,
                MUTED,
                callback,
            )),
            None,
        ),
        None => (None, None),
    };
    let search_field = if let Some(field) = search_input {
        Widget::from(
            Container::new()
                .height(48.0)
                .alignment(Alignment::CENTER_LEFT)
                .child(field),
        )
    } else {
        line(icon_label("search", 16.0, "Search", 14.0, MUTED), 48.0)
    };
    let search = if let Some(callback) = new_thread {
        let mut controls = vec![Widget::from(Expanded::new(search_field))];
        controls.extend(header_add);
        controls.extend([
            Button::with_child("New thread", icon("square-pen", 16.0, MUTED))
                .style(
                    ButtonStyle::new()
                        .variant(ButtonVariant::Ghost)
                        .fixed_size(Size::new(28.0, 28.0))
                        .background(SIDEBAR),
                )
                .on_click(callback)
                .into(),
            SizedBox::new().width(12.0).into(),
        ]);
        Row::new(controls)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into()
    } else {
        search_field
    };
    Container::new()
        .width(255.0)
        .color(SIDEBAR)
        .child(Column::new(vec![
            line(
                Row::new(vec![
                    label("Butler", 14.0, FontWeight::W500, INK),
                    SizedBox::new().width(8.0).into(),
                    toolbar_glyph("arrow-left", 14.0, FAINT),
                    toolbar_glyph("arrow-right", 14.0, FAINT),
                ])
                .main_axis_size(MainAxisSize::Min)
                .into(),
                40.0,
            ),
            search,
            space(24.0),
            project_caption,
            space(8.0),
            action.unwrap_or_else(|| space(26.0)),
            Expanded::new(SingleChildScrollView::new(
                Column::new(threads)
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
            ))
            .into(),
            line(
                Row::new(vec![
                    Button::with_child("Settings", icon("settings", 16.0, MUTED))
                        .style(
                            ButtonStyle::new()
                                .variant(ButtonVariant::Ghost)
                                .fixed_size(Size::new(28.0, 28.0))
                                .background(SIDEBAR),
                        )
                        .on_click(open_settings)
                        .into(),
                    toolbar_glyph("chart-no-axes-column", 16.0, MUTED),
                ])
                .main_axis_size(MainAxisSize::Min)
                .into(),
                48.0,
            ),
        ]))
        .into()
}

fn line(child: Widget, height: f32) -> Widget {
    Container::new()
        .width(255.0)
        .height(height)
        .padding(EdgeInsets::symmetric(16.0, 0.0))
        .alignment(Alignment::CENTER_LEFT)
        .child(child)
        .into()
}

fn project_button(
    text: &str,
    width: f32,
    height: f32,
    background: Color,
    ink: Color,
    click: impl Fn() + 'static,
) -> Widget {
    Button::with_child(
        text,
        icon_label(
            "plus",
            if width < 100.0 { 12.0 } else { 16.0 },
            text,
            if width < 100.0 { 12.0 } else { 14.0 },
            ink,
        ),
    )
    .style(
        style(height, background, ink)
            .variant(if width == 116.0 {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Ghost
            })
            .fixed_size(Size::new(width, height)),
    )
    .on_click(click)
    .into()
}

fn toolbar_glyph(name: &str, size: f32, color: Color) -> Widget {
    Container::new()
        .width(28.0)
        .height(28.0)
        .alignment(Alignment::CENTER)
        .child(icon(name, size, color))
        .into()
}

pub(crate) fn icon_label(
    name: &str,
    size: f32,
    text: &str,
    text_size: f32,
    color: Color,
) -> Widget {
    Row::new(vec![
        icon(name, size, color),
        SizedBox::new().width(6.0).into(),
        label(text, text_size, FontWeight::W500, color),
    ])
    .main_axis_size(MainAxisSize::Min)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .into()
}

fn style(height: f32, background: Color, ink: Color) -> ButtonStyle {
    ButtonStyle::new()
        .height(height)
        .background(background)
        .foreground(ink)
        .padding(EdgeInsets::symmetric(8.0, 0.0))
        .text_style(
            TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(14.0),
        )
        .border_radius(8.0)
}

pub(crate) fn label(text: &str, size: f32, weight: FontWeight, color: Color) -> Widget {
    Text::new(text)
        .style(
            TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(size)
                .font_weight(weight)
                .color(color),
        )
        .into()
}

pub(crate) fn space(height: f32) -> Widget {
    SizedBox::new().height(height).into()
}
