//! Butler's buttons: composition from Incular widgets, not a framework design system.
//! Light variants are sourced from T3 v0.1.11 components/ui/button.tsx and index.css.

use incular::{
    prelude::*,
    widgets::{
        MouseCursor,
        internal::{ActionInteractionController, ActionPolicy, ActionSplashPolicy, ActionSurface},
    },
};
use std::{rc::Rc, time::Duration};

const ACCENT: Color = Color::rgba(244, 244, 245, 255);
const PRIMARY: Color = Color::rgba(27, 78, 216, 255);
const INPUT: Color = Color::rgba(212, 212, 216, 255);

#[derive(Clone, Copy, Default)]
pub enum ButtonVariant {
    #[default]
    Outline,
    Ghost,
    Primary,
}

#[derive(Clone)]
pub struct ButtonStyle {
    background: Color,
    foreground: Color,
    padding: EdgeInsets,
    height: f32,
    width: Option<f32>,
    radius: f32,
    variant: ButtonVariant,
    alignment: Alignment,
    text: TextStyle,
}

impl ButtonStyle {
    pub fn new() -> Self {
        Self {
            background: Color::rgba(255, 255, 255, 255),
            foreground: Color::rgba(24, 24, 27, 255),
            padding: EdgeInsets::symmetric(12.0, 0.0),
            height: 32.0,
            width: None,
            radius: 8.0,
            variant: ButtonVariant::Outline,
            alignment: Alignment::CENTER,
            text: TextStyle::new()
                .font_family("Liberation Sans")
                .font_size(14.0)
                .line_height_absolute(20.0)
                .font_weight(FontWeight::W500),
        }
    }

    pub fn variant(mut self, value: ButtonVariant) -> Self {
        self.variant = value;
        self
    }
    pub fn height(mut self, value: f32) -> Self {
        self.height = value;
        self
    }
    pub fn fixed_size(mut self, value: Size) -> Self {
        self.width = Some(value.width);
        self.height = value.height;
        self
    }
    pub fn background(mut self, value: Color) -> Self {
        self.background = value;
        self
    }
    pub fn foreground(mut self, value: Color) -> Self {
        self.foreground = value;
        self
    }
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = value;
        self
    }
    pub fn border_radius(mut self, value: f32) -> Self {
        self.radius = value;
        self
    }
    pub fn alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.text = value;
        self
    }
}

pub struct Button {
    label: String,
    child: Option<Widget>,
    style: ButtonStyle,
    click: Option<Rc<dyn Fn()>>,
    enabled: bool,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            child: None,
            style: ButtonStyle::new(),
            click: None,
            enabled: true,
        }
    }

    pub fn with_child(label: impl Into<String>, child: Widget) -> Self {
        Self {
            child: Some(child),
            ..Self::new(label)
        }
    }

    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
    pub fn on_click(mut self, click: impl Fn() + 'static) -> Self {
        self.click = Some(Rc::new(click));
        self
    }
}

impl From<Button> for Widget {
    fn from(button: Button) -> Self {
        let Button {
            label,
            child,
            style,
            click,
            enabled,
        } = button;
        let active = enabled && click.is_some();
        let state = ActionInteractionController::new();
        let presentation = {
            let label = label.clone();
            let interaction = state.clone();
            Widget::stateful_builder(state.revision(), move |_| {
                let s = interaction.state();
                let _ = interaction.take_ink_events();
                let (fill, border) = match style.variant {
                    ButtonVariant::Outline => (
                        if s.hovered || s.pressed {
                            Color::rgba(249, 249, 250, 255)
                        } else {
                            style.background
                        },
                        INPUT,
                    ),
                    ButtonVariant::Ghost => (
                        if s.hovered || s.pressed {
                            ACCENT
                        } else {
                            style.background
                        },
                        Color::TRANSPARENT,
                    ),
                    ButtonVariant::Primary => (
                        if s.hovered || s.pressed {
                            Color::rgba(50, 96, 220, 255)
                        } else {
                            style.background
                        },
                        PRIMARY,
                    ),
                };
                let decoration = BoxDecoration::new()
                    .color(fill)
                    .border(if s.focused && active {
                        Border::new(2.0, PRIMARY)
                    } else {
                        Border::new(1.0, border)
                    })
                    .border_radius(BorderRadius::all(Radius::circular(style.radius)));
                let label = child.clone().unwrap_or_else(|| {
                    Text::new(label.clone())
                        .style(style.text.clone().color(style.foreground))
                        .into()
                });
                let mut view = Container::new()
                    .height(style.height)
                    .padding(style.padding)
                    .alignment(style.alignment)
                    .decoration(decoration)
                    .child(label);
                if let Some(width) = style.width {
                    view = view.width(width);
                }
                let view: Widget = view.into();
                if active {
                    view
                } else {
                    Opacity::new(0.64, view).into()
                }
            })
        };
        let mut surface = ActionSurface::with_child(presentation)
            .label(label)
            .enabled(active)
            .mouse_cursor(if active {
                MouseCursor::Click
            } else {
                MouseCursor::Basic
            })
            .interaction_controller(state)
            .policy(ActionPolicy {
                feedback_enabled: false,
                splash: ActionSplashPolicy::None,
                transition_duration: Duration::ZERO,
            });
        if let Some(callback) = click.filter(|_| active) {
            surface = surface.on_press(move || callback());
        }
        surface.into()
    }
}
