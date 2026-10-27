//! Optional local LLM access for the query compiler and `/ask`.
//!
//! Kensaku is local-first: there is no LLM call anywhere on the search or
//! indexing path. This module exists **only** behind explicit configuration:
//! the user names a provider (Ollama or LM Studio by default, or any
//! OpenAI-compatible endpoint) and Kensaku calls it over loopback HTTP for two
//! narrow jobs:
//!
//! - **Query compilation** — natural language → a strict JSON of validated
//!   query tokens. The model output is parsed and validated against the
//!   existing query grammar before it is ever shown; invalid output is
//!   dropped, never guessed around.
//! - **`/ask`** — answer a question strictly from retrieval results with path
//!   citations. No retrieval, no answer.
//!
//! The model never retrieves on its own and is never required: every LLM
//! feature degrades to the deterministic path when the provider is off or
//! unreachable. Cloud endpoints are opt-in (`llm_allow_cloud`); nothing
//! leaves the machine unless the user turned that on.

pub mod ask;
pub mod compiler;

use crate::config::KensakuConfig;
use serde_json::{Value, json};
use std::io::Read as _;
use std::time::Duration;

/// Loopback defaults for the well-known runtimes.
pub const OLLAMA_DEFAULT_URL: &str = "http://127.0.0.1:11434";
pub const LMSTUDIO_DEFAULT_URL: &str = "http://127.0.0.1:1234/v1";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Small local models can be slow on first load; a cold Ollama call can take
/// tens of seconds. Generous, but bounded.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RESPONSE_CHARS: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Ollama (`/api/chat`, `/api/tags`).
    Ollama,
    /// LM Studio and anything speaking the OpenAI chat-completions shape.
    OpenAiCompatible,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmTarget {
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
}

impl LlmTarget {
    /// Builds the target from config. Returns `None` when the LLM is off or
    /// the configuration is incomplete; callers must treat that as the
    /// deterministic-only path.
    pub fn from_config(config: &KensakuConfig) -> Option<Self> {
        let provider = match config.llm_provider.as_str() {
            "ollama" => Provider::Ollama,
            "lmstudio" => Provider::OpenAiCompatible,
            "openai" if config.llm_allow_cloud => Provider::OpenAiCompatible,
            _ => return None,
        };
        let base_url = match (&config.llm_base_url[..], provider) {
            ("", Provider::Ollama) => OLLAMA_DEFAULT_URL.to_string(),
            ("", Provider::OpenAiCompatible) => LMSTUDIO_DEFAULT_URL.to_string(),
            (url, _) => url.trim_end_matches('/').to_string(),
        };
        let model = config.llm_model.trim().to_string();
        if model.is_empty() {
            return None;
        }
        Some(Self {
            provider,
            base_url,
            model,
        })
    }
}

#[derive(Debug)]
pub enum LlmError {
    /// No provider configured (the deterministic path should be used).
    NotConfigured,
    /// Configured but unreachable or misbehaving.
    Unavailable(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "no local LLM provider is configured"),
            Self::Unavailable(message) => write!(f, "LLM provider unavailable: {}", message),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

/// Sends one chat completion to the configured provider.
///
/// `cloud_allowed` must be true for non-loopbase base URLs; this is the
/// per-use consent gate for `/ask` and stays false for everything else.
pub fn chat(
    target: &LlmTarget,
    messages: &[ChatMessage],
    cloud_allowed: bool,
) -> Result<String, LlmError> {
    if !is_loopback(&target.base_url) && !cloud_allowed {
        return Err(LlmError::Unavailable(
            "endpoint is not loopback and cloud providers were not consented for this call"
                .to_string(),
        ));
    }

    let (url, body) = match target.provider {
        Provider::Ollama => (
            format!("{}/api/chat", target.base_url),
            json!({
                "model": target.model,
                "messages": messages,
                "stream": false,
                "options": { "temperature": 0.0 },
            }),
        ),
        Provider::OpenAiCompatible => (
            format!("{}/chat/completions", target.base_url),
            json!({
                "model": target.model,
                "messages": messages,
                "stream": false,
                "temperature": 0.0,
            }),
        ),
    };

    let response = agent()
        .post(&url)
        .send_json(body)
        .map_err(|e| LlmError::Unavailable(describe_ureq_error(e)))?;

    let mut text = String::new();
    let reader = response.into_reader();
    reader
        .take(MAX_RESPONSE_CHARS as u64)
        .read_to_string(&mut text)
        .map_err(|e| LlmError::Unavailable(format!("read failed: {}", e)))?;
    if text.len() >= MAX_RESPONSE_CHARS {
        return Err(LlmError::Unavailable(
            "response exceeded the size cap".to_string(),
        ));
    }

    let value: Value = serde_json::from_str(&text)
        .map_err(|e| LlmError::Unavailable(format!("invalid JSON from provider: {}", e)))?;

    let content = match target.provider {
        Provider::Ollama => value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .map(str::to_string),
        Provider::OpenAiCompatible => value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    content
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| LlmError::Unavailable("provider returned no content".to_string()))
}

/// Lists models available at the target, best effort. Used by the settings
/// UI to populate the model picker and by detection.
pub fn list_models(target: &LlmTarget) -> Result<Vec<String>, LlmError> {
    let url = match target.provider {
        Provider::Ollama => format!("{}/api/tags", target.base_url),
        Provider::OpenAiCompatible => format!("{}/models", target.base_url),
    };
    let response = agent()
        .get(&url)
        .call()
        .map_err(|e| LlmError::Unavailable(describe_ureq_error(e)))?;
    let mut text = String::new();
    let reader = response.into_reader();
    reader
        .take(MAX_RESPONSE_CHARS as u64)
        .read_to_string(&mut text)
        .map_err(|e| LlmError::Unavailable(format!("read failed: {}", e)))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| LlmError::Unavailable(format!("invalid JSON from provider: {}", e)))?;
    let models = match target.provider {
        Provider::Ollama => value["models"].as_array().map(|models| {
            models
                .iter()
                .filter_map(|m| m["name"].as_str())
                .map(str::to_string)
                .collect()
        }),
        Provider::OpenAiCompatible => value["data"].as_array().map(|models| {
            models
                .iter()
                .filter_map(|m| m["id"].as_str())
                .map(str::to_string)
                .collect()
        }),
    };
    Ok(models.unwrap_or_default())
}

/// True when a provider is configured, reachable, and has at least one model.
pub fn detect(target: &LlmTarget) -> Result<Vec<String>, LlmError> {
    list_models(target)
}

/// Shared HTTP client with bounded connect/total timeouts.
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
}

fn describe_ureq_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => {
            let mut body = String::new();
            let mut reader = response.into_reader();
            let _ = reader.by_ref().take(2_048).read_to_string(&mut body);
            format!("HTTP {}: {}", code, body.trim())
        }
        other => other.to_string(),
    }
}

/// Only loopback hosts are considered local; anything else is cloud.
fn is_loopback(base_url: &str) -> bool {
    let authority = base_url
        .strip_prefix("http://")
        .or_else(|| base_url.strip_prefix("https://"))
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or("");
    // Bracketed IPv6 keeps its colons inside the brackets.
    let host = if let Some(rest) = authority.strip_prefix('[') {
        format!("[{}]", rest.split(']').next().unwrap_or(""))
    } else {
        authority.split(':').next().unwrap_or("").to_string()
    };
    host == "127.0.0.1" || host.eq_ignore_ascii_case("localhost") || host == "[::1]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KensakuConfig;

    fn config(provider: &str, base_url: &str, model: &str) -> KensakuConfig {
        KensakuConfig {
            llm_provider: provider.to_string(),
            llm_base_url: base_url.to_string(),
            llm_model: model.to_string(),
            ..KensakuConfig::default()
        }
    }

    #[test]
    fn target_requires_provider_and_model() {
        assert!(LlmTarget::from_config(&config("off", "", "")).is_none());
        assert!(LlmTarget::from_config(&config("ollama", "", "llama3")).is_some());
        assert!(LlmTarget::from_config(&config("ollama", "", "")).is_none());
    }

    #[test]
    fn target_defaults_urls_per_provider() {
        let ollama = LlmTarget::from_config(&config("ollama", "", "m")).unwrap();
        assert_eq!(ollama.base_url, OLLAMA_DEFAULT_URL);
        let lmstudio = LlmTarget::from_config(&config("lmstudio", "", "m")).unwrap();
        assert_eq!(lmstudio.base_url, LMSTUDIO_DEFAULT_URL);
    }

    #[test]
    fn cloud_provider_requires_consent() {
        assert!(LlmTarget::from_config(&config("openai", "", "m")).is_none());
        let allowed = KensakuConfig {
            llm_allow_cloud: true,
            ..config("openai", "https://api.example.com/v1", "m")
        };
        assert!(LlmTarget::from_config(&allowed).is_some());
    }

    #[test]
    fn cloud_endpoints_refused_without_consent() {
        let target = LlmTarget {
            provider: Provider::OpenAiCompatible,
            base_url: "https://api.example.com/v1".to_string(),
            model: "m".to_string(),
        };
        let err = chat(&target, &[ChatMessage::user("hi")], false).unwrap_err();
        assert!(matches!(err, LlmError::Unavailable(ref m) if m.contains("consent")));
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("http://127.0.0.1:11434"));
        assert!(is_loopback("http://localhost:1234/v1"));
        assert!(is_loopback("http://[::1]:1234/v1"));
        assert!(!is_loopback("https://api.example.com/v1"));
    }

    #[test]
    fn unreachable_provider_reports_unavailable_not_panic() {
        let target = LlmTarget {
            provider: Provider::OpenAiCompatible,
            base_url: "http://127.0.0.1:9".to_string(),
            model: "m".to_string(),
        };
        // Port 9 (discard) — nothing listens; this must return an error.
        let result = chat(&target, &[ChatMessage::user("hi")], true);
        assert!(result.is_err());
    }
}
