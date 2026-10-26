//! Local-LLM query compilation: natural language → validated query tokens.
//!
//! The deterministic intent compiler (`crate::query::intent`) handles common
//! phrasings without any model. When the user opts into a local LLM and the
//! deterministic pass has nothing to offer, this module asks the model for a
//! strict JSON refinement and **validates every token against the existing
//! query grammar** before it is used. Invalid output is dropped entirely;
//! there is no fallback guess, no silent rewrite, and no retrieval: the model
//! only ever sees the user's question text and returns tokens for the same
//! grammar the UI already exposes as removable chips.
//!
//! Results are cached (temperature 0 makes repeats deterministic, but a cache
//! also keeps repeated keystrokes from re-hitting the model).

use super::{ChatMessage, LlmError, LlmTarget};
use crate::query::InterpretationChip;
use crate::query::lexer::{Token, tokenize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

/// Keys an LLM is allowed to emit. Regex (`re:`) is deliberately excluded:
/// model-authored regex against a local index is an unbounded-cost footgun,
/// and `visual` stays beta-gated.
const ALLOWED_KEYS: &[&str] = &[
    "type", "ext", "in", "path", "modified", "size", "is", "lang", "name", "content", "mode",
];

const MAX_FILTERS: usize = 6;
const MAX_QUERY_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct CompiledQuery {
    /// Final composed query (terms + validated filter tokens), ready for the
    /// existing search pipeline.
    pub query: String,
    /// Just the text terms, filters removed.
    pub text: String,
    /// Validated filter tokens, e.g. `type:image`.
    pub filters: Vec<String>,
    /// Optional explicit sort: `newest` | `oldest` | `largest` | `name`.
    pub sort: Option<String>,
    /// Chips for the UI, same shape as the deterministic compiler emits.
    pub chips: Vec<InterpretationChip>,
}

/// Compile one natural-language query through the configured local model.
pub fn compile(target: &LlmTarget, raw: &str) -> Result<CompiledQuery, LlmError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(LlmError::Unavailable("empty query".to_string()));
    }
    if let Some(cached) = Cache::global().get(target, raw) {
        return Ok(cached);
    }

    let messages = [
        ChatMessage::system(SYSTEM_PROMPT),
        ChatMessage::user(raw),
    ];
    let response = super::chat(target, &messages, false)?;
    let compiled = parse_and_validate(raw, &response)
        .map_err(|reason| LlmError::Unavailable(format!("model output rejected: {}", reason)))?;
    Cache::global().put(target, raw, compiled.clone());
    Ok(compiled)
}

const SYSTEM_PROMPT: &str = r#"You translate a natural-language file-search request into a strict JSON object. Kensaku searches the user's local file index.

Reply with ONLY a JSON object (no prose, no code fences) shaped exactly like:
{"query": "<text terms only>", "filters": ["<filter token>", ...], "sort": null}

Rules:
- "query": the words to search for, filters and filler removed. Keep the user's language. No ":" tokens, no quotes-as-syntax, no regex.
- "filters": zero or more tokens from EXACTLY this grammar:
    type:<image|document|code|video|audio|archive|text|data|font>
    ext:<extension without dot>          e.g. ext:pdf
    in:"<directory path>"                scope, e.g. in:"~/Projects"
    path:"<substring>"                   path contains
    modified:<24h|7d|30d|365d>           recency window
    size:<"<N"|"<N"|">N"|">=N"|low..high> in bytes or with kb/mb/gb suffix, e.g. size:<10mb
    is:<image|dir|file>                  kind filter
    lang:<language>                      e.g. lang:rust
    name:"<substring>"                   file name contains
    content:"<substring>"                file content contains
    mode:<ranked|exhaustive>             rarely needed
  Never invent other keys. Never negate. At most 6 filters.
- "sort": null, or one of "newest", "oldest", "largest", "name" if the user asked for an ordering.
If the request is already plain keywords, return {"query": "<same words>", "filters": [], "sort": null}."#;

#[derive(Debug, Deserialize)]
struct ModelOutput {
    query: Option<String>,
    #[serde(default)]
    filters: Vec<String>,
    sort: Option<String>,
}

/// Validates raw model output into a [`CompiledQuery`], or rejects it.
pub fn parse_and_validate(_raw: &str, response: &str) -> Result<CompiledQuery, String> {
    let value = extract_json(response)?;
    let output: ModelOutput = serde_json::from_value(value)
        .map_err(|e| format!("unexpected JSON shape: {}", e))?;

    // Text terms: reject filter-like or regex-like content; the model must not
    // smuggle grammar into the text.
    let text = output.query.unwrap_or_default();
    let text = text.trim().to_string();
    if text.is_empty() && output.filters.is_empty() {
        return Err("empty result".to_string());
    }
    if text.chars().count() > MAX_QUERY_CHARS {
        return Err("query text exceeds 200 characters".to_string());
    }
    for token in tokenize(&text) {
        match token {
            Token::Word(word) | Token::NegatedWord(word) | Token::Quoted(word) => {
                if word.contains(':') {
                    return Err(format!("query text must not contain filter syntax: {:?}", word));
                }
            }
            Token::KeyValue { .. } | Token::NegatedQuoted(_) => {
                return Err("query text must not contain filter syntax".to_string());
            }
            Token::Regex(_) => {
                return Err("query text must not contain regex syntax".to_string());
            }
        }
    }

    // Filters: each must be a single, well-formed token with an allowed key.
    if output.filters.len() > MAX_FILTERS {
        return Err(format!("more than {} filters", MAX_FILTERS));
    }
    let mut filters = Vec::with_capacity(output.filters.len());
    let mut chips = Vec::with_capacity(output.filters.len());
    for filter in &output.filters {
        let token = validate_filter_token(filter)?;
        filters.push(token.clone());
        chips.push(chip_for(filter, &token));
    }

    // Sort: only the exact vocabulary the UI already supports.
    let sort = match output.sort.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("null") => None,
        Some(word) => match word.to_ascii_lowercase().as_str() {
            "newest" | "oldest" | "largest" | "name" => Some(word.to_ascii_lowercase()),
            other => return Err(format!("unknown sort: {:?}", other)),
        },
    };

    let mut query = text.clone();
    for filter in &filters {
        if !query.is_empty() {
            query.push(' ');
        }
        query.push_str(filter);
    }

    Ok(CompiledQuery {
        query,
        text,
        filters,
        sort,
        chips,
    })
}

/// One filter token must tokenize to exactly one KeyValue whose key is in the
/// allow-list and whose value is non-empty (mode allows nothing else).
fn validate_filter_token(filter: &str) -> Result<String, String> {
    let filter = filter.trim();
    if filter.is_empty() || filter.chars().count() > 120 {
        return Err("filter token is empty or too long".to_string());
    }
    let tokens = tokenize(filter);
    if tokens.len() != 1 {
        return Err(format!("filter token must be a single token: {:?}", filter));
    }
    let (key, value) = match &tokens[0] {
        Token::KeyValue { key, value, .. } => (key, value),
        Token::Regex(_) => return Err("filter key not allowed: re".to_string()),
        _ => return Err(format!("filter token must be key:value: {:?}", filter)),
    };
    let key_lower = key.to_lowercase();
    if !ALLOWED_KEYS.contains(&key_lower.as_str()) {
        return Err(format!("filter key not allowed: {:?}", key));
    }
    if key_lower != "mode" && value.trim().is_empty() {
        return Err(format!("filter value is empty: {:?}", filter));
    }
    Ok(filter.to_string())
}

/// Maps a validated token onto the chip vocabulary the UI already renders.
fn chip_for(source: &str, token: &str) -> InterpretationChip {
    let key = tokenize(token)
        .first()
        .and_then(|t| match t {
            Token::KeyValue { key, .. } => Some(key.to_lowercase()),
            _ => None,
        })
        .unwrap_or_default();
    let kind = match key.as_str() {
        "in" | "path" => "place",
        "modified" => "time",
        "size" => "size",
        "mode" => "mode",
        _ => "type",
    };
    InterpretationChip {
        kind: kind.to_string(),
        label: chip_label(&key, source, token),
        raw: token.to_string(),
        source: source.to_string(),
    }
}

fn chip_label(key: &str, source: &str, token: &str) -> String {
    let _ = source;
    let value = tokenize(token)
        .first()
        .and_then(|t| match t {
            Token::KeyValue { value, .. } => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_default();
    match key {
        "modified" => format!("Modified: {}", value),
        "size" => format!("Size: {}", value),
        "in" | "path" => format!("In: {}", value),
        "ext" => format!("Type: {}", value),
        "type" => format!("Type: {}", value),
        "is" => format!("Kind: {}", value),
        "lang" => format!("Language: {}", value),
        "mode" => format!("Mode: {}", value),
        _ => {
            let _ = source;
            token.to_string()
        }
    }
}

/// Pulls the outermost JSON object out of a model response that may be wrapped
/// in prose or code fences.
fn extract_json(response: &str) -> Result<Value, String> {
    let start = response
        .find('{')
        .ok_or_else(|| "no JSON object in response".to_string())?;
    let end = response
        .rfind('}')
        .filter(|end| *end > start)
        .ok_or_else(|| "unterminated JSON object".to_string())?;
    serde_json::from_str(&response[start..=end])
        .map_err(|e| format!("invalid JSON: {}", e))
}

/// Tiny bounded LRU keyed by provider + model + normalized input. Hand-rolled
/// to keep the dependency tree unchanged; 128 entries is plenty for a search
/// session.
pub struct Cache {
    map: std::sync::Mutex<Inner>,
}

struct Inner {
    order: VecDeque<String>,
    entries: HashMap<String, CompiledQuery>,
}

impl Cache {
    fn global() -> &'static Cache {
        static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
        CACHE.get_or_init(|| Cache {
            map: std::sync::Mutex::new(Inner {
                order: VecDeque::new(),
                entries: HashMap::new(),
            }),
        })
    }

    fn key(target: &LlmTarget, raw: &str) -> String {
        format!(
            "{}\u{0}{}\u{0}{}",
            target.provider.as_str(),
            target.model,
            raw.trim().to_lowercase()
        )
    }

    fn get(&self, target: &LlmTarget, raw: &str) -> Option<CompiledQuery> {
        let key = Self::key(target, raw);
        let mut inner = self.map.lock().expect("llm cache lock");
        if let Some(value) = inner.entries.get(&key).cloned() {
            if let Some(pos) = inner.order.iter().position(|k| *k == key) {
                inner.order.remove(pos);
                inner.order.push_back(key);
            }
            return Some(value);
        }
        None
    }

    fn put(&self, target: &LlmTarget, raw: &str, value: CompiledQuery) {
        const CAPACITY: usize = 128;
        let key = Self::key(target, raw);
        let mut inner = self.map.lock().expect("llm cache lock");
        if inner.entries.contains_key(&key)
            && let Some(pos) = inner.order.iter().position(|k| *k == key)
        {
            inner.order.remove(pos);
        }
        inner.entries.insert(key.clone(), value);
        inner.order.push_back(key);
        while inner.order.len() > CAPACITY {
            let oldest = inner.order.pop_front();
            if let Some(oldest) = oldest {
                inner.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn validate(raw: &str, response: Value) -> Result<CompiledQuery, String> {
        parse_and_validate(raw, &response.to_string())
    }

    #[test]
    fn accepts_clean_output_and_composes_query() {
        let compiled = validate(
            "recent big images in Downloads",
            json!({
                "query": "images",
                "filters": ["type:image", "modified:7d", "size:>1mb", "in:\"~/Downloads\""],
                "sort": null
            }),
        )
        .expect("valid output");
        assert_eq!(compiled.text, "images");
        assert_eq!(compiled.filters.len(), 4);
        assert_eq!(
            compiled.query,
            "images type:image modified:7d size:>1mb in:\"~/Downloads\""
        );
        assert_eq!(compiled.chips.len(), 4);
        assert!(compiled.chips.iter().any(|chip| chip.kind == "place"));
        assert!(compiled.chips.iter().any(|chip| chip.kind == "time"));
    }

    #[test]
    fn accepts_sort_vocabulary_only() {
        let compiled = validate(
            "latest resume",
            json!({"query": "resume", "filters": [], "sort": "newest"}),
        )
        .expect("valid");
        assert_eq!(compiled.sort.as_deref(), Some("newest"));

        let err = validate(
            "x",
            json!({"query": "x", "filters": [], "sort": "random"}),
        )
        .unwrap_err();
        assert!(err.contains("unknown sort"));
    }

    #[test]
    fn rejects_regex_and_disallowed_keys() {
        let err = validate(
            "x",
            json!({"query": "x", "filters": ["re:\"a|b\""], "sort": null}),
        )
        .unwrap_err();
        assert!(err.contains("not allowed"), "got: {err}");

        let err = validate(
            "x",
            json!({"query": "x", "filters": ["sudo:rm"], "sort": null}),
        )
        .unwrap_err();
        assert!(err.contains("not allowed"), "got: {err}");
    }

    #[test]
    fn rejects_filter_syntax_in_text() {
        let err = validate(
            "x",
            json!({"query": "type:image invoice", "filters": [], "sort": null}),
        )
        .unwrap_err();
        assert!(err.contains("filter syntax"), "got: {err}");

        let err = validate(
            "x",
            json!({"query": "re:\".*\"", "filters": [], "sort": null}),
        )
        .unwrap_err();
        assert!(err.contains("regex"), "got: {err}");
    }

    #[test]
    fn rejects_over_limit_filters_and_garbage() {
        let filters: Vec<String> = (0..8).map(|i| format!("ext:e{}", i)).collect();
        let err = validate("x", json!({"query": "x", "filters": filters, "sort": null}))
            .unwrap_err();
        assert!(err.contains("more than 6"));

        assert!(parse_and_validate("x", "no json here").is_err());
        assert!(parse_and_validate("x", "{\"query\":").is_err());
    }

    #[test]
    fn extracts_json_from_prose_and_fences() {
        let response = "Sure! Here it is:\n```json\n{\"query\": \"needle\", \"filters\": [], \"sort\": null}\n```";
        let compiled = parse_and_validate("needle docs", response).expect("extracted");
        assert_eq!(compiled.query, "needle");
    }

    #[test]
    fn empty_result_is_rejected() {
        let err = validate("x", json!({"query": "", "filters": [], "sort": null})).unwrap_err();
        assert!(err.contains("empty result"));
    }

    #[test]
    fn chips_carry_labels_and_sources() {
        let compiled = validate(
            "big pdfs at home",
            json!({
                "query": "pdfs",
                "filters": ["ext:pdf", "size:>10mb", "in:\"~\""],
                "sort": null
            }),
        )
        .expect("valid");
        assert!(compiled.chips.iter().any(|c| c.label == "Size: >10mb"));
        assert!(compiled
            .chips
            .iter()
            .any(|c| c.label.starts_with("In: ")));
        assert!(compiled
            .chips
            .iter()
            .all(|c| !c.source.is_empty()));
    }

    #[test]
    fn cache_roundtrip_and_eviction() {
        let cache = Cache {
            map: std::sync::Mutex::new(Inner {
                order: VecDeque::new(),
                entries: HashMap::new(),
            }),
        };
        let target = LlmTarget {
            provider: super::super::Provider::Ollama,
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "m".to_string(),
        };
        let compiled = CompiledQuery {
            query: "needle".to_string(),
            text: "needle".to_string(),
            filters: vec![],
            sort: None,
            chips: vec![],
        };
        assert!(cache.get(&target, "needle").is_none());
        cache.put(&target, "needle", compiled.clone());
        assert_eq!(cache.get(&target, "NEEDLE ").unwrap().query, "needle");
        for i in 0..130 {
            cache.put(&target, &format!("q{}", i), compiled.clone());
        }
        assert!(cache.get(&target, "needle").is_none(), "oldest entry evicted");
    }
}
