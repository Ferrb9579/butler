use incular::prelude::*;

const CANVAS: Color = Color::rgba(0, 0, 0, 255);
const SURFACE: Color = Color::rgba(18, 18, 18, 255);
const FOREGROUND: Color = Color::rgba(245, 245, 245, 255);
const MUTED: Color = Color::rgba(163, 163, 163, 255);
const SIDEBAR_WIDTH: f32 = 256.0;

pub fn view(search: TextEditingController, prompt: TextEditingController) -> Widget {
    Row::new(vec![sidebar(search), Expanded::new(thread(prompt)).into()])
        .alignment(CrossAxisAlignment::Stretch)
        .into()
}

fn sidebar(search: TextEditingController) -> Widget {
    Container::new()
        .width(SIDEBAR_WIDTH)
        .color(SURFACE)
        .padding(EdgeInsets::all(12.0))
        .child(Column::new(Vec::<Widget>::from([
            editor(search, "Search"),
            SizedBox::new().height(16.0).into(),
            label("Projects", 12.0, MUTED),
        ])))
        .into()
}

fn thread(prompt: TextEditingController) -> Widget {
    let composer = Container::new()
        .width(720.0)
        .padding(EdgeInsets::all(10.0))
        .decoration(
            BoxDecoration::new()
                .color(SURFACE)
                .border_radius(BorderRadius::all(Radius::circular(16.0))),
        )
        .child(editor(prompt, "Ask anything..."));

    Container::new()
        .color(CANVAS)
        .padding(EdgeInsets::all(16.0))
        .child(Column::new(Vec::<Widget>::from([
            Container::new()
                .height(32.0)
                .alignment(Alignment::CENTER_LEFT)
                .child(label("New thread", 14.0, FOREGROUND))
                .into(),
            Spacer::new().into(),
            Container::new()
                .alignment(Alignment::CENTER)
                .child(composer)
                .into(),
        ])))
        .into()
}

fn label(text: &str, size: f32, color: Color) -> Widget {
    Text::new(text)
        .style(TextStyle::new().font_size(size).color(color))
        .into()
}

fn editor(controller: TextEditingController, placeholder: &str) -> Widget {
    EditableText::new(controller)
        .placeholder(placeholder)
        .placeholder_color(MUTED)
        .style(TextStyle::new().font_size(14.0).color(FOREGROUND))
        .into()
}
