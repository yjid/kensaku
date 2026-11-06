//! Lightweight Markdown text extraction, shared by indexing and snippets.
//!
//! This is a best-effort text reader, not a CommonMark renderer or YAML parser.
//! Frontmatter is retained verbatim (without its delimiters), so even unfamiliar
//! YAML values stay searchable. Code is retained literally; link destinations
//! and reference definitions are omitted. Reads and output are byte-bounded.

use std::collections::HashSet;
use std::path::Path;

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

pub fn extract_markdown(path: &Path, max_bytes: usize) -> Option<String> {
    let raw = super::text::read_text(path, max_bytes)?;
    Some(clean_text(&raw, max_bytes))
}

/// Retain bounded head/tail sampling while decoding each segment independently.
pub fn extract_sampled(path: &Path, max_bytes: usize) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    const OMISSION: &str = "\n\n... [content omitted] ...\n\n";
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size <= max_bytes as u64 || max_bytes <= OMISSION.len() + 4 {
        return extract_markdown(path, max_bytes);
    }
    let half = (max_bytes - OMISSION.len()) / 2;
    let mut head = vec![0; half];
    let n = file.read(&mut head).ok()?;
    head.truncate(n);
    let utf16 = matches!(head.get(..2), Some([0xff, 0xfe] | [0xfe, 0xff]));
    if !utf16 && head.iter().take(1024).any(|&b| b == 0) {
        return None;
    }
    let mut start = size.saturating_sub(half as u64);
    if utf16 && start % 2 != 0 {
        start += 1;
    }
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut tail = Vec::new();
    file.take(half as u64).read_to_end(&mut tail).ok()?;
    let (head_text, tail_text) = if utf16 {
        (
            super::text::decode_utf16_sample(&head, &head, false)?,
            super::text::decode_utf16_sample(&tail, &head, true)?,
        )
    } else {
        let end = match std::str::from_utf8(&head) {
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            _ => head.len(),
        };
        let skip = tail
            .iter()
            .take(3)
            .take_while(|&&b| b & 0xc0 == 0x80)
            .count();
        (
            String::from_utf8_lossy(&head[..end]).into_owned(),
            String::from_utf8_lossy(&tail[skip..]).into_owned(),
        )
    };
    Some(format!(
        "{}{}{}",
        clean_text(&head_text, half),
        OMISSION,
        clean_text(&tail_text, half)
    ))
}

pub(crate) fn clean_text(raw: &str, max_bytes: usize) -> String {
    let mut text = plain_text(raw);
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

enum Line<'a> {
    Text(&'a str),
    Literal(&'a str),
    Blank,
}

/// Converts a bounded text buffer to searchable text. Used after text decoding.
fn plain_text(raw: &str) -> String {
    let lines: Vec<&str> = raw.trim_start_matches('\u{feff}').lines().collect();
    let frontmatter_end = if lines.first().is_some_and(|line| line.trim_end() == "---") {
        lines
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(i, line)| matches!(line.trim_end(), "---" | "...").then_some(i))
    } else {
        None
    };
    let mut blocks = Vec::with_capacity(lines.len());
    let mut references = HashSet::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut container = (0usize, 0usize);
    let mut skip_title = false;
    for (i, line) in lines.into_iter().enumerate() {
        if skip_title && title_line(line) {
            skip_title = false;
            continue;
        }
        skip_title = false;
        if let Some(end) = frontmatter_end {
            if i == 0 || i == end {
                continue;
            }
            if i < end {
                blocks.push(Line::Literal(line));
                continue;
            }
        }
        // Strip only the container that opened this fence. Any additional
        // quote/list-looking characters are literal code, not new containers.
        if let Some((marker, length)) = fence {
            if let Some(code) = fence_content(line, container) {
                if fence_marker(code).is_some_and(|(m, n, rest)| {
                    m == marker && n >= length && rest.trim().is_empty()
                }) {
                    fence = None;
                    blocks.push(Line::Blank);
                } else {
                    blocks.push(Line::Literal(code));
                }
                continue;
            }
            // The enclosing container ended; parse this line as a new block.
            fence = None;
        }
        let (quotes, unquoted) = quote_prefix(line);
        let (list_indent, line) = list_prefix(unquoted);
        if let Some((marker, length, rest)) = fence_marker(line)
            && (marker != b'`' || !rest.contains('`'))
        {
            container = (quotes, list_indent);
            fence = Some((marker, length));
            blocks.push(Line::Blank);
        } else if line.starts_with("    ") || line.starts_with('\t') {
            blocks.push(Line::Literal(line));
        } else if let Some((label, needs_title)) = reference_definition(line) {
            references.insert(normalize_label(label));
            skip_title = needs_title;
            blocks.push(Line::Blank);
        } else {
            blocks.push(Line::Text(line));
        }
    }

    let mut merged = Vec::new();
    let mut paragraph = String::new();
    for block in blocks {
        if let Line::Text(line) = block
            && !line.trim().is_empty()
        {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(block_prefix(line));
        } else {
            if !paragraph.is_empty() {
                merged.push((false, std::mem::take(&mut paragraph)));
            }
            merged.push((
                true,
                match block {
                    Line::Literal(s) => s.to_string(),
                    _ => String::new(),
                },
            ));
        }
    }
    if !paragraph.is_empty() {
        merged.push((false, paragraph));
    }
    let mut out = String::with_capacity(raw.len());
    for (literal, line) in merged {
        if literal {
            out.push_str(&line);
        } else {
            inline_text(&line, &references, &mut out, 0);
        }
        out.push('\n');
    }
    out.trim_matches(['\n', '\r']).to_string()
}

/// Consume exactly the opening fence's quote depth and list indentation.
fn fence_content(mut line: &str, (quotes, indent): (usize, usize)) -> Option<&str> {
    for _ in 0..quotes {
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() > 3 {
            return None;
        }
        line = trimmed.strip_prefix('>')?;
        line = line.strip_prefix(' ').unwrap_or(line);
    }
    if line.trim().is_empty() {
        return Some("");
    }
    if line.bytes().take(indent).all(|b| b == b' ') && line.len() >= indent {
        Some(&line[indent..])
    } else {
        None
    }
}

fn quote_prefix(mut line: &str) -> (usize, &str) {
    let mut count = 0;
    loop {
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() > 3 {
            break;
        }
        let Some(rest) = trimmed.strip_prefix('>') else {
            break;
        };
        count += 1;
        line = rest.strip_prefix(' ').unwrap_or(rest);
    }
    (count, line)
}

fn list_prefix(line: &str) -> (usize, &str) {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return (0, line);
    }
    let marker = if trimmed.starts_with(['-', '+', '*']) {
        1
    } else {
        let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 9 || !trimmed[digits..].starts_with(['.', ')']) {
            return (0, line);
        }
        digits + 1
    };
    if !trimmed[marker..].starts_with(' ') {
        return (0, line);
    }
    let width = line.len() - trimmed.len() + marker + 1;
    (width, &line[width..])
}

fn code_end(text: &str, length: usize) -> Option<usize> {
    let mut offset = 0;
    while offset < text.len() {
        let start = offset + text[offset..].find('`')?;
        let n = text[start..].bytes().take_while(|&b| b == b'`').count();
        if n == length {
            return Some(start);
        }
        offset = start + n;
    }
    None
}

// Parse destination + optional title separately so malformed links retain text.
fn link_end(text: &str) -> Option<usize> {
    fn whitespace(text: &str, mut i: usize) -> usize {
        while let Some(ch) = text[i..].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            i += ch.len_utf8();
        }
        i
    }
    let mut i = whitespace(text, 1);
    if text.as_bytes().get(i) == Some(&b')') {
        return Some(i);
    }
    if text.as_bytes().get(i) == Some(&b'<') {
        i += 1;
        loop {
            let ch = text[i..].chars().next()?;
            i += ch.len_utf8();
            match ch {
                '>' => break,
                '<' => return None,
                '\\' => {
                    i += text[i..].chars().next()?.len_utf8();
                }
                _ => {}
            }
        }
    } else {
        let mut depth = 0usize;
        while let Some(ch) = text[i..].chars().next() {
            match ch {
                ')' if depth == 0 => return Some(i),
                '(' => depth += 1,
                ')' => depth -= 1,
                ch if ch.is_whitespace() && depth == 0 => break,
                ch if ch.is_whitespace() => return None,
                '\\' => {
                    i += 1;
                    i += text[i..].chars().next()?.len_utf8();
                    continue;
                }
                _ => {}
            }
            i += ch.len_utf8();
        }
        if depth != 0 {
            return None;
        }
    }
    let spaced = whitespace(text, i);
    if text.as_bytes().get(spaced) == Some(&b')') {
        return Some(spaced);
    }
    if spaced == i {
        return None;
    }
    i = spaced;
    let opener = text[i..].chars().next()?;
    let closer = match opener {
        '\'' | '"' => opener,
        '(' => ')',
        _ => return None,
    };
    i += opener.len_utf8();
    loop {
        let ch = text[i..].chars().next()?;
        i += ch.len_utf8();
        if ch == '\\' {
            i += text[i..].chars().next()?.len_utf8();
        } else if ch == closer {
            break;
        }
    }
    i = whitespace(text, i);
    (text.as_bytes().get(i) == Some(&b')')).then_some(i)
}

fn fence_marker(line: &str) -> Option<(u8, usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = *trimmed.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = trimmed.bytes().take_while(|&b| b == marker).count();
    (length >= 3).then_some((marker, length, &trimmed[length..]))
}

fn title_line(line: &str) -> bool {
    let text = line.trim();
    if line.len() - line.trim_start_matches(' ').len() > 3 || line.starts_with('\t') {
        return false;
    }
    [('"', '"'), ('\'', '\''), ('(', ')')]
        .iter()
        .any(|&(a, b)| text.len() >= 2 && text.starts_with(a) && text.ends_with(b))
}

fn reference_definition(line: &str) -> Option<(&str, bool)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let label = trimmed.strip_prefix('[')?;
    let end = label.find("]:")?;
    if label[..end].trim().is_empty() || label[end + 2..].trim().is_empty() {
        return None;
    }
    let destination = label[end + 2..].trim_start();
    let wrapped = format!("({destination})");
    if link_end(&wrapped) != Some(wrapped.len() - 1) {
        return None;
    }
    let remainder = if destination.starts_with('<') {
        destination.find('>').map(|i| &destination[i + 1..])
    } else {
        destination
            .find(char::is_whitespace)
            .map(|i| &destination[i..])
    };
    Some((
        &label[..end],
        remainder.is_none_or(|rest| rest.trim().is_empty()),
    ))
}

fn normalize_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn block_prefix(line: &str) -> &str {
    let mut text = line.trim_start();
    while let Some(rest) = text.strip_prefix('>') {
        text = rest.trim_start();
    }
    let hashes = text.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && text[hashes..].starts_with(char::is_whitespace) {
        text = text[hashes..].trim();
        // ATX closing hashes must be preceded by whitespace.
        let without_hashes = text.trim_end_matches('#');
        if without_hashes.ends_with(char::is_whitespace) {
            text = without_hashes.trim_end();
        }
    }
    for prefix in ["- ", "+ ", "* "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return rest;
        }
    }
    text
}

fn label_end(text: &str) -> Option<usize> {
    let mut offset = 0;
    let mut depth = 0usize;
    while offset < text.len() {
        let rest = &text[offset..];
        let ch = rest.chars().next()?;
        if ch == '\\' {
            offset += 1;
            if let Some(next) = text[offset..].chars().next() {
                offset += next.len_utf8();
            }
            continue;
        }
        if ch == '`' {
            let n = rest.bytes().take_while(|&b| b == b'`').count();
            if let Some(end) = code_end(&rest[n..], n) {
                offset += n + end + n;
                continue;
            }
            // Preserve ambiguous labels without repeatedly scanning the suffix.
            return None;
        }
        if ch == '[' {
            depth += 1;
        }
        if ch == ']' {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(offset);
            }
        }
        offset += ch.len_utf8();
    }
    None
}

/// Finds a balanced delimiter, respecting escapes. An unmatched opener is
/// retained as text; callers consume the remainder instead of scanning it again.
fn closing(text: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (i, b) in text.bytes().enumerate() {
        if escaped {
            escaped = false;
        } else if b == b'\\' {
            escaped = true;
        } else if b == open {
            depth += 1;
        } else if b == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn inline_text(text: &str, references: &HashSet<String>, out: &mut String, depth: usize) {
    // Bound recursion for nested labels in malformed or generated Markdown.
    if depth >= 16 {
        out.push_str(text);
        return;
    }
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(escaped) = rest.strip_prefix('\\')
            && let Some(ch) = escaped.chars().next()
            && ch.is_ascii_punctuation()
        {
            out.push(ch);
            rest = &escaped[ch.len_utf8()..];
            continue;
        }
        if rest.starts_with('`') {
            let n = rest.bytes().take_while(|&b| b == b'`').count();
            if let Some(end) = code_end(&rest[n..], n) {
                out.push_str(&rest[n..n + end]);
                rest = &rest[n + end + n..];
                continue;
            }
            out.push_str(rest);
            return;
        }
        let image = rest.starts_with("![");
        let start = usize::from(image);
        if rest[start..].starts_with('[') {
            let bracketed = &rest[start..];
            let Some(end) = label_end(bracketed) else {
                out.push_str(rest);
                return;
            };
            let label = &bracketed[1..end];
            let after = &bracketed[end + 1..];
            let consumed = if after.starts_with('(') {
                let Some(end) = link_end(after) else {
                    out.push_str(rest);
                    return;
                };
                Some(end + 1)
            } else if after.starts_with('[') {
                closing(after, b'[', b']').and_then(|end| {
                    let key = &after[1..end];
                    let key = if key.is_empty() { label } else { key };
                    references
                        .contains(&normalize_label(key))
                        .then_some(end + 1)
                })
            } else {
                references.contains(&normalize_label(label)).then_some(0)
            };
            if let Some(n) = consumed {
                inline_text(label, references, out, depth + 1);
                rest = &after[n..];
                continue;
            }
            // Keep malformed/unresolved links intact, without repeatedly
            // searching the same nested brackets.
            out.push_str(&rest[..start + end + 1]);
            rest = after;
            continue;
        }
        let ch = rest.chars().next().expect("nonempty text");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_destinations_and_titles_are_preserved() {
        assert_eq!(plain_text("[id]: two words"), "[id]: two words");
        assert_eq!(
            plain_text("[id]: /url \"existing\"\n\"keep prose\""),
            "\"keep prose\""
        );
        for source in ["[label](two words)", "[label](url \"title\" trailing)"] {
            assert_eq!(plain_text(source), source);
        }
        assert_eq!(
            plain_text("[id]: /url \"existing title\"\n    \"keep this code\""),
            "    \"keep this code\""
        );
    }

    #[test]
    fn bounded_utf16_and_sampling_keep_searchable_words() {
        let path =
            std::env::temp_dir().join(format!("kensaku-md-sampling-{}.md", std::process::id()));
        for be in [false, true] {
            let encode = |source: &str| {
                let mut bytes = if be {
                    vec![0xfe, 0xff]
                } else {
                    vec![0xff, 0xfe]
                };
                for u in source.encode_utf16() {
                    bytes.extend(if be { u.to_be_bytes() } else { u.to_le_bytes() });
                }
                bytes
            };
            std::fs::write(&path, encode("word😀")).unwrap();
            assert_eq!(extract_markdown(&path, 12).as_deref(), Some("word"));
            let source = format!(
                "HeadToken 😀\n{}\n[TailToken](hidden-url)",
                "padding 😀\n".repeat(1000)
            );
            std::fs::write(&path, encode(&source)).unwrap();
            for budget in [511, 512, 513] {
                let text = extract_sampled(&path, budget).unwrap();
                assert!(text.contains("HeadToken"));
                assert!(text.contains("TailToken"));
                assert!(!text.contains("hidden-url"));
                assert!(!text.contains('\0'));
                assert!(text.len() <= budget);
            }
        }
        std::fs::write(&path, [b"not\0text".as_slice(), &vec![b'x'; 2000]].concat()).unwrap();
        assert!(extract_sampled(&path, 512).is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn links_respect_literal_code_inside_labels() {
        assert_eq!(plain_text("[use `]` here](destination)"), "use ] here");
        assert_eq!(plain_text("[use `[` here](destination)"), "use [ here");
        assert_eq!(plain_text("[id]: url\n    \"code\""), "    \"code\"");
    }

    #[test]
    fn frontmatter_delimiters_must_start_at_column_zero() {
        for marker in ["---", "..."] {
            let source = format!(
                "---\ndescription: |\n  {marker}\n  [keep](metadata-value)\ntags: [metadata]\n---  \nBody"
            );
            let expected = format!(
                "description: |\n  {marker}\n  [keep](metadata-value)\ntags: [metadata]\nBody"
            );
            assert_eq!(plain_text(&source), expected);
        }
        assert_eq!(
            plain_text("    ---\n    [code](keep)"),
            "    ---\n    [code](keep)"
        );
    }

    #[test]
    fn fenced_code_keeps_quote_like_content_literal() {
        for (source, expected) in [
            ("```sh\n> [code](keep-me)\n```", "> [code](keep-me)"),
            ("> ```sh\n> > [code](keep-me)\n> ```", "> [code](keep-me)"),
            (
                "> > ~~~\n> > > [code](keep-me)\n> > ~~~",
                "> [code](keep-me)",
            ),
            ("- ```sh\n  > [code](keep-me)\n  ```", "> [code](keep-me)"),
            (
                "> - ```sh\n>   > [code](keep-me)\n>   ```",
                "> [code](keep-me)",
            ),
            ("```\n> ```\n[code](keep-me)\n```", "> ```\n[code](keep-me)"),
            ("> ```\n> literal\n[outside](drop)", "literal\noutside"),
        ] {
            assert_eq!(plain_text(source), expected, "source: {source}");
        }
    }

    #[test]
    fn containers_and_titles() {
        assert_eq!(plain_text("> > ~~~\n> > [x](keep)\n> > ~~~"), "[x](keep)");
        assert_eq!(plain_text("- ```\n  [x](keep)\n  ```"), "[x](keep)");
        assert_eq!(plain_text("[x][id]\n\n[id]: <url>\n  \"title\""), "x");
        assert_eq!(plain_text("[x](<a(b> \"t )\")"), "x");
        assert_eq!(plain_text("    [code](keep-me)"), "    [code](keep-me)");
    }

    #[test]
    fn reader_preserves_initial_code_and_encodings() {
        let path =
            std::env::temp_dir().join(format!("kensaku-md-reader-{}.md", std::process::id()));
        for be in [false, true] {
            let source = "    [code](keep-me)\n";
            let mut bytes = if be {
                vec![0xfe, 0xff]
            } else {
                vec![0xff, 0xfe]
            };
            for unit in source.encode_utf16() {
                bytes.extend(if be {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                });
            }
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(
                extract_markdown(&path, 1000).as_deref(),
                Some("    [code](keep-me)")
            );
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn review_regressions() {
        assert_eq!(plain_text("`a``b`"), "a``b");
        assert_eq!(
            plain_text("[label](https://example.com \"a ( title\")"),
            "label"
        );
        assert_eq!(plain_text("[label](\nhttps://example.com\n)"), "label");
        assert_eq!(
            plain_text("> ```rust\n> [code](keep-me)\n> ```"),
            "[code](keep-me)"
        );
    }

    #[test]
    fn frontmatter_values_and_body_are_searchable() {
        let text = plain_text(include_str!("../../../tests/fixtures/markdown/note.md"));
        for value in [
            "Searchable title",
            "rust",
            "検索",
            "Alternate name",
            "Hello world",
            "let answer = 42;",
            "Project page",
            "Diagram label",
        ] {
            assert!(text.contains(value), "missing {value:?}: {text}");
        }
        for markup in [
            "---",
            "```",
            "https://example.com",
            "images/diagram.png",
            "[Project page]",
        ] {
            assert!(!text.contains(markup), "retained {markup:?}: {text}");
        }
    }

    #[test]
    fn code_is_literal_and_fences_must_match() {
        assert_eq!(
            plain_text("````rust\n[code](keep-me)\n```\n````\n~~~\nx_y\n~~~"),
            "[code](keep-me)\n```\n\n\nx_y"
        );
        assert_eq!(
            plain_text("Use `[code](literal)` and [label](drop)."),
            "Use [code](literal) and label."
        );
    }

    #[test]
    fn links_handle_nested_destinations_escapes_and_references() {
        let text = plain_text(
            "[One](https://example.com/a_(b)) ![Alt](img.png \"title\")\n[Two][ID] [id][] [ID]\n[id]: https://example.com\n\\[literal](keep)",
        );
        assert_eq!(text, "One Alt Two id ID\n\n[literal](keep)");
    }

    #[test]
    fn frontmatter_with_crlf_bom_and_unusual_yaml_stays_literal() {
        assert_eq!(
            plain_text("\u{feff}---\r\ntitle: '[keep](value)'\r\ninvalid: [\r\n...\r\n# Body"),
            "title: '[keep](value)'\ninvalid: [\nBody"
        );
    }

    #[test]
    fn malformed_input_is_preserved_without_panicking() {
        assert_eq!(
            plain_text("---\ntitle: no end\n[broken](url\n```rust\ncode"),
            "--- title: no end [broken](url\n\ncode"
        );
        let links = "[a](".repeat(25_000);
        assert_eq!(plain_text(&links), links);
        let brackets = "[".repeat(100_000);
        assert_eq!(plain_text(&brackets), brackets);
    }
}
