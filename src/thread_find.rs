//! Local transcript Find. The sidebar seed follows T3 v0.1.11's
//! `threadFindStore.ts::findForMessageMatch`; literal transcript matching
//! follows `packages/shared/src/threadSearch.ts`.

use icu_normalizer::{
    ComposingNormalizer, DecomposingNormalizer, properties::CanonicalCombiningClassMap,
};
use std::{borrow::Cow, ops::Range};

/// Search for literal, non-overlapping occurrences. Each result is a sorted
/// UTF-8 byte range in the original text, safe for slicing; neither the text
/// nor query is normalized, accent-folded, whitespace-collapsed, or trimmed.
pub fn find_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if text.is_empty() || query.is_empty() {
        return Vec::new();
    }
    let haystack = lower(text);
    let needle = lower(query);
    let matches = haystack.match_indices(needle.as_ref());
    if text.is_ascii() {
        return matches
            .map(|(start, hit)| start..start + hit.len())
            .collect();
    }

    // The case/apostrophe fold emits one scalar for each original scalar,
    // not necessarily the same number of bytes. Walk the two boundary
    // sequences together, once, without storing a per-character offset map.
    fn boundaries(s: &str) -> impl Iterator<Item = usize> {
        s.char_indices().map(|(at, _)| at).chain([s.len()])
    }
    let mut offsets = boundaries(&haystack).zip(boundaries(text));
    let mut boundary = (0, 0);
    let mut original_at = |at| {
        while boundary.0 < at {
            boundary = offsets.next().expect("fold preserves scalar boundaries");
        }
        debug_assert_eq!(boundary.0, at);
        boundary.1
    };
    matches
        .map(|(start, hit)| original_at(start)..original_at(start + hit.len()))
        .collect()
}

fn lower(s: &str) -> Cow<'_, str> {
    if s.is_ascii() && !s.bytes().any(|c| c.is_ascii_uppercase()) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.chars().map(find_case).collect())
    }
}

fn straight(c: char) -> char {
    match c {
        '‘' | '’' => '\'',
        _ => c,
    }
}

fn find_case(c: char) -> char {
    if c == 'İ' {
        return 'i';
    }
    let mut lowercase = c.to_lowercase();
    match (lowercase.next(), lowercase.next()) {
        (Some('ς'), None) => 'σ',
        (Some(unit), None) if unit.len_utf16() == c.len_utf16() => straight(unit),
        _ => c,
    }
}

/// Derive an in-thread query from a sidebar message hit. Prefer the whole
/// multiword phrase where the snippet has it (including its original
/// whitespace, case and punctuation). Otherwise pick the longest eligible
/// token, with first-token ties, and recover its displayed spelling, favoring
/// a token ending at a word boundary. Do not use this fold for transcript
/// Find itself, which preserves accents.
pub fn seed_query(sidebar_query: &str, snippet: &str) -> String {
    let tokens = message_tokens(sidebar_query);
    let words: Vec<_> = tokens
        .split(js_space)
        .filter(|word| !word.is_empty())
        .collect();
    if words.len() > 1
        && let Some(phrase) = displayed_phrase(snippet, &words)
    {
        return phrase.to_owned();
    }

    let mut longest = "";
    for word in words.iter().copied().filter(|word| long_token(word)) {
        if word.encode_utf16().count() > longest.encode_utf16().count() {
            longest = word;
        }
    }
    let chosen = if longest.is_empty() {
        sidebar_query
    } else {
        longest
    };
    displayed_token(snippet, chosen)
        .unwrap_or(chosen)
        .to_owned()
}

fn js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’')
}

fn strip_wrapping_quotes(s: &str) -> &str {
    s.trim_start_matches(js_space)
        .trim_start_matches(is_quote)
        .trim_end_matches(js_space)
        .trim_end_matches(is_quote)
}

fn message_tokens(query: &str) -> String {
    ComposingNormalizer::new_nfc()
        .normalize_iter(strip_wrapping_quotes(query).chars().map(straight))
        .collect::<String>()
        .to_lowercase()
}

fn cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn long_token(token: &str) -> bool {
    match token.chars().count() {
        0 | 1 => false,
        2 => token
            .chars()
            .any(|c| cjk(c) || matches!(c, '\u{ac00}'..='\u{d7af}')),
        _ => true,
    }
}

fn phrase_case(c: char) -> char {
    // The reference phrase is an /iu regexp, without literal Find's special
    // dotted-I fold. Single-scalar case equivalence maps e.g. ſ to s.
    if matches!(c, 'İ' | 'ı') {
        return c;
    }
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(one), None) => find_case(one),
        _ => find_case(c),
    }
}

fn displayed_phrase<'a>(snippet: &'a str, words: &[&str]) -> Option<&'a str> {
    let mut previous = None;
    for (start, ch) in snippet.char_indices() {
        let boundary = previous.is_none_or(|p: char| !p.is_alphanumeric());
        previous = Some(ch);
        if !boundary {
            continue;
        }

        let mut source = snippet[start..].char_indices().peekable();
        let mut matched = true;
        for (index, word) in words.iter().enumerate() {
            if index > 0 {
                if source.next_if(|(_, c)| js_space(*c)).is_none() {
                    matched = false;
                    break;
                }
                while source.next_if(|(_, c)| js_space(*c)).is_some() {}
            }
            if !word.chars().all(|wanted| {
                source
                    .next()
                    .is_some_and(|(_, c)| phrase_case(c) == phrase_case(wanted))
            }) {
                matched = false;
                break;
            }
        }
        if matched && source.peek().is_none_or(|(_, c)| !c.is_alphanumeric()) {
            let end = source
                .peek()
                .map_or(snippet.len(), |(offset, _)| start + offset);
            return snippet.get(start..end);
        }
    }
    None
}

fn accent_fold(c: char) -> char {
    if c.is_ascii() || c.len_utf16() != 1 {
        return find_case(c);
    }
    let marks = CanonicalCombiningClassMap::new();
    let mut base = DecomposingNormalizer::new_nfkd()
        .normalize_iter(std::iter::once(c))
        .filter(|&part| marks.get_u8(part) == 0)
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ς' { 'σ' } else { straight(c) });
    match (base.next(), base.next()) {
        (Some(one), None) if one.len_utf16() == 1 => one,
        _ => find_case(c),
    }
}

fn word_start(chars: &[char], at: usize) -> bool {
    let c = chars[at];
    if at == 0 {
        return true;
    }
    let before = chars[at - 1];
    if at >= 2 && matches!(before, '\'' | '’') && chars[at - 2].is_alphanumeric() {
        return false;
    }
    !before.is_alphanumeric()
        || (before.is_lowercase() && c.is_uppercase())
        || (before.is_uppercase()
            && c.is_uppercase()
            && chars.get(at + 1).is_some_and(|next| next.is_lowercase()))
        || before.is_numeric() != c.is_numeric()
        || cjk(c)
}

fn displayed_token<'a>(snippet: &'a str, query: &str) -> Option<&'a str> {
    let tokens = message_tokens(query);
    let token = tokens.split(js_space).find(|word| long_token(word))?;
    let needs_start = token.chars().count() < 4;
    let folded = ComposingNormalizer::new_nfc()
        .normalize_iter(strip_wrapping_quotes(token).chars())
        .map(accent_fold)
        .collect::<Vec<_>>();
    let first = *folded.first()?;
    let own_boundary = !first.is_alphanumeric();
    let source: Vec<_> = snippet.chars().collect();
    let display: Vec<_> = source.iter().copied().map(accent_fold).collect();
    let mut offsets: Vec<_> = snippet.char_indices().map(|(offset, _)| offset).collect();
    offsets.push(snippet.len());

    let mut accepted = Vec::<Range<usize>>::new();
    let mut at = 0;
    while at + folded.len() <= display.len() {
        let end = at + folded.len();
        if display[at..end] == folded && (!needs_start || own_boundary || word_start(&source, at)) {
            if let Some(last) = accepted.last_mut().filter(|last| last.end == at) {
                last.end = end;
            } else {
                accepted.push(at..end);
            }
            at = end;
        } else {
            at += 1;
        }
    }
    let shown = accepted
        .iter()
        .find(|range| source.get(range.end).is_none_or(|c| !c.is_alphanumeric()))
        .or_else(|| accepted.first())?;
    snippet.get(offsets[shown.start]..offsets[shown.end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ported_reference_find_fixtures_use_original_utf8_ranges() {
        assert_eq!(
            find_ranges("Deploy the Deployment", "deploy"),
            [0..6, 11..17]
        );
        assert_eq!(find_ranges("aaaa", "aa"), [0..2, 2..4]);
        assert_eq!(find_ranges("İİİ needle", "needle"), [7..13]);
        assert_eq!(find_ranges("ΟΣ ος οσ", "Σ"), [2..4, 7..9, 12..14]);
        assert_eq!(find_ranges("ΟΣ ος οσ", "ς"), [2..4, 7..9, 12..14]);
        assert_eq!(find_ranges("İ ΟΣ", "Σ"), [5..7]);
        assert_eq!(find_ranges("don't don’t", "don’t"), [0..5, 6..13]);
        assert_eq!(find_ranges("İ don‘t", "don't"), [3..10]);
        assert_eq!(
            find_ranges("📍 İstanbul ıstanbul i\u{307}stanbul", "istanbul"),
            [5..14],
        );
        assert_eq!(find_ranges("Istanbul istanbul", "İSTANBUL"), [0..8, 9..17]);
    }

    #[test]
    fn source_slices_remain_valid_across_multibyte_folds() {
        let source = "🧪 ‘Σ’ ς‘σ İσ É e\u{301} 🙂🙂🙂";
        for (query, expected) in [
            ("'σ", vec!["‘Σ", "‘σ"]),
            ("İΣ", vec!["İσ"]),
            ("é", vec!["É"]),
            ("e", vec!["e"]),
            ("🙂🙂", vec!["🙂🙂"]),
            ("", vec![]),
        ] {
            let ranges = find_ranges(source, query);
            assert!(ranges.windows(2).all(|pair| pair[0].end <= pair[1].start));
            assert_eq!(
                ranges
                    .into_iter()
                    .map(|r| source.get(r).unwrap())
                    .collect::<Vec<_>>(),
                expected,
            );
        }
        assert_eq!(find_ranges("Café cafe", " cafe"), [5..10]); // No trim.
    }

    #[test]
    fn sidebar_seed_keeps_reference_phrase_and_recovers_accented_word() {
        assert_eq!(seed_query("cafe", "The cafeteria, then the Café"), "Café");
        assert_eq!(seed_query("fix sidebar", "fix the sidebar lag"), "sidebar");
        assert_eq!(
            seed_query("vp run dev", "then Vp  run dev again"),
            "Vp  run dev"
        );
        assert_eq!(
            seed_query("“don't stop”", "📍 Don’t\tSTOP yet"),
            "Don’t\tSTOP"
        );
        assert_eq!(seed_query("cafe open", "Café open"), "Café"); // Accent breaks exact phrase.
        assert_eq!(seed_query("cat cats", "scat cats!"), "cats");
        assert_eq!(seed_query("cafe déjà", "déjà printed Café"), "Café"); // Longest tie favors first.
        assert_eq!(seed_query("中文", "处理中文项目"), "中文");
        assert_eq!(seed_query("absent", "…visible excerpt…"), "absent");
        assert_eq!(seed_query("it", "it here"), "it");
    }
}
