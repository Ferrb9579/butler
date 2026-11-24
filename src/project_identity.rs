//! T3 v0.1.11 project monograms: projectIdentity.ts, projectIconColors.ts,
//! and components/ProjectMonogram.tsx.

use icu_normalizer::ComposingNormalizer;
use icu_properties::{
    CodePointMapData,
    props::{GeneralCategory, GeneralCategoryGroup},
};
use icu_segmenter::GraphemeClusterSegmenter;
use incular::{prelude::*, text::TextEngine};
use std::cell::RefCell;

const EXTENT: f32 = 16.0;
const TEXT_BASELINE: f32 = 10.8;
const BACKGROUND_ALPHA: u8 = 36; // 14% in Incular's encoded sRGBA8.

// Order is part of projectIdentity.ts's persisted-looking name hash. Values
// are the bundled main-C7LLcwbG.css Tailwind oklch() -600 (light) and -400
// (dark) tokens converted to sRGB, clipped to the native sRGBA8 gamut and
// rounded to the nearest channel value.
const COLORS: [([u8; 3], [u8; 3]); 18] = [
    ([74, 85, 101], [153, 161, 175]),  // gray
    ([231, 0, 11], [255, 100, 103]),   // red
    ([245, 73, 0], [255, 137, 4]),     // orange
    ([225, 113, 0], [255, 185, 0]),    // amber
    ([208, 135, 0], [253, 199, 0]),    // yellow
    ([94, 165, 0], [154, 230, 0]),     // lime
    ([0, 166, 62], [5, 223, 114]),     // green
    ([0, 153, 102], [0, 212, 146]),    // emerald
    ([0, 150, 137], [0, 213, 190]),    // teal
    ([0, 146, 184], [0, 211, 242]),    // cyan
    ([0, 132, 209], [0, 188, 255]),    // sky
    ([21, 93, 252], [81, 162, 255]),   // blue
    ([79, 57, 246], [124, 134, 255]),  // indigo
    ([127, 34, 254], [166, 132, 255]), // violet
    ([152, 16, 250], [194, 122, 255]), // purple
    ([200, 0, 222], [237, 106, 255]),  // fuchsia
    ([230, 0, 118], [251, 100, 182]),  // pink
    ([236, 0, 63], [255, 99, 126]),    // rose
];

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    text: String,
    palette: usize,
}

fn identity(project_name: &str) -> Identity {
    let normalized = ComposingNormalizer::new_nfkc().normalize(project_name);
    let normalized =
        normalized.trim_matches(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}'));
    let categories = CodePointMapData::<GeneralCategory>::new();
    let is_number = |c| GeneralCategoryGroup::Number.contains(categories.get(c));
    let mut words = normalized
        .split(|c| !GeneralCategoryGroup::Letter.contains(categories.get(c)) && !is_number(c))
        .filter(|word| !word.is_empty());
    let text = if let Some(word) = words.next() {
        let mut glyphs = word.chars();
        let first = glyphs.next().expect("nonempty word");
        let later_number = glyphs.find(|&c| is_number(c));
        let second = later_number
            .or_else(|| words.last().and_then(|word| word.chars().next()))
            .or_else(|| word.chars().next_back())
            .unwrap_or(first);
        [first, second]
            .into_iter()
            .flat_map(char::to_uppercase)
            .take(2)
            .collect()
    } else {
        "PR".to_owned()
    };

    // String (rather than char-at-a-time) lowercase handles contextual
    // final sigma. Default Unicode lowercase is the en-US mapping here.
    let seed = if normalized.is_empty() {
        "project"
    } else {
        normalized
    };
    let palette = seed.to_lowercase().chars().fold(0, |index, glyph| {
        (31 * index + glyph as usize) % COLORS.len()
    });
    Identity { text, palette }
}

fn foreground(index: usize, dark: bool) -> Color {
    let (light, darker) = COLORS[index];
    let [r, g, b] = if dark { darker } else { light };
    Color::rgba(r, g, b, 255)
}

fn text_advance(text: &str) -> f32 {
    // Segmenter yields zero plus each successive UTF-8 grapheme end.
    if GraphemeClusterSegmenter::new().segment_str(text).nth(1) == Some(text.len()) {
        6.0
    } else {
        12.0
    }
}

thread_local! {
    static TEXT_ENGINE: RefCell<TextEngine> = RefCell::new(TextEngine::new());
}

/// Paints the decorative, non-interactive project monogram fallback at
/// the requested square size. The adjacent project title supplies its name.
pub fn widget(project_name: &str, size: f32, dark: bool) -> Widget {
    let size = if size.is_finite() { size.max(0.0) } else { 0.0 };
    if size == 0.0 {
        return SizedBox::new().width(0.0).height(0.0).into();
    }
    let Identity { text, palette } = identity(project_name);
    let ink = foreground(palette, dark);
    let tint = Color::rgba(ink.red, ink.green, ink.blue, BACKGROUND_ALPHA);
    let mut canvas = Canvas::default();
    let tile = RRect::uniform(
        Rect::from_origin_size(Offset::ZERO, Size::new(size, size)),
        size * 0.25,
    );
    canvas.rrect(tile, tint);

    let style = TextStyle::new()
        .font_family("ui-monospace")
        .fallback_family(FontFamily::named("SF Mono"))
        .fallback_family(FontFamily::named("SFMono-Regular"))
        .fallback_family(FontFamily::named("Menlo"))
        .fallback_family(FontFamily::named("Consolas"))
        .fallback_family(FontFamily::named("Liberation Mono"))
        .fallback_family(FontFamily::Monospace)
        .font_size(8.25)
        .font_weight(FontWeight::W700);
    let layout =
        TEXT_ENGINE.with_borrow_mut(|engine| engine.layout(&text, &style, None, TextAlign::Left));
    if let Some(line) = layout.lines.first().filter(|line| line.width > 0.0) {
        let scale = size / EXTENT;
        let advance = text_advance(&text);
        // SVG text-anchor="middle", y=10.8, lengthAdjust="spacingAndGlyphs".
        // The text engine puts the first line's glyphs on its rounded paint
        // baseline (engine.rs::snap_line_boxes), not its reported metrics baseline.
        let x = scale * (EXTENT * 0.5 - advance * 0.5);
        let y = scale * (TEXT_BASELINE - line.baseline.round());
        canvas.save_clip_rrect(tile);
        canvas.save_transform(AffineTransform::translation(Offset::new(x, y)).then(
            AffineTransform::scale_non_uniform(scale * advance / line.width, scale),
        ));
        for run in line.runs.iter() {
            canvas.glyph_run(run.clone(), ink);
        }
        canvas.restore();
        canvas.restore();
    }
    Widget::from(CustomPaint::new(Size::new(size, size), canvas.finish())).exclude_semantics()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_word_rules_and_modular_palette_in_source_order() {
        for (input, text, palette) in [
            ("T3 Code", "T3", 16),
            ("Project Alpha", "PA", 7),
            ("one two three", "OT", 14),
            ("\u{feff}ＰＲ１ tools  ", "P1", 10),
            ("a3b4", "A3", 16),
            ("ŵ", "ŴŴ", 13),
            ("ßolo", "SS", 13),
            ("\u{345}Alpha", "AA", 11), // Alphabetic combining mark is not a Letter.
            ("ΟΣ", "ΟΣ", 1),            // Contextual final sigma affects the color.
            ("İstanbul", "İL", 15),     // Unicode lowercase expands dotted I in the hash.
            ("한글 프로젝트", "한프", 7),
            ("𐐨 Test", "𐐀T", 6), // Astral scalars count once in the hash.
            ("🧪", "PR", 4),
        ] {
            assert_eq!(
                identity(input),
                Identity {
                    text: text.into(),
                    palette
                },
                "{input}"
            );
        }
        assert_eq!(identity("  "), identity("project").with_text("PR"));
    }

    impl Identity {
        fn with_text(mut self, text: &str) -> Self {
            self.text = text.into();
            self
        }
    }

    #[test]
    fn widget_paints_scaled_viewbox_and_svg_text_baseline() {
        use incular::{rendering::PaintCommand, runtime::Runtime};

        let mut runtime = Runtime::new(widget("Project Alpha", 14.0, false)).unwrap();
        let (list, _) = runtime
            .run_frame(Constraints::tight(Size::new(14.0, 14.0)))
            .unwrap();
        let mut transforms = vec![AffineTransform::IDENTITY];
        let mut saw_tile = false;
        let mut saw_text = false;
        for command in list.commands() {
            match command {
                PaintCommand::RRect { rrect, .. } => {
                    saw_tile |= *rrect
                        == RRect::uniform(
                            Rect::from_origin_size(Offset::ZERO, Size::new(14.0, 14.0)),
                            3.5,
                        );
                }
                PaintCommand::PushTransform { transform } => {
                    transforms.push(transforms.last().unwrap().then(*transform));
                }
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::GlyphRun { run, color } if *color == foreground(7, false) => {
                    let transform = transforms.last().unwrap();
                    let glyph = run.glyphs.first().unwrap();
                    let laid_out_baseline =
                        run.origin + Offset::new(glyph.offset.x, -glyph.offset.y);
                    let point = transform.transform_point(laid_out_baseline);
                    assert!((point.x - 1.75).abs() < 0.01, "{point:?}");
                    assert!((point.y - 9.45).abs() < 0.01, "{point:?}");
                    saw_text = true;
                }
                _ => {}
            }
        }
        assert!(saw_tile && saw_text);
    }

    #[test]
    fn palette_modes_and_grapheme_advances_match_the_reference() {
        assert_eq!(foreground(7, false), Color::rgba(0, 153, 102, 255));
        assert_eq!(foreground(7, true), Color::rgba(0, 212, 146, 255));
        assert_eq!(foreground(0, false), Color::rgba(74, 85, 101, 255));
        assert_eq!(foreground(17, true), Color::rgba(255, 99, 126, 255));
        assert_eq!(text_advance("PA"), 12.);
        assert_eq!(text_advance("가"), 6.);
    }
}
