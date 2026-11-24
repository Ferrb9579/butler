//! Inline title search over this workspace's saved Butler threads.
//! Source: T3 Code v0.1.11 SidebarThreadHeader.tsx, Sidebar.tsx, and paletteSearch.ts.

use crate::{
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use butler::threads::ThreadSummary;
use icu_normalizer::{
    ComposingNormalizer, DecomposingNormalizer, properties::CanonicalCombiningClassMap,
};
use incular::{
    gestures::GestureCallbacks,
    prelude::*,
    widgets::{MouseRegion, internal::ExplicitSemantics},
};
use std::{cell::Cell, rc::Rc};
use uuid::Uuid;

const BACKGROUND_HOVER: Color = Color::rgba(252, 252, 252, 255);
const INK: Color = Color::rgba(24, 24, 27, 255);
const MUTED: Color = Color::rgba(113, 113, 123, 255);
const ICON: Color = Color::rgba(133, 133, 142, 255);
const RING: Color = Color::rgba(27, 78, 216, 179);
const HEIGHT: f32 = 32.0;
const FIELD_LABEL: &str = "Search threads";

#[derive(Clone)]
pub struct Search {
    controller: TextEditingController,
    highlighted: Rc<Cell<Option<Uuid>>>,
    revision: Rc<Cell<u64>>,
    hovered: Rc<Cell<bool>>,
    focused: Rc<Cell<bool>>,
}

impl Default for Search {
    fn default() -> Self {
        Self::new()
    }
}

impl Search {
    pub fn new() -> Self {
        Self {
            controller: TextEditingController::new(),
            highlighted: Rc::new(Cell::new(None)),
            revision: Rc::new(Cell::new(0)),
            hovered: Rc::new(Cell::new(false)),
            focused: Rc::new(Cell::new(false)),
        }
    }

    pub fn reset(&self) {
        self.controller.clear();
        self.highlighted.set(None);
        self.invalidate();
    }

    pub fn query(&self) -> String {
        self.controller.text()
    }

    /// Raw nonblank input replaces the ordinary list, even if punctuation
    /// normalization leaves no searchable terms.
    pub fn active(&self) -> bool {
        !self.query().trim_matches(js_whitespace).is_empty()
    }

    /// Observe with Widget::stateful_builder around the sidebar header and results.
    pub fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    pub fn highlighted(&self, ids: &[Uuid]) -> Option<Uuid> {
        self.highlighted
            .get()
            .filter(|id| ids.contains(id))
            .or_else(|| ids.first().copied())
    }

    pub fn highlight(&self, id: Uuid) {
        if self.highlighted.replace(Some(id)) != Some(id) {
            self.invalidate();
        }
    }

    fn invalidate(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    fn move_highlight(&self, ids: &[Uuid], forward: bool) {
        if let Some(id) = wrapped_highlight(ids, self.highlighted(ids), forward) {
            self.highlight(id);
        }
    }
}

fn wrapped_highlight(ids: &[Uuid], selected: Option<Uuid>, forward: bool) -> Option<Uuid> {
    if ids.is_empty() {
        return None;
    }
    let current = selected
        .and_then(|id| ids.iter().position(|entry| *entry == id))
        .unwrap_or(0);
    let index = if forward {
        (current + 1) % ids.len()
    } else {
        (current + ids.len() - 1) % ids.len()
    };
    Some(ids[index])
}

/// Source-based title filtering for an already loaded, one-workspace history.
/// Exact names outrank words/prefixes; weak interior hits use a four-character
/// minimum. Equal matches retain the store's recent-first order. No content,
/// branch/PR metadata, fuzzy typo pass, or usage learning is represented here.
pub fn matching_threads<'a>(history: &'a [ThreadSummary], query: &str) -> Vec<&'a ThreadSummary> {
    let query = fold_query(query);
    if query.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<Vec<char>> = query
        .split(' ')
        .map(|word| word.chars().collect())
        .collect();
    let optional_fillers = tokens.iter().any(|word| !filler(word));
    let mut rows: Vec<_> = history
        .iter()
        .filter_map(|thread| {
            title_score(&thread.title, &query, &tokens, optional_fillers)
                .map(|score| (thread, score))
        })
        .collect();
    rows.sort_by(|(_, left), (_, right)| {
        right
            .tier
            .cmp(&left.tier)
            .then_with(|| right.whole_words.cmp(&left.whole_words))
            .then_with(|| right.quality.total_cmp(&left.quality))
    });
    rows.into_iter().map(|(thread, _)| thread).collect()
}

fn js_whitespace(character: char) -> bool {
    character == '\u{feff}' || (character.is_whitespace() && character != '\u{0085}')
}

fn fold_query(raw: &str) -> String {
    let bare = raw.trim_matches(|c| {
        js_whitespace(c) || matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’')
    });
    let canonical: String = ComposingNormalizer::new_nfc()
        .normalize_iter(bare.chars())
        .collect();
    let folded: String = canonical.chars().map(fold_character).collect();
    folded
        .split(js_whitespace)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn fold_character(character: char) -> char {
    if character.is_ascii() {
        return character.to_ascii_lowercase();
    }
    if matches!(character, '‘' | '’') {
        return '\'';
    }
    let marks = CanonicalCombiningClassMap::new();
    let mut mapped = DecomposingNormalizer::new_nfkd()
        .normalize_iter(std::iter::once(character))
        .filter(|&c| marks.get_u8(c) == 0)
        .flat_map(char::to_lowercase);
    if let (Some(single), None) = (mapped.next(), mapped.next()) {
        if single.len_utf16() == character.len_utf16() {
            return if single == 'ς' { 'σ' } else { single };
        }
    }
    // The reference leaves multi-unit expansions (for example ﬁ) intact.
    character.to_lowercase().next().unwrap_or(character)
}

fn filler(token: &[char]) -> bool {
    matches!(
        token,
        ['a']
            | ['a', 'n']
            | ['a', 'n', 'd']
            | ['f', 'o', 'r']
            | ['i', 'n']
            | ['o', 'f']
            | ['o', 'n']
            | ['t', 'h', 'e']
            | ['t', 'o']
            | ['w', 'i', 't', 'h']
    )
}

fn cjk(character: char) -> bool {
    matches!(character, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn starts_word(chars: &[char], index: usize) -> bool {
    let current = chars[index];
    if index == 0 {
        return current.is_alphanumeric();
    }
    let previous = chars[index - 1];
    let after_apostrophe =
        index >= 2 && matches!(previous, '\'' | '’') && chars[index - 2].is_alphanumeric();
    current.is_alphanumeric()
        && !after_apostrophe
        && (!previous.is_alphanumeric()
            || (previous.is_lowercase() && current.is_uppercase())
            || (previous.is_uppercase()
                && current.is_uppercase()
                && chars.get(index + 1).is_some_and(|next| next.is_lowercase()))
            || previous.is_numeric() != current.is_numeric()
            || cjk(current))
}

struct Score {
    tier: u8,
    whole_words: bool,
    quality: f32,
}

fn title_score(
    title: &str,
    query: &str,
    tokens: &[Vec<char>],
    optional_fillers: bool,
) -> Option<Score> {
    let raw: Vec<_> = title.chars().collect();
    let text: Vec<_> = raw.iter().copied().map(fold_character).collect();
    let boundaries: Vec<_> = (0..raw.len())
        .map(|index| starts_word(&raw, index))
        .collect();
    let mut total = 0.0_f32;
    let mut counted = 0_u32;
    let mut whole_words = true;
    for token in tokens {
        let best = best_title_token(&text, &boundaries, token);
        if best == 0.0 {
            if optional_fillers && filler(token) {
                continue;
            }
            return None;
        }
        if filler(token) {
            whole_words &= optional_fillers;
        } else {
            whole_words &= token.len() >= 3 && best == 1.0;
        }
        total += best;
        counted += 1;
    }
    if counted == 0 {
        return None;
    }
    let folded: String = text.iter().collect();
    let exact = folded == query;
    let mut quality = total / counted as f32;
    if exact {
        quality += 0.5;
    } else if tokens.len() > 1 && folded.contains(query) {
        quality += 0.25;
    }
    Some(Score {
        tier: if exact {
            3
        } else if quality >= 0.7 {
            2
        } else if quality >= 0.4 {
            1
        } else {
            0
        },
        whole_words,
        quality,
    })
}

fn best_title_token(text: &[char], boundaries: &[bool], token: &[char]) -> f32 {
    if token.len() > text.len() {
        return 0.0;
    }
    let mut best = 0.0_f32;
    for (start, window) in text.windows(token.len()).enumerate() {
        if window != token {
            continue;
        }
        if boundaries[start] || !token[0].is_alphanumeric() {
            let end = start + token.len();
            if end == text.len() || boundaries[end] || !text[end].is_alphanumeric() {
                return 1.0;
            }
            let rest = text[end..]
                .iter()
                .take_while(|c| c.is_alphanumeric())
                .count();
            best = best.max(0.7 + 0.2 * token.len() as f32 / (token.len() + rest) as f32);
        } else if token.len() >= 4 {
            best = best.max(0.3);
        }
    }
    best
}

/// Build inside the same retained sidebar owner as the results; result IDs
/// must be given in their current visible order. A pointer selection should
/// call Search::reset before dispatch, just as keyboard selection does here.
pub fn field(search: Search, result_ids: Vec<Uuid>, on_choose: impl Fn(Uuid) + 'static) -> Widget {
    let active = search.active();
    let on_change = search.clone();
    let with_keys = search.clone();
    let editor = Widget::from(
        EditableText::new(search.controller.clone())
            .size(Size::new(0.0, HEIGHT))
            .content_padding(EdgeInsets::from_ltrb(
                32.0,
                0.0,
                if active { 29.0 } else { 8.0 },
                0.0,
            ))
            .placeholder("Search")
            .placeholder_color(MUTED)
            .cursor_color(INK)
            .style(
                typography::interface()
                    .font_size(14.0)
                    .line_height_absolute(20.0)
                    .font_weight(FontWeight::W500)
                    .color(INK),
            )
            .focused_border(Border::new(1.0, RING), 6.0)
            // Enter is an explicit list action, not a form completion.
            .on_editing_complete(|| {})
            .on_changed(move |_| {
                on_change.highlighted.set(None);
                on_change.invalidate();
            }),
    )
    // Overlay only the name, retaining native text selection and edit actions.
    .semantics(ExplicitSemantics::new(SemanticRole::GenericContainer).label(FIELD_LABEL));
    let editor = KeyboardListener::new(editor)
        .include_semantics(false)
        .on_key(move |event| {
            if !event.state.is_down() || with_keys.controller.preedit().is_some() {
                return false;
            }
            if event.key == KeyboardKey::Named(NamedKey::Escape) && with_keys.active() {
                with_keys.reset();
                return true;
            }
            if result_ids.is_empty() || !with_keys.active() {
                return false;
            }
            match event.key {
                KeyboardKey::Named(NamedKey::ArrowUp) => {
                    with_keys.move_highlight(&result_ids, false)
                }
                KeyboardKey::Named(NamedKey::ArrowDown) => {
                    with_keys.move_highlight(&result_ids, true)
                }
                KeyboardKey::Named(NamedKey::Enter) => {
                    if let Some(id) = with_keys.highlighted(&result_ids) {
                        with_keys.reset();
                        on_choose(id);
                    }
                }
                _ => return false,
            }
            true
        });
    let mut layers = vec![
        Positioned::new(icon("search", 16.0, ICON))
            .left(8.0)
            .top(8.0),
        Positioned::fill(editor),
    ];
    if active {
        let clear = search.clone();
        let button = Button::with_child("Clear thread search", icon("close", 12.0, MUTED))
            .style(
                ButtonStyle::new()
                    .variant(ButtonVariant::Ghost)
                    .fixed_size(Size::new(20.0, 20.0))
                    .padding(EdgeInsets::ZERO)
                    .border_radius(4.0)
                    .background(Color::TRANSPARENT),
            )
            .on_click(move || clear.reset());
        layers.push(Positioned::new(button).right(6.0).top(6.0));
    }
    let background = if search.hovered.get() || search.focused.get() {
        BACKGROUND_HOVER
    } else {
        Color::TRANSPARENT
    };
    let focus = search.clone();
    let strip = GestureDetector::new(
        Container::new()
            .height(HEIGHT)
            .decoration(
                BoxDecoration::new()
                    .color(background)
                    .border_radius(BorderRadius::circular(6.0)),
            )
            .child(Stack::new(layers)),
    )
    .callbacks(GestureCallbacks {
        on_focus_within: Some(Rc::new(move |focused| {
            if focus.focused.replace(focused) != focused {
                focus.invalidate();
            }
        })),
        ..GestureCallbacks::default()
    });
    let hover = search.clone();
    MouseRegion::new(strip)
        .opaque(false)
        .on_enter(move |_| {
            if !hover.hovered.replace(true) {
                hover.invalidate();
            }
        })
        .on_exit(move |_| {
            if search.hovered.replace(false) {
                search.invalidate();
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: u128, title: &str) -> ThreadSummary {
        ThreadSummary {
            id: Uuid::from_u128(id),
            title: title.into(),
        }
    }

    fn titles<'a>(records: &'a [ThreadSummary], query: &str) -> Vec<&'a str> {
        matching_threads(records, query)
            .into_iter()
            .map(|thread| thread.title.as_str())
            .collect()
    }

    #[test]
    fn titles_fold_rank_and_keep_store_order_for_equal_matches() {
        let histories = [
            thread(1, "Search for notes"),
            thread(2, "Notes Search"),
            thread(3, "Search"),
            thread(4, "Fix Café parser"),
            thread(5, "GitHub migration"),
        ];
        assert_eq!(
            titles(&histories, " SEARCH "),
            ["Search", "Search for notes", "Notes Search"]
        );
        assert_eq!(titles(&histories, " ‘FIX the caFe’ "), ["Fix Café parser"]);
        assert_eq!(
            titles(&histories, "arch"),
            ["Search for notes", "Notes Search", "Search"]
        );
        assert!(titles(&histories, "rch").is_empty());
        assert_eq!(titles(&histories, "hub migr"), ["GitHub migration"]);
        assert!(titles(&histories, "search absent").is_empty());
        assert!(titles(&histories, "  ‘ ’ ").is_empty());
    }

    #[test]
    fn wrap_covers_ends_empty_and_stale_selection() {
        let ids = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
        assert_eq!(wrapped_highlight(&ids, Some(ids[2]), true), Some(ids[0]));
        assert_eq!(wrapped_highlight(&ids, Some(ids[0]), false), Some(ids[2]));
        assert_eq!(
            wrapped_highlight(&ids, Some(Uuid::nil()), true),
            Some(ids[1])
        );
        assert_eq!(wrapped_highlight(&[], None, false), None);
    }
}
