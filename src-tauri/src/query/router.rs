use crate::query::ast::{Filter, Query, TermField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryRoute {
    NameCode,
    ContentPhrase,
    FilteredScope,
    SemanticSearch,
    VisualSearch,
    ScopeNavigation,
    General,
}

const NL_STOPWORDS: &[&str] = &[
    "how", "to", "what", "is", "where", "the", "a", "an", "in", "of", "for", "with", "on", "at",
    "by", "from", "and", "or", "why", "can", "i", "find", "which", "who", "show", "me", "all",
];

pub fn route_query(query: &Query) -> QueryRoute {
    let raw_trimmed = query.raw.trim();

    // 1. Ends with '/' -> pure scope navigation
    if raw_trimmed.ends_with('/') || raw_trimmed.ends_with('\\') {
        return QueryRoute::ScopeNavigation;
    }

    // 2. type: or in: present -> filtered, scope-first
    let has_type_filter = query.filters.iter().any(|f| match f {
        Filter::Type(_) | Filter::Ext(_) => true,
        Filter::Not(inner) => matches!(**inner, Filter::Type(_) | Filter::Ext(_)),
        _ => false,
    });
    let has_scope = !query.scopes.is_empty();
    if has_type_filter || has_scope {
        let has_image_filter = query.filters.iter().any(|f| match f {
            Filter::Type(t) => t == "image",
            Filter::Ext(ext) => matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "tif"
            ),
            _ => false,
        });
        if has_image_filter && !query.terms.is_empty() {
            return QueryRoute::VisualSearch;
        }
        return QueryRoute::FilteredScope;
    }

    // 3. quoted or re: -> content + phrase/regex
    let has_quote_or_regex = query
        .terms
        .iter()
        .any(|t| t.phrase || t.is_regex || t.field == TermField::Content);
    if has_quote_or_regex {
        return QueryRoute::ContentPhrase;
    }

    // Check tokens and spaces
    let tokens_count = query.terms.len();
    let has_space = raw_trimmed.chars().any(|c| c.is_whitespace());

    // 4. <=2 tokens, no spaces, has camelCase/_/. -> name-heavy + code tokenizer
    if tokens_count <= 2 && !has_space && is_code_identifier(raw_trimmed) {
        return QueryRoute::NameCode;
    }

    // 5. >=4 tokens or contains NL stopwords -> semantic search
    if tokens_count >= 4 || contains_nl_stopword(&query.terms) {
        return QueryRoute::SemanticSearch;
    }

    QueryRoute::General
}

fn is_code_identifier(s: &str) -> bool {
    if s.contains('_') || s.contains('.') || s.contains('-') {
        return true;
    }
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_lowercase() {
            prev_lower = true;
        } else if ch.is_uppercase() && prev_lower {
            return true;
        } else {
            prev_lower = false;
        }
    }
    false
}

fn contains_nl_stopword(terms: &[crate::query::ast::Term]) -> bool {
    for term in terms {
        let text_lower = term.text.to_lowercase();
        if NL_STOPWORDS.contains(&text_lower.as_str()) {
            return true;
        }
    }
    false
}
