//! Minimal search cleanup, not a Markdown renderer. Metadata and unsupported
//! syntax are retained as text. Shared by indexing, snippets and previews.
use regex::Regex;
use std::{path::Path, sync::OnceLock};

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

/// Remove top-level fence markers and common single-line link destinations.
/// Code contents stay literal. No YAML, references or container parsing.
pub fn strip(source: &str) -> String {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    let inline = INLINE.get_or_init(|| Regex::new(
        r#"!?\[([^\[\]\n]*)\]\([ \t]*(?:<[^<>\n]*>|[^()\s]+)(?:[ \t]+(?:"[^"\n]*"|'[^'\n]*'))?[ \t]*\)"#
    ).expect("constant Markdown cleanup pattern"));
    let mut out = String::with_capacity(source.len());
    let mut fence = None;
    for line in source.lines() {
        if let Some((marker, length)) = fence {
            if fence_marker(line)
                .is_some_and(|(m, n, rest)| m == marker && n >= length && rest.trim().is_empty())
            {
                fence = None;
            } else {
                out.push_str(line);
            }
        } else if let Some((marker, length, rest)) = fence_marker(line)
            && (marker != b'`' || !rest.contains('`'))
        {
            fence = Some((marker, length));
        } else if line.starts_with("    ")
            || line.starts_with('\t')
            || line.contains(['`', '\\'])
            || !line.contains("](")
        {
            out.push_str(line);
        } else {
            out.push_str(&inline.replace_all(line, "$1"));
        }
        out.push('\n');
    }
    out.trim_matches(['\r', '\n']).to_string()
}

fn fence_marker(line: &str) -> Option<(u8, usize, &str)> {
    let text = line.trim_start_matches(' ');
    if line.len() - text.len() > 3 {
        return None;
    }
    let marker = *text.as_bytes().first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let length = text.bytes().take_while(|&b| b == marker).count();
    (length >= 3).then_some((marker, length, &text[length..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_image_alt_text_does_not_keep_the_destination() {
        assert_eq!(strip("![](image.png)"), "");
        assert_eq!(
            strip("Before ![](<image path.png> \"title\") after"),
            "Before  after"
        );
        assert_eq!(strip("![Alt](image.png)"), "Alt");
        assert_eq!(strip("`![](literal.png)`"), "`![](literal.png)`");
        assert_eq!(strip("```\n![](literal.png)\n```"), "![](literal.png)");
    }

    #[test]
    fn strips_basic_destinations_and_fences_but_keeps_searchable_text() {
        assert_eq!(
            strip(
                "---\ntitle: Searchable\n---\n[Label](https://example.com) ![Alt](image.png \"title\")\n```rust\n> [code](keep)\n```"
            ),
            "---\ntitle: Searchable\n---\nLabel Alt\n\n> [code](keep)"
        );
        assert_eq!(
            strip("[Label](<some path>) [Other](url 'title')"),
            "Label Other"
        );
    }

    #[test]
    fn preserves_code_and_unsupported_syntax() {
        for source in [
            "    [code](keep)",
            "Use `[code](keep)` here",
            "[label][ref]\n[ref]: url",
            "[nested](url(with-parens))",
            "[broken](url",
            "[label](two words)",
            "[label](url \"title\" trailing)",
            "> ```\n> [code][ref]\n> ```",
            "---\ntext: |\n  ---\n  value\n---",
        ] {
            assert_eq!(strip(source), source);
        }
        assert_eq!(strip("````\n```\n[code](keep)\n````"), "```\n[code](keep)");
        assert_eq!(strip("~~~\ncode\n~~~"), "code");
        let malformed = "[".repeat(100_000);
        assert_eq!(strip(&malformed), malformed);
    }
}
