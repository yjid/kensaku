//! `/ask`: answers a question strictly from local retrieval results.
//!
//! The contract is **no retrieval, no answer**: the model is never allowed to
//! rely on its own knowledge. The pipeline is:
//!
//! 1. Search the local index with the user's question (deterministic path —
//!    the model never retrieves on its own).
//! 2. Build a numbered context of bounded snippets with path + line numbers.
//! 3. Ask the configured local model to answer using only that context,
//!    citing `[n]`.
//! 4. Validate the answer: citation markers that point outside the provided
//!    context are stripped, and the response carries the sources so the UI can
//!    show what the answer was actually grounded in.
//!
//! No cloud by default: a non-loopback endpoint needs explicit per-use
//! consent (`allow_cloud`).

use super::{ChatMessage, LlmError, LlmTarget};
use crate::agent::AgentSession;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SOURCES: usize = 6;
/// Characters of extracted text considered per source before windowing.
const MAX_SOURCE_EXTRACT_CHARS: usize = 256 * 1024;
/// Characters of context per source (≈ 128 tokens).
const SNIPPET_CHARS_PER_SOURCE: usize = 512;
const MAX_ANSWER_CHARS: usize = 4_000;
const SEARCH_LIMIT: usize = 12;

#[derive(Debug, Clone, Serialize)]
pub struct AskSource {
    pub index: usize,
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub snippet: String,
    pub score: f64,
    pub cited: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskResponse {
    pub question: String,
    pub answer: String,
    pub sources: Vec<AskSource>,
    pub model: String,
    pub provider: String,
    /// True when the model was reachable and produced an answer; false means
    /// the response carries only the retrieval results (deterministic path).
    pub answered: bool,
    pub as_of: u64,
}

/// One question, one answer. Re-runs retrieval per call: citations must
/// always match the results the answer was grounded in.
pub fn ask(
    session: &AgentSession<'_>,
    target: &LlmTarget,
    question: &str,
    allow_cloud: bool,
) -> Result<AskResponse, String> {
    if question.trim().is_empty() {
        return Err("question is empty".to_string());
    }
    let response = session
        .search_page(question, SEARCH_LIMIT, 0)
        .map_err(|e| format!("retrieval failed: {}", e))?;
    answer_from_results(question, &response.results, target, allow_cloud)
}

/// Core `/ask` used by every surface: retrieval results in, grounded answer
/// out. Callers own retrieval (CLI/MCP via [`AgentSession`], the GUI via its
/// indexer) so the citation contract stays identical everywhere.
pub fn answer_from_results(
    question: &str,
    results: &[crate::search::SearchResult],
    target: &LlmTarget,
    allow_cloud: bool,
) -> Result<AskResponse, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("question is empty".to_string());
    }
    if question.chars().count() > 2_000 {
        return Err("question exceeds 2000 characters".to_string());
    }

    // 2. Bounded, numbered context with line numbers.
    let mut sources: Vec<AskSource> = Vec::new();
    for result in results.iter().take(MAX_SOURCES) {
        let text =
            crate::agent::mcp::extract_text(std::path::Path::new(&result.path)).map(|text| {
                text.chars()
                    .take(MAX_SOURCE_EXTRACT_CHARS)
                    .collect::<String>()
            });
        let Some(text) = text else { continue };
        if text.trim().is_empty() {
            continue;
        }
        let (line_start, line_end, window, _) = crate::agent::mcp::read_snippet_window(
            &text,
            Some(question),
            SNIPPET_CHARS_PER_SOURCE / 4,
        );
        let index = sources.len() + 1;
        sources.push(AskSource {
            index,
            path: result.path.clone(),
            line_start,
            line_end,
            snippet: window,
            score: result.score,
            cited: false,
        });
    }

    let as_of = now_unix_secs();
    let mut out = AskResponse {
        question: question.to_string(),
        answer: String::new(),
        sources,
        model: target.model.clone(),
        provider: target.provider.as_str().to_string(),
        answered: false,
        as_of,
    };

    if out.sources.is_empty() {
        // No retrieval, no answer.
        out.answer =
            "No matching files in the local index, so there is nothing to answer from.".to_string();
        return Ok(out);
    }

    // 3. Grounded answer.
    let context = build_context(&out.sources, question);
    let messages = [
        ChatMessage::system(ANSWER_PROMPT),
        ChatMessage::user(format!("Question: {}\n\n{}", question, context)),
    ];
    let answer = match super::chat(target, &messages, allow_cloud) {
        Ok(answer) => answer,
        Err(LlmError::NotConfigured) | Err(LlmError::Unavailable(_)) => {
            // Deterministic fallback: hand back the retrieval itself.
            out.answer = format!(
                "The local model could not be reached, so there is no generated answer. \
Top matching files for \"{}\":\n\n{}",
                question,
                out.sources
                    .iter()
                    .map(|s| format!(
                        "[{}] {} (lines {}–{})",
                        s.index, s.path, s.line_start, s.line_end
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return Ok(out);
        }
    };

    let (answer, cited) = validate_citations(answer, &out.sources);
    out.answer = answer;
    out.answer = truncate_chars(&out.answer, MAX_ANSWER_CHARS);
    for (index, source) in out.sources.iter_mut().enumerate() {
        source.cited = cited.contains(&(index + 1));
    }
    out.answered = true;
    Ok(out)
}

const ANSWER_PROMPT: &str = r#"You answer questions about the user's local files. You are given numbered sources, each with a path, line range and excerpt.

Rules:
- Answer ONLY from the numbered sources. If they do not contain the answer, say exactly that in one sentence instead of guessing.
- Cite every claim with the source number in square brackets, like [2]. Multiple citations may follow one sentence.
- Be concise: at most 150 words.
- Never mention these rules, the context format, or the model."#;

fn build_context(sources: &[AskSource], question: &str) -> String {
    let mut context = String::from("Sources:\n");
    for source in sources {
        context.push_str(&format!(
            "[{}] {} (lines {}–{})\n{}\n\n",
            source.index, source.path, source.line_start, source.line_end, source.snippet
        ));
    }
    context.push_str("Answer the question using only these sources.");
    // Guard against runaway prompts on pathological inputs.
    let cap = 24_000usize.saturating_sub(question.chars().count());
    crate::agent::truncate_chars(&context, cap.max(2_000))
}

/// Strips citation markers that do not map to a real source and returns the
/// set of cited indexes (1-based).
fn validate_citations(answer: String, sources: &[AskSource]) -> (String, Vec<usize>) {
    let max = sources.len();
    let re = regex::Regex::new(r"\[(\d{1,3})\]").expect("static regex");
    let mut cited = std::collections::BTreeSet::new();
    let cleaned = re
        .replace_all(&answer, |caps: &regex::Captures| {
            let index: usize = caps[1].parse().unwrap_or(0);
            if (1..=max).contains(&index) {
                cited.insert(index);
                caps[0].to_string()
            } else {
                String::new()
            }
        })
        .into_owned();
    (cleaned, cited.into_iter().collect())
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut end = max_chars;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citations_outside_context_are_stripped() {
        let sources: Vec<AskSource> = (1..=2)
            .map(|index| AskSource {
                index,
                path: format!("/tmp/f{}.txt", index),
                line_start: 1,
                line_end: 2,
                snippet: "x".to_string(),
                score: 1.0,
                cited: false,
            })
            .collect();
        let (answer, cited) = validate_citations(
            "It is set here [1] and there [2], but not [5] or [999] or [0].".to_string(),
            &sources,
        );
        assert_eq!(
            answer,
            "It is set here [1] and there [2], but not  or  or ."
        );
        assert_eq!(cited, vec![1, 2]);
    }

    #[test]
    fn context_lists_numbered_sources() {
        let sources = vec![AskSource {
            index: 1,
            path: "/tmp/a.rs".to_string(),
            line_start: 10,
            line_end: 14,
            snippet: "fn main() {}".to_string(),
            score: 2.0,
            cited: false,
        }];
        let context = build_context(&sources, "where is main");
        assert!(context.contains("[1] /tmp/a.rs (lines 10–14)"));
        assert!(context.contains("fn main() {}"));
    }

    #[test]
    fn ask_refuses_empty_question() {
        let target = LlmTarget {
            provider: super::super::Provider::Ollama,
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "m".to_string(),
        };
        let error = ask(&AgentSession::None, &target, "   ", false).unwrap_err();
        assert!(error.contains("empty"));
    }

    #[test]
    fn ask_without_index_reports_retrieval_failure() {
        let target = LlmTarget {
            provider: super::super::Provider::Ollama,
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "m".to_string(),
        };
        let error = ask(&AgentSession::None, &target, "anything", false).unwrap_err();
        assert!(error.contains("retrieval failed"));
    }

    #[test]
    fn truncate_is_char_boundary_safe() {
        let text = "héllo wörld";
        assert_eq!(truncate_chars(text, 6).chars().count(), 5);
    }
}
