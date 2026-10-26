//! Deterministic natural-language intent compilation.
//!
//! Small, local, explainable rules map common phrasings ("latest resume",
//! "screenshots from last week", "big videos in Projects") onto the existing
//! query grammar plus an explicit sort. No model, no network, no silent
//! rewrites: every emitted filter is returned as a removable chip.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpretationChip {
    /// "type" | "time" | "size" | "sort" | "place"
    pub kind: String,
    pub label: String,
    /// The filter token appended to the query, e.g. `type:image`.
    pub raw: String,
    /// The words in the user's input that produced this chip.
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct Intent {
    /// Query with intent phrases removed and filters appended.
    pub query: String,
    /// Explicit sort requested by language, if any.
    pub sort: Option<String>,
    pub chips: Vec<InterpretationChip>,
}

fn take(
    work: &mut String,
    re: &Regex,
    kind: &str,
    label: &str,
    raw: &str,
    appends: &mut Vec<String>,
    chips: &mut Vec<InterpretationChip>,
) {
    if let Some(m) = re.find(work) {
        let source = m.as_str().trim().to_string();
        *work = format!("{} {}", &work[..m.start()], &work[m.end()..]);
        appends.push(raw.to_string());
        chips.push(InterpretationChip {
            kind: kind.to_string(),
            label: label.to_string(),
            raw: raw.to_string(),
            source,
        });
    }
}

/// Compiles natural language into the existing query grammar.
pub fn compile(raw: &str) -> Intent {
    // Explicit grammar is authoritative: never reinterpret a query that
    // already contains a filter token.
    let has_explicit_filter = raw.split_whitespace().any(|t| {
        t.contains(':') && !t.starts_with("http") && !t.starts_with("https")
    });

    let mut work = raw.to_string();
    let mut appends: Vec<String> = Vec::new();
    let mut chips: Vec<InterpretationChip> = Vec::new();
    let mut sort: Option<String> = None;

    if !has_explicit_filter {
        // --- Sort words -------------------------------------------------
        for (pat, order, label) in [
            (r"(?i)\b(?:latest|newest|most recent)\b", "newest", "Newest"),
            (r"(?i)\boldest\b", "oldest", "Oldest"),
            (r"(?i)\b(?:biggest|largest)\b", "largest", "Largest"),
        ] {
            if sort.is_none() {
                if let Ok(re) = Regex::new(pat)
                    && re.is_match(&work)
                {
                    let source = re.find(&work).map(|m| m.as_str().to_string()).unwrap_or_default();
                    work = re.replace(&work, " ").to_string();
                    sort = Some(order.to_string());
                    chips.push(InterpretationChip {
                        kind: "sort".to_string(),
                        label: format!("Sort: {}", label),
                        raw: format!("sort:{}", order),
                        source,
                    });
                }
            }
        }

        // --- Time windows ----------------------------------------------
        for (pat, filter, label) in [
            (r"(?i)\btoday\b", "modified:<1d", "Modified: today"),
            (r"(?i)\byesterday\b", "modified:1d..2d", "Modified: yesterday"),
            (r"(?i)\b(?:this|last|past) week\b", "modified:<7d", "Modified: last 7 days"),
            (r"(?i)\b(?:this|last|past) month\b", "modified:<30d", "Modified: last 30 days"),
            (r"(?i)\b(?:this|last|past) year\b", "modified:<365d", "Modified: last year"),
        ] {
            if let Ok(re) = Regex::new(pat) {
                take(&mut work, &re, "time", label, filter, &mut appends, &mut chips);
            }
        }

        // --- Size -------------------------------------------------------
        if let Ok(re) = Regex::new(r"(?i)\b(?:under|less than|smaller than)\s+(\d+(?:\.\d+)?)\s*(kb|mb|gb)\b") {
            if let Some(c) = re.captures(&work) {
                let number = c[1].to_string();
                let unit = c[2].to_string();
                let raw = format!("size:<{}{}", number, unit.to_lowercase());
                let source = c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
                work = re.replace(&work, " ").to_string();
                appends.push(raw.clone());
                chips.push(InterpretationChip {
                    kind: "size".to_string(),
                    label: format!("Size: under {}{}", number, unit.to_uppercase()),
                    raw,
                    source,
                });
            }
        }
        if let Ok(re) = Regex::new(r"(?i)\b(?:over|more than|larger than|bigger than)\s+(\d+(?:\.\d+)?)\s*(kb|mb|gb)\b") {
            if let Some(c) = re.captures(&work) {
                let number = c[1].to_string();
                let unit = c[2].to_string();
                let raw = format!("size:>{}{}", number, unit.to_lowercase());
                let source = c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
                work = re.replace(&work, " ").to_string();
                appends.push(raw.clone());
                chips.push(InterpretationChip {
                    kind: "size".to_string(),
                    label: format!("Size: over {}{}", number, unit.to_uppercase()),
                    raw,
                    source,
                });
            }
        }
        for (pat, filter, label) in [
            (r"(?i)\b(?:huge|big|large)\b", "size:>100mb", "Size: over 100 MB"),
            (r"(?i)\b(?:tiny|small)\b", "size:<1mb", "Size: under 1 MB"),
        ] {
            if let Ok(re) = Regex::new(pat) {
                take(&mut work, &re, "size", label, filter, &mut appends, &mut chips);
            }
        }

        // --- Media / document nouns ------------------------------------
        for (pat, filter, label) in [
            (r"(?i)\b(?:screenshots?|images?|photos?|pictures?)\b", "type:image", "Type: images"),
            (r"(?i)\bpdfs?\b", "ext:pdf", "Type: PDF"),
            (r"(?i)\b(?:videos?|movies?|clips?)\b", "type:media", "Type: media"),
            (r"(?i)\b(?:audio|music|songs?)\b", "type:media", "Type: media"),
            (r"(?i)\b(?:code|source)\b", "type:code", "Type: code"),
            (r"(?i)\b(?:documents?|docs?)\b", "type:doc", "Type: documents"),
            (r"(?i)\b(?:spreadsheets?|excel)\b", "type:doc", "Type: documents"),
            (r"(?i)\b(?:presentations?|slides?)\b", "type:doc", "Type: documents"),
            (r"(?i)\b(?:folders?|directories|directory)\b", "type:dir", "Type: folders"),
            (r"(?i)\b(?:archives?|zips?)\b", "type:archive", "Type: archives"),
            (r"(?i)\blogs?\b", "ext:log", "Type: logs"),
        ] {
            if let Ok(re) = Regex::new(pat) {
                take(&mut work, &re, "type", label, filter, &mut appends, &mut chips);
            }
        }

        // --- Place ------------------------------------------------------
        if let Ok(re) = Regex::new(
            r"(?i)\bin\s+(?:my\s+)?(projects|downloads|documents|desktop|pictures|videos|music|work)\b",
        ) {
            if let Some(c) = re.captures(&work) {
                let place = c[1].to_string();
                let raw = format!("in:{}", place);
                let source = c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
                work = re.replace(&work, " ").to_string();
                appends.push(raw.clone());
                chips.push(InterpretationChip {
                    kind: "place".to_string(),
                    label: format!("In: {}", place),
                    raw,
                    source,
                });
            }
        }
    }

    if chips.is_empty() {
        return Intent {
            query: raw.to_string(),
            sort: None,
            chips: Vec::new(),
        };
    }

    // Drop connective filler left behind once the intent words are removed.
    if let Ok(re) = Regex::new(r"(?i)\b(?:from|the|a|an|my|me|show|find|get|give|all|some|of)\b") {
        work = re.replace_all(&work, " ").to_string();
    }

    let mut query = work.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut seen = HashSet::new();
    for token in appends {
        if seen.insert(token.clone()) {
            if !query.is_empty() {
                query.push(' ');
            }
            query.push_str(&token);
        }
    }

    if query.trim().is_empty() {
        query = raw.to_string();
    }

    Intent {
        query,
        sort,
        chips,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_resume_becomes_newest_sort() {
        let intent = compile("latest resume");
        assert_eq!(intent.sort.as_deref(), Some("newest"));
        assert_eq!(intent.query, "resume");
        assert!(intent.chips.iter().any(|c| c.raw == "sort:newest"));
    }

    #[test]
    fn screenshots_from_last_week_adds_type_and_time() {
        let intent = compile("screenshots from last week");
        assert!(intent.query.contains("type:image"));
        assert!(intent.query.contains("modified:<7d"));
    }

    #[test]
    fn big_videos_in_projects_composes_filters() {
        let intent = compile("big videos in Projects");
        assert!(intent.sort.is_none());
        assert!(intent.query.contains("size:>100mb"));
        assert!(intent.query.contains("type:media"));
        assert!(intent.query.contains("in:Projects"));
    }

    #[test]
    fn explicit_grammar_is_not_reinterpreted() {
        let intent = compile("type:pdf latest");
        assert!(intent.chips.is_empty());
        assert_eq!(intent.query, "type:pdf latest");
    }

    #[test]
    fn plain_query_is_untouched() {
        let intent = compile("docker error");
        assert!(intent.chips.is_empty());
        assert_eq!(intent.query, "docker error");
    }
}
