//! Bounded, profile-local search of saved Butler exchanges. Nothing is read
//! until a caller explicitly searches qualified text in an exact workspace.
//! T3 v0.1.11 references: paletteSearch.ts and orchestration-v2/ThreadSearch.ts.

use butler::threads::{Conversation, ThreadStore, Workspace};
use icu_normalizer::{
    ComposingNormalizer, DecomposingNormalizer, properties::CanonicalCombiningClassMap,
};
use std::{borrow::Cow, collections::HashSet, io};
use uuid::Uuid;

const MAX_CANDIDATES: usize = 50;
const QUERY_UNITS: usize = 200;
const SNIPPET_UNITS: usize = 240;
const SNIPPET_LEAD: usize = 24;
const NEAR_SPAN: usize = 60;
const MAX_POSITIONS_PER_WORD: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageSource {
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageHit {
    pub thread_id: Uuid,
    pub source: MessageSource,
    pub snippet: String,
    /// 0 is the initial exchange; subsequent stored exchanges are 1 based.
    pub exchange_index: usize,
}

/// A short query may still match a title, but must not trigger body reads.
pub fn qualifies(query: &str) -> bool {
    Query::new(query).is_some()
}

/// Candidates must come from the caller's desired history, already sorted in
/// display order. Read at most the first 50 candidate records, one at a time;
/// existing ThreadStore limits each individual file to 2 MiB. Slipstreamed
/// failures/malformed records cannot disclose content from a different scope.
/// The caller can suppress title hits and render the rest in returned order.
pub fn search(
    store: &ThreadStore,
    workspace: &Workspace,
    candidates: &[Uuid],
    query: &str,
) -> io::Result<Vec<MessageHit>> {
    let Some(query) = Query::new(query) else {
        return Ok(Vec::new());
    };
    let mut seen = HashSet::with_capacity(candidates.len().min(MAX_CANDIDATES));
    let mut results = Vec::new();
    for &id in candidates.iter().take(MAX_CANDIDATES) {
        if id.is_nil() || !seen.insert(id) {
            continue;
        }
        match store.load(id) {
            Ok(Some(thread)) if &thread.workspace == workspace => {
                if let Some(hit) = best_message(&thread, &query) {
                    results.push(hit);
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    "Could not search saved thread messages.",
                ));
            }
        }
    }
    Ok(results)
}

struct Query {
    tokens: Vec<Token>,
    long_count: usize,
}

struct Token {
    folded: Vec<char>,
    // The reference counts Unicode code points for the length gate, not bytes.
    original_len: usize,
    long: bool,
}

impl Query {
    fn new(input: &str) -> Option<Self> {
        let input = prefix_utf16(input.trim_matches(js_whitespace), QUERY_UNITS)
            .trim_matches(js_whitespace);
        let input = input.trim_matches(|c| {
            js_whitespace(c) || matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’')
        });
        let normalized = normalize_nfc(input);
        let mut long_count = 0;
        let tokens = normalized
            .split(js_whitespace)
            .filter(|word| !word.is_empty())
            .map(|word| {
                let original_len = word.chars().count();
                let long = original_len >= 3 || (original_len == 2 && word.chars().any(cjk_word));
                long_count += usize::from(long);
                Token {
                    folded: word.chars().map(fold).collect(),
                    original_len,
                    long,
                }
            })
            .collect::<Vec<_>>();
        (long_count != 0).then_some(Self { tokens, long_count })
    }
}

fn best_message(thread: &Conversation, query: &Query) -> Option<MessageHit> {
    let pairs = std::iter::once((&thread.prompt, &thread.reply)).chain(
        thread
            .follow_ups
            .iter()
            .map(|exchange| (&exchange.prompt, &exchange.reply)),
    );
    let mut best: Option<(u8, MessageHit)> = None;
    for (exchange_index, (prompt, reply)) in pairs.enumerate() {
        for (source, original) in [
            (MessageSource::User, prompt),
            (MessageSource::Assistant, reply),
        ] {
            let text = normalize_nfc(original);
            let Some((quality, match_offset)) = match_message(&text, query) else {
                continue;
            };
            let improves = best.as_ref().is_none_or(|(prior_quality, prior)| {
                (quality, source == MessageSource::User, exchange_index)
                    > (
                        *prior_quality,
                        prior.source == MessageSource::User,
                        prior.exchange_index,
                    )
            });
            if improves {
                best = Some((
                    quality,
                    MessageHit {
                        thread_id: thread.id,
                        source,
                        snippet: build_snippet(&text, match_offset),
                        exchange_index,
                    },
                ));
            }
        }
    }
    best.map(|(_, hit)| hit)
}

fn cjk(character: char) -> bool {
    matches!(character, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
}

fn cjk_word(character: char) -> bool {
    cjk(character) || matches!(character, '\u{ac00}'..='\u{d7af}')
}

fn js_whitespace(character: char) -> bool {
    character == '\u{feff}' || (character.is_whitespace() && character != '\u{0085}')
}

fn prefix_utf16(text: &str, limit: usize) -> &str {
    let mut units = 0;
    for (start, c) in text.char_indices() {
        units += c.len_utf16();
        if units > limit {
            return &text[..start];
        }
    }
    text
}

fn normalize_nfc(text: &str) -> Cow<'_, str> {
    if text.is_ascii() {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(
            ComposingNormalizer::new_nfc()
                .normalize_iter(text.chars())
                .collect(),
        )
    }
}

fn fold(character: char) -> char {
    match character {
        '‘' | '’' => return '\'',
        'ς' => return 'σ',
        _ if character.is_ascii() => return character.to_ascii_lowercase(),
        _ => {}
    }
    let marks = CanonicalCombiningClassMap::new();
    let mut candidate = DecomposingNormalizer::new_nfkd()
        .normalize_iter(std::iter::once(character))
        .filter(|&c| marks.get_u8(c) == 0);
    // Fold when there is a single Latin/compatibility base, preserving
    // Japanese voicing marks and multi-letter ligatures.
    if let (Some(ascii), None) = (candidate.next(), candidate.next()) {
        if ascii.is_ascii_alphanumeric() {
            return ascii.to_ascii_lowercase();
        }
    }
    character.to_lowercase().next().unwrap_or(character)
}

fn word_start(chars: &[char], index: usize) -> bool {
    let current = chars[index];
    if !current.is_alphanumeric() || cjk(current) || index == 0 {
        return true;
    }
    let prior = chars[index - 1];
    if index >= 2 && matches!(prior, '\'' | '’') && chars[index - 2].is_alphanumeric() {
        return false;
    }
    !prior.is_alphanumeric()
        || (prior.is_lowercase() && current.is_uppercase())
        || (prior.is_uppercase()
            && current.is_uppercase()
            && chars.get(index + 1).is_some_and(|next| next.is_lowercase()))
        || prior.is_numeric() != current.is_numeric()
}

fn word_end(chars: &[char], index: usize) -> bool {
    index == chars.len() || !chars[index].is_alphanumeric() || word_start(chars, index)
}

type Positions = Vec<usize>;

struct WordHit {
    quality: u8,
    at: Positions,
}

fn best_token(
    original: &[char],
    folded: &[char],
    token: &Token,
    first_only: bool,
) -> Option<WordHit> {
    let mut best = WordHit {
        quality: 0,
        at: Vec::new(),
    };
    let mut run = (0, 0, false);
    for (start, window) in folded.windows(token.folded.len()).enumerate() {
        if window != token.folded {
            continue;
        }
        let end = start + window.len();
        if end > run.1 {
            let mut run_start = start;
            while run_start > 0 && original[run_start - 1].is_alphanumeric() {
                run_start -= 1;
            }
            let mut run_end = end;
            while run_end < original.len() && original[run_end].is_alphanumeric() {
                run_end += 1;
            }
            let contents = &original[run_start..run_end];
            run = (
                run_start,
                run_end,
                contents.len() >= 7
                    && contents.iter().all(char::is_ascii_hexdigit)
                    && contents.iter().any(char::is_ascii_digit),
            );
        }
        if run.2 && run.0 < start {
            continue;
        }
        let quality = if word_start(original, start) {
            if word_end(original, end) { 100 } else { 80 }
        } else if token.original_len >= 4 {
            30
        } else {
            continue; // A short suffix/mid-word hit is noise in message text.
        };
        if quality > best.quality {
            best.quality = quality;
            best.at.clear();
        }
        if quality == best.quality && best.at.len() < MAX_POSITIONS_PER_WORD {
            best.at.push(start);
        }
        if first_only && quality == 100 {
            break;
        }
    }
    (!best.at.is_empty()).then_some(best)
}

fn phrase_start(original: &[char], folded: &[char], query: &Query) -> Option<usize> {
    if query.tokens.len() <= 1 {
        return None;
    }
    let first = &query.tokens[0].folded;
    'start: for (start, span) in folded.windows(first.len()).enumerate() {
        if span != first || (start > 0 && original[start - 1].is_alphanumeric()) {
            continue;
        }
        let mut end = start + first.len();
        for token in &query.tokens[1..] {
            if !original.get(end).is_some_and(|c| js_whitespace(*c)) {
                continue 'start;
            }
            while original.get(end).is_some_and(|c| js_whitespace(*c)) {
                end += 1;
            }
            let next = end + token.folded.len();
            if folded.get(end..next) != Some(token.folded.as_slice()) {
                continue 'start;
            }
            end = next;
        }
        if !original.get(end).is_some_and(|c| c.is_alphanumeric()) {
            return Some(start);
        }
    }
    None
}

fn shortest_span(places: &[Positions]) -> (usize, usize) {
    let mut points = places
        .iter()
        .enumerate()
        .flat_map(|(term, at)| at.iter().map(move |&index| (index, term)))
        .collect::<Vec<_>>();
    points.sort_unstable();
    let Some(&(first, _)) = points.first() else {
        return (0, usize::MAX);
    };
    let mut best = (first, usize::MAX);
    let mut term_counts = vec![0; places.len()];
    let mut covered = 0;
    let mut left = 0;
    for (right, term) in points.iter().copied() {
        if term_counts[term] == 0 {
            covered += 1;
        }
        term_counts[term] += 1;
        while covered == places.len() {
            let (start, left_term) = points[left];
            if right - start < best.1 {
                best = (start, right - start);
            }
            term_counts[left_term] -= 1;
            if term_counts[left_term] == 0 {
                covered -= 1;
            }
            left += 1;
        }
    }
    best
}

/// Return quality on the source's 0–125 scale, and a UTF-8-safe byte offset.
fn match_message(text: &str, query: &Query) -> Option<(u8, usize)> {
    let original = text.chars().collect::<Vec<_>>();
    let folded = original.iter().copied().map(fold).collect::<Vec<_>>();
    let mut quality = 100;
    let mut positions = Vec::with_capacity(query.tokens.len());
    for token in &query.tokens {
        let hit = best_token(&original, &folded, token, query.tokens.len() == 1)?;
        if token.long {
            quality = quality.min(hit.quality);
        }
        positions.push(hit.at);
    }
    let character_offset = if let Some(start) = phrase_start(&original, &folded, query) {
        quality += 25;
        start
    } else {
        let (start, span) = shortest_span(&positions);
        if query.long_count > 1 && span <= NEAR_SPAN {
            quality += 10;
        }
        start
    };
    let byte_offset = text.char_indices().nth(character_offset)?.0;
    Some((quality, byte_offset))
}

fn build_snippet(text: &str, match_offset: usize) -> String {
    let mut start = match_offset;
    let mut units = 0;
    for (byte_index, c) in text[..match_offset].char_indices().rev() {
        units += c.len_utf16();
        if units > SNIPPET_LEAD {
            break;
        }
        start = byte_index;
    }
    if start > 0 {
        if let Some((byte_index, c)) = text[start..match_offset]
            .char_indices()
            .find(|&(_, c)| js_whitespace(c))
        {
            start += byte_index + c.len_utf8();
        }
    }
    // Read a bounded stretch, so excerpts never copy a large trailing body.
    let raw = prefix_utf16(&text[start..], SNIPPET_UNITS * 2);
    let collapsed = raw
        .split(js_whitespace)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let shown = prefix_utf16(&collapsed, SNIPPET_UNITS - 2);
    let suffix = shown.len() < collapsed.len() || start + raw.len() < text.len();
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        shown,
        if suffix { "…" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use butler::threads::Exchange;
    use std::{fs, path::PathBuf};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("butler-msg-search-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn workspace(&self, name: &str) -> Workspace {
            let dir = self.0.join(name);
            fs::create_dir(&dir).unwrap();
            Workspace::Local(dir.canonicalize().unwrap())
        }
        fn store(&self) -> ThreadStore {
            ThreadStore::new(self.0.join("profile"))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn turn(workspace: &Workspace, id: u128, prompt: &str, reply: &str) -> Conversation {
        Conversation {
            id: Uuid::from_u128(id),
            workspace: workspace.clone(),
            prompt: prompt.into(),
            reply: reply.into(),
            follow_ups: vec![],
        }
    }

    #[test]
    fn qualification_and_boundary_rules() {
        assert!(!qualifies("  'ab' "));
        assert!(!qualifies("to or"));
        assert!(qualifies("abc"));
        assert!(qualifies("中文"));
        assert!(qualifies("한국"));
        let cutoff = format!("{}xyz", "😀".repeat(100));
        assert_eq!(prefix_utf16(&cutoff, QUERY_UNITS).chars().count(), 100);
        assert_eq!(prefix_utf16("abcd😀", 5), "abcd");
        assert!(match_message("drafts", &Query::new("fts").unwrap()).is_none());
        assert!(match_message("particular", &Query::new("ticu").unwrap()).is_some());
        assert!(match_message("7cafe52bb8", &Query::new("cafe").unwrap()).is_none());
        assert!(match_message("Café open", &Query::new("cafe open").unwrap()).is_some());
        assert!(match_message("Open         docs", &Query::new("open docs").unwrap()).is_some());
        assert!(match_message("Open", &Query::new("open docs").unwrap()).is_none());
        assert!(match_message("abcdef", &Query::new("abcd f").unwrap()).is_none());
    }

    #[test]
    fn one_best_persisted_hit_per_thread_in_candidate_order_and_exact_scope() {
        let temp = Temp::new();
        let a = temp.workspace("a");
        let b = temp.workspace("b");
        let store = temp.store();
        let first = turn(&a, 1, "A first title", "What Azure can do");
        let hidden = turn(&b, 2, "azure private project body", "Not this scope");
        let second = turn(&a, 3, "Second", "Use Azure for storage");
        let split = turn(&a, 4, "Mention Azure", "Separately storage");
        for thread in [&first, &hidden, &second, &split] {
            store.save(thread).unwrap();
        }
        let after = store
            .append(
                &first,
                Exchange {
                    prompt: "We use Azure now".into(),
                    reply: "Azure replied".into(),
                },
            )
            .unwrap();
        store
            .append(
                &after,
                Exchange {
                    prompt: "Azure again, newest".into(),
                    reply: "Azure answered".into(),
                },
            )
            .unwrap();

        let ids = [
            hidden.id,
            second.id,
            first.id,
            first.id,
            split.id,
            Uuid::from_u128(55),
        ];
        let hits = search(&store, &a, &ids, "azure").unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.thread_id).collect::<Vec<_>>(),
            [second.id, first.id, split.id]
        );
        assert_eq!(hits[0].source, MessageSource::Assistant);
        assert_eq!(hits[1].source, MessageSource::User);
        assert_eq!(hits[1].exchange_index, 2);
        assert!(hits[1].snippet.contains("newest"));
        assert_eq!(
            search(&store, &a, &ids, "azure storage").unwrap()[0].thread_id,
            second.id
        );
        assert_eq!(
            search(&store, &a, &[split.id], "azure storage")
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn match_quality_beats_user_role_and_snippets_are_real_bounded_unicode() {
        let scope = Workspace::Scratch;
        let thread = turn(
            &scope,
            1,
            "Open a model",
            &format!(
                "{} mode after\n\n{}",
                "old words and 🌍 ".repeat(4),
                "🦋 long words ".repeat(100)
            ),
        );
        let hit = best_message(&thread, &Query::new("mode").unwrap()).unwrap();
        assert_eq!(hit.source, MessageSource::Assistant);
        assert!(hit.snippet.starts_with('…'));
        assert!(hit.snippet.ends_with('…'));
        assert!(hit.snippet.contains("mode after"));
        assert!(!hit.snippet.contains('\n'));
        assert!(hit.snippet.encode_utf16().count() <= SNIPPET_UNITS);
    }

    #[test]
    fn only_the_first_fifty_candidates_are_inspected_and_short_queries_return_empty() {
        let temp = Temp::new();
        let scope = temp.workspace("data");
        let store = temp.store();
        let record = turn(&scope, 51, "test", "cranberry");
        store.save(&record).unwrap();
        let candidates = (1..=51).map(Uuid::from_u128).collect::<Vec<_>>();
        assert!(
            search(&store, &scope, &candidates, "cranberry")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            search(&store, &scope, &[record.id], "cranberry")
                .unwrap()
                .len(),
            1
        );
        assert!(
            search(&store, &scope, &[record.id], "cr")
                .unwrap()
                .is_empty()
        );
    }
}

// The UI owns the request identity. A newly typed query replaces the debounce
// immediately; any already-running file read finishes before another starts.
#[derive(Clone, Debug, Default)]
pub enum Results {
    #[default]
    Idle,
    Pending,
    Failed,
    Ready(Vec<MessageHit>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    workspace: Workspace,
    candidates: Vec<Uuid>,
    query: String,
    history_revision: u64,
}

#[derive(Default)]
struct LookupState {
    request: Option<Request>,
    generation: u64,
    timer: Option<incular::runtime::TaskHandle>,
    ready: bool,
    running: bool,
    results: Results,
}

#[derive(Clone)]
pub struct Lookup {
    state: std::rc::Rc<std::cell::RefCell<LookupState>>,
    changed: incular::prelude::Signal<u64>,
}

impl Default for Lookup {
    fn default() -> Self {
        Self {
            state: Default::default(),
            changed: incular::prelude::Signal::new(0),
        }
    }
}

impl Lookup {
    pub fn poll(
        &self,
        cx: &incular::runtime::BuildContext,
        store: Option<&ThreadStore>,
        workspace: &Workspace,
        history: &[butler::threads::ThreadSummary],
        query: String,
        history_revision: u64,
    ) -> Results {
        self.changed.get();
        let request =
            (store.is_some() && !history.is_empty() && qualifies(&query)).then(|| Request {
                workspace: workspace.clone(),
                candidates: history
                    .iter()
                    .take(MAX_CANDIDATES)
                    .map(|row| row.id)
                    .collect(),
                query,
                history_revision,
            });
        let mut state = self.state.borrow_mut();
        if state.request != request {
            if let Some(timer) = state.timer.take() {
                timer.cancel();
            }
            state.generation = state.generation.wrapping_add(1);
            state.ready = false;
            state.request = request;
            state.results = Results::Idle;
            if state.request.is_some() {
                state.results = Results::Pending;
                let lookup = self.clone();
                let generation = state.generation;
                state.timer = Some(cx.spawn_into(
                    incular::runtime::sleep(std::time::Duration::from_millis(120)),
                    move |result, _| {
                        let mut state = lookup.state.borrow_mut();
                        if state.generation == generation {
                            state.timer = None;
                            state.ready = result.is_ok();
                            if result.is_err() {
                                state.results = Results::Failed;
                            }
                            lookup.notify();
                        }
                    },
                ));
            }
        }
        if state.ready
            && !state.running
            && let (Some(request), Some(store)) = (state.request.clone(), store.cloned())
        {
            state.ready = false;
            state.running = true;
            let lookup = self.clone();
            let generation = state.generation;
            cx.spawn_blocking(
                move || {
                    search(
                        &store,
                        &request.workspace,
                        &request.candidates,
                        &request.query,
                    )
                },
                move |result, _| {
                    let mut state = lookup.state.borrow_mut();
                    state.running = false;
                    if state.generation == generation {
                        state.results = match result {
                            Ok(Ok(hits)) => Results::Ready(hits),
                            _ => Results::Failed,
                        };
                    }
                    lookup.notify();
                },
            );
        }
        state.results.clone()
    }

    fn notify(&self) {
        self.changed.set(self.changed.get().wrapping_add(1));
    }
}
