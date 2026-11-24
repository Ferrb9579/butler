//! Inert projection of ordinary multiline saved assistant Markdown.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

const MAX_BYTES: usize = 16 * 1024;
const MAX_EVENTS: usize = 512;

/// Accept only ordinary paragraphs with literal text and soft line breaks.
/// Everything else uses the existing raw text and Find path, including
/// intentional single-line replies and unsupported mixed Markdown.
pub fn project(source: &str) -> Option<Vec<String>> {
    if source.len() > MAX_BYTES
        || !source.contains('\n')
        // T3 also recognizes local paths, literal GFM links, skill tokens,
        // directives and Insight blocks that plain CommonMark can miss.
        || source.contains(['/', '\\', '@', '$'])
        || source.contains("★ Insight")
        || [":codex-file-citation", ":codex-followup", "::artifact-template"]
            .iter()
            .any(|name| source.contains(name))
        || source.as_bytes().windows(4).any(|s| s.eq_ignore_ascii_case(b"www."))
        || source.chars().any(|c| (c.is_control() && c != '\n') || is_rtl_candidate(c))
    {
        return None;
    }
    let options = Options::ENABLE_GFM
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut paragraphs = Vec::new();
    let mut current: Option<String> = None;
    let mut end = 0;
    for (index, (event, range)) in Parser::new_ext(source, options)
        .into_offset_iter()
        .enumerate()
    {
        if index >= MAX_EVENTS {
            return None;
        }
        match event {
            Event::Start(Tag::Paragraph)
                if current.is_none() && only_line_feeds(source.get(end..range.start)?) =>
            {
                current = Some(String::new());
                end = range.end;
            }
            Event::End(TagEnd::Paragraph) => {
                let paragraph = current.take()?;
                // Don't half-project whitespace our rendering doesn't match.
                if paragraph.is_empty()
                    || paragraph.starts_with(' ')
                    || paragraph.ends_with(' ')
                    || paragraph.contains("  ")
                {
                    return None;
                }
                paragraphs.push(paragraph);
            }
            // Entity decoding, escapes, HTML and non-paragraph events are out
            // of scope. Raw text remains available to Find in those cases.
            Event::Text(text) if source.get(range.clone())? == text.as_ref() => {
                current.as_mut()?.push_str(text.as_ref());
            }
            Event::SoftBreak if source.get(range)? == "\n" => {
                current.as_mut()?.push(' ');
            }
            _ => return None,
        }
    }
    // Definitions and other skipped source cannot disappear between blocks.
    (!paragraphs.is_empty() && current.is_none() && only_line_feeds(source.get(end..)?))
        .then_some(paragraphs)
}

fn only_line_feeds(text: &str) -> bool {
    text.bytes().all(|b| b == b'\n')
}

// T3's textDirection.ts gives such blocks their own paragraph direction.
// Keep the existing literal path until that per-paragraph bidi policy is ported.
fn is_rtl_candidate(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{08ff}' | '\u{fb1d}'..='\u{fdff}'
        | '\u{fe70}'..='\u{feff}' | '\u{10800}'..='\u{10fff}' | '\u{1e800}'..='\u{1efff}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_only_plain_paragraphs_preserving_visible_utf8() {
        let doc = project("Check the café.\n\nThen\nship it.").unwrap();
        assert_eq!(doc, ["Check the café.", "Then ship it."]);
        assert_eq!(project("Lead\ntail.").unwrap(), ["Lead tail."]);
    }

    #[test]
    fn single_lines_and_unsupported_markdown_stay_entirely_literal() {
        assert!(project("Ordinary single-line prose.").is_none());
        for text in [
            "**strong** and *emphasis*",
            "This is **strong**.",
            "This is **strong**.\n\nLeave both blocks literal.",
            "Plain then  \na hard break",
            "Plain and &#x41;.",
            "Plain and `code`.",
            "```sh\necho literal\n```",
            "[literal destination](hidden.txt)",
            "![literal file](photo.png)",
            "www.example.test",
            "/tmp/note",
            "$skill",
            ":codex-followup[Do something]",
            "::artifact-template[Only literal]",
            "<span>literal</span>",
            "[definition]: hidden.txt",
            "Two  spaces",
            "- list",
            "~~strike~~",
            "---",
            "עברית",
        ] {
            let mixed = format!("Plain first paragraph.\n\n{text}");
            assert!(project(&mixed).is_none(), "{mixed}");
        }
        assert!(project(&format!("{}\nend", "x".repeat(MAX_BYTES))).is_none());
        assert!(project(&"x\n\n".repeat(MAX_EVENTS)).is_none());
    }
}
