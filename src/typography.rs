//! T3 Code's bundled Latin Inter variable face, with native platform fallback.

use incular::prelude::*;
use std::cell::Cell;

const INTER_LATIN: &[u8] = include_bytes!("../assets/Inter-Latin-Variable.ttf");

thread_local! {
    static REGISTERED: Cell<bool> = const { Cell::new(false) };
}

pub fn register() {
    REGISTERED.with(|flag| {
        if !flag.replace(true) {
            incular::text::register_application_font(INTER_LATIN);
        }
    });
}

pub fn interface() -> TextStyle {
    TextStyle::new()
        .font_family("Inter")
        .fallback_family(FontFamily::SystemUi)
        .fallback_family(FontFamily::SansSerif)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_text_resolves_to_bundled_inter_in_regular_and_semibold() {
        register();
        let mut engine = incular::text::TextEngine::new();
        for weight in [FontWeight::W400, FontWeight::W600] {
            let layout = engine.layout(
                "Provider display name",
                &interface().font_size(14.0).font_weight(weight),
                None,
                TextAlign::Start,
            );
            let line = &layout.lines[0];
            assert!(!line.runs.is_empty());
            assert!(
                line.runs
                    .iter()
                    .all(|run| run.font.bytes().as_ref() == INTER_LATIN)
            );
        }
    }
}
