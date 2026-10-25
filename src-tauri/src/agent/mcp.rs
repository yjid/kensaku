//! Model Context Protocol server over stdio.
//!
//! Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout. **stdout is
//! reserved for protocol messages**; diagnostics go to stderr. The index is
//! opened once per process, so per-call cost is just the query.

use super::{HeadlessIndex, NO_INDEX_MESSAGE, compose_query, now_unix_secs, truncate_chars};
use crate::indexer::content::read_text_content;
use crate::indexer::extract::extract_document;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

pub const PROTOCOL_VERSION: &str = "2024-11-05";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Serves MCP over stdin/stdout until EOF.
pub fn serve() -> Result<(), String> {
    // A missing index is not fatal: `initialize` must still answer, and tool
    // calls return a clear "no index" error until the GUI has run once.
    let index = match HeadlessIndex::open() {
        Ok(index) => Some(index),
        Err(e) => {
            eprintln!("[Kensaku MCP] {}", e);
            None
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("Failed to read stdin: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle_request(index.as_ref(), &request),
            Err(e) => Some(error_response(
                Value::Null,
                PARSE_ERROR,
                &format!("Parse error: {}", e),
            )),
        };

        if let Some(response) = response {
            let encoded = serde_json::to_string(&response)
                .map_err(|e| format!("Failed to encode response: {}", e))?;
            writeln!(out, "{}", encoded).map_err(|e| format!("Failed to write stdout: {}", e))?;
            out.flush()
                .map_err(|e| format!("Failed to flush stdout: {}", e))?;
        }
    }

    Ok(())
}

/// Handles one decoded JSON-RPC message.
///
/// Returns `None` for notifications (no reply is allowed). `index` is `None`
/// when the local index could not be opened, which keeps `initialize` usable
/// while tool calls fail with a helpful message.
pub fn handle_request(index: Option<&HeadlessIndex>, request: &Value) -> Option<Value> {
    let Some(object) = request.as_object() else {
        return Some(error_response(
            Value::Null,
            PARSE_ERROR,
            "Parse error: request must be a JSON object",
        ));
    };

    let id = object.get("id").cloned();
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return id.map(|id| error_response(id, INVALID_REQUEST, "Invalid Request: missing method"));
    };

    // Notifications never get a reply (includes `notifications/initialized`).
    if method.starts_with("notifications/") {
        return None;
    }
    // No id means notification semantics even for request-like methods.
    let id = id?;

    let outcome: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(index, object.get("params")),
        other => Err((METHOD_NOT_FOUND, format!("Method not found: {}", other))),
    };

    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error_response(id, code, &message),
    })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "kensaku",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Search the local Kensaku file index. Call tools/list to discover tools.",
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "search",
            "description": "Search the local Kensaku index (file names, paths and document contents). Returns ranked results with snippets.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query. Supports Kensaku syntax (ext:, in:, content:, re:, modified:)."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 50,
                        "default": 10,
                        "description": "Maximum number of results."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["ranked", "exhaustive"],
                        "default": "ranked",
                        "description": "Ranked order or exhaustive enumeration by path."
                    },
                    "path": {
                        "type": "string",
                        "description": "Only return results under this directory."
                    },
                    "ext": {
                        "type": "string",
                        "description": "Only return files with this extension (e.g. rs or .rs)."
                    },
                    "modified_after": {
                        "type": "string",
                        "description": "Only files modified within this duration (e.g. 1h, 7d, 30d)."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "read_snippet",
            "description": "Read a bounded snippet from an indexed file, centered on the first case-insensitive match for an optional query.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path of the file to read."
                    },
                    "query": {
                        "type": "string",
                        "description": "Text to center the snippet on (case-insensitive)."
                    },
                    "max_tokens": {
                        "type": "integer",
                        "minimum": 32,
                        "maximum": 1024,
                        "default": 256,
                        "description": "Approximate token budget (about four characters per token)."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "index_state",
            "description": "Report index freshness and size: file/dir counts, content documents, semantic units and watched roots.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }),
    ]
}

enum ToolFailure {
    /// Protocol-level invalid arguments (mapped to JSON-RPC -32602).
    Invalid(String),
    /// Tool ran but failed (returned as a tool result with `isError`).
    Execution(String),
}

fn call_tool(
    index: Option<&HeadlessIndex>,
    params: Option<&Value>,
) -> Result<Value, (i64, String)> {
    let Some(params) = params.and_then(Value::as_object) else {
        return Err((
            INVALID_PARAMS,
            "Invalid params: tools/call requires an object".to_string(),
        ));
    };

    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err((
            INVALID_PARAMS,
            "Invalid params: tool 'name' is required".to_string(),
        ));
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let outcome: Result<Value, ToolFailure> = match name {
        "search" => tool_search(index, &arguments),
        "read_snippet" => tool_read_snippet(&arguments),
        "index_state" => tool_index_state(index),
        other => {
            return Err((INVALID_PARAMS, format!("Unknown tool: {}", other)));
        }
    };

    Ok(match outcome {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": value.to_string() }],
            "isError": false,
        }),
        Err(ToolFailure::Invalid(message)) => return Err((INVALID_PARAMS, message)),
        Err(ToolFailure::Execution(message)) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
        }),
    })
}

fn tool_search(index: Option<&HeadlessIndex>, args: &Value) -> Result<Value, ToolFailure> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| ToolFailure::Invalid("Invalid params: 'query' is required".to_string()))?;

    let limit = match args.get("limit") {
        None | Some(Value::Null) => 10,
        Some(value) => value
            .as_u64()
            .filter(|limit| (1..=50).contains(limit))
            .map(|limit| limit as usize)
            .ok_or_else(|| {
                ToolFailure::Invalid(
                    "Invalid params: 'limit' must be an integer between 1 and 50".to_string(),
                )
            })?,
    };

    let mode = match args.get("mode") {
        None | Some(Value::Null) => "ranked".to_string(),
        Some(Value::String(mode)) if mode == "ranked" || mode == "exhaustive" => mode.clone(),
        Some(_) => {
            return Err(ToolFailure::Invalid(
                "Invalid params: 'mode' must be \"ranked\" or \"exhaustive\"".to_string(),
            ));
        }
    };

    let path = optional_string(args, "path")?;
    let ext = optional_string(args, "ext")?;
    let modified_after = optional_string(args, "modified_after")?;

    let composed = compose_query(
        query,
        path.as_deref(),
        ext.as_deref(),
        modified_after.as_deref(),
        mode == "exhaustive",
    );

    let index = index.ok_or_else(|| ToolFailure::Execution(NO_INDEX_MESSAGE.to_string()))?;
    let response = index
        .search(&composed, limit)
        .map_err(ToolFailure::Execution)?;

    let results: Vec<Value> = response
        .results
        .iter()
        .map(|result| {
            json!({
                "path": result.path,
                "score": result.score,
                "snippet": result
                    .snippet
                    .as_deref()
                    .map(|snippet| truncate_chars(snippet, 400)),
                "match_type": result.match_type,
                "size": result.file_size,
                "modified": result.modified,
            })
        })
        .collect();

    Ok(json!({
        "results": results,
        "total": response.total,
        "mode": response.mode,
        "as_of": now_unix_secs(),
    }))
}

fn tool_read_snippet(args: &Value) -> Result<Value, ToolFailure> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| ToolFailure::Invalid("Invalid params: 'path' is required".to_string()))?;

    let query = optional_string(args, "query")?;

    let max_tokens = match args.get("max_tokens") {
        None | Some(Value::Null) => 256,
        Some(value) => value
            .as_u64()
            .filter(|tokens| (32..=1024).contains(tokens))
            .map(|tokens| tokens as usize)
            .ok_or_else(|| {
                ToolFailure::Invalid(
                    "Invalid params: 'max_tokens' must be an integer between 32 and 1024"
                        .to_string(),
                )
            })?,
    };

    let file = Path::new(path);
    let text = extract_text(file)
        .ok_or_else(|| ToolFailure::Execution(format!("Could not read text from {}", path)))?;

    let (line_start, line_end, window, truncated) =
        read_snippet_window(&text, query.as_deref(), max_tokens);

    Ok(json!({
        "path": path,
        "line_start": line_start,
        "line_end": line_end,
        "text": window,
        "truncated": truncated,
    }))
}

fn tool_index_state(index: Option<&HeadlessIndex>) -> Result<Value, ToolFailure> {
    let index = index.ok_or_else(|| ToolFailure::Execution(NO_INDEX_MESSAGE.to_string()))?;
    let state = index.state().map_err(ToolFailure::Execution)?;

    Ok(json!({
        "files": state["files"],
        "dirs": state["dirs"],
        "content_docs": state["content_docs"],
        "semantic_units": state["semantic_units"],
        "index_bytes": state["index_bytes"],
        "model_id": state["model_id"],
        "active_model_id": state["active_model_id"],
        "model_name": state["model_name"],
        "roots": state["roots"],
        "as_of": now_unix_secs(),
    }))
}

fn optional_string(args: &Value, key: &str) -> Result<Option<String>, ToolFailure> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ToolFailure::Invalid(format!(
            "Invalid params: '{}' must be a string",
            key
        ))),
    }
}

fn extract_text(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    const DOCUMENT_EXTENSIONS: &[&str] = &[
        "pdf", "doc", "docx", "ppt", "pptx", "xls", "xlsx", "epub", "odt", "ods", "odp", "rtf",
    ];

    if DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
        extract_document(path, 1_048_576).map(|document| document.text)
    } else {
        read_text_content(path, 1_048_576)
    }
}

/// Returns `(line_start, line_end, text, truncated)` for a snippet centered on
/// the first case-insensitive `query` match, capped at `max_tokens * 4` chars.
///
/// Line numbers are 1-based and relative to the full extracted text.
pub fn read_snippet_window(
    text: &str,
    query: Option<&str>,
    max_tokens: usize,
) -> (usize, usize, String, bool) {
    let cap_chars = max_tokens.saturating_mul(4).max(1);

    let match_byte = query
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .and_then(|query| text.to_lowercase().find(&query.to_lowercase()))
        .unwrap_or(0)
        .min(text.len());

    // Keep a little context before the match, then fill the rest of the budget
    // after it.
    let mut start = match_byte.saturating_sub(cap_chars / 4);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }

    let mut end = start;
    for (chars, (offset, ch)) in text[start..].char_indices().enumerate() {
        if chars >= cap_chars {
            break;
        }
        end = start + offset + ch.len_utf8();
    }

    let window = text[start..end].to_string();
    let line_start = text[..start].bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_end = line_start + window.bytes().filter(|byte| *byte == b'\n').count();
    let truncated = start > 0 || end < text.len();

    (line_start, line_end, window, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KensakuConfig;
    use crate::indexer::content::ContentIndex;
    use crate::indexer::metadata_db::MetadataDb;
    use crate::indexer::walker::FileEntry;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        index: Option<HeadlessIndex>,
        dir: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.index.take();
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn fixture(tag: &str) -> Fixture {
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kensaku-mcp-test-{}-{}-{}-{}",
            tag,
            std::process::id(),
            seq,
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("notes.txt");
        let content = "alpha line\nbeta line with needle\nomega line\n";
        fs::write(&file, content).unwrap();
        let modified = fs::metadata(&file)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let config = KensakuConfig {
            watched_dirs: vec![dir.clone()],
            ..KensakuConfig::default()
        };
        config.save_to(&dir).unwrap();

        let db = MetadataDb::open_in_dir(&dir).unwrap();
        db.upsert(&FileEntry {
            id: 1,
            path: file.clone(),
            name: "notes.txt".to_string(),
            extension: Some("txt".to_string()),
            size: content.len() as u64,
            modified,
            is_dir: false,
            dev: 0,
            ino: 0,
            char_bloom: crate::search::fuzzy::compute_char_mask("notes.txt"),
            content_pending: false,
        })
        .unwrap();
        drop(db);

        {
            let index_dir = dir.join("index");
            let content_index = ContentIndex::new(&index_dir).unwrap();
            content_index
                .add_or_update(&file, "notes.txt", content, modified)
                .unwrap();
            content_index.commit().unwrap();
        }

        let index = HeadlessIndex::open_at(&dir).expect("open headless index");
        Fixture {
            index: Some(index),
            dir,
        }
    }

    fn request(value: Value) -> Value {
        value
    }

    #[test]
    fn initialize_reports_protocol_and_server_info() {
        let fixture = fixture("initialize");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"})),
        )
        .expect("initialize must reply");

        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "kensaku");
        assert_eq!(
            response["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn notifications_get_no_reply() {
        let fixture = fixture("notifications");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
        );
        assert!(response.is_none());
    }

    #[test]
    fn tools_list_exposes_three_tools_with_valid_schemas() {
        let fixture = fixture("tools-list");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})),
        )
        .expect("tools/list must reply");

        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["search", "read_snippet", "index_state"]);

        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "tool {:?}", tool["name"]);
            assert!(schema["properties"].is_object());
        }
    }

    #[test]
    fn tools_call_without_index_returns_is_error() {
        let response = handle_request(
            None,
            &request(json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "search", "arguments": {"query": "needle"}}
            })),
        )
        .expect("tools/call must reply");

        assert_eq!(response["result"]["isError"], true);
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("No Kensaku index found"), "got: {text}");
    }

    #[test]
    fn unknown_method_is_method_not_found() {
        let fixture = fixture("unknown-method");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({"jsonrpc": "2.0", "id": 4, "method": "does/not/exist"})),
        )
        .expect("unknown method must reply");

        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn search_tool_returns_results_and_truncates_snippets() {
        let fixture = fixture("search");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {"name": "search", "arguments": {"query": "needle", "limit": 5}}
            })),
        )
        .expect("tools/call must reply");

        assert_eq!(response["result"]["isError"], false);
        let payload: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(payload["results"].as_array().unwrap().len(), 1);
        assert!(
            payload["results"][0]["path"]
                .as_str()
                .unwrap()
                .ends_with("notes.txt")
        );
        assert!(
            payload["results"][0]["snippet"]
                .as_str()
                .unwrap()
                .contains("needle")
        );
        assert_eq!(payload["mode"], "ranked");
        assert!(payload["as_of"].as_u64().unwrap() > 0);
    }

    #[test]
    fn index_state_tool_reports_counts() {
        let fixture = fixture("state");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": {"name": "index_state", "arguments": {}}
            })),
        )
        .expect("tools/call must reply");

        let payload: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(payload["files"], 1);
        assert_eq!(payload["dirs"], 0);
        assert!(payload["index_bytes"].as_u64().unwrap() > 0);
        assert!(payload["as_of"].as_u64().unwrap() > 0);
        assert!(payload["model_id"].is_null());
        assert!(payload["active_model_id"].as_u64().unwrap() > 0);
        assert!(!payload["model_name"].as_str().unwrap().is_empty());
    }

    #[test]
    fn index_state_separates_units_model_from_active_model() {
        let fixture = fixture("model-ids");
        // Simulate vectors written with a different model id than the one the
        // process will embed new queries with.
        {
            let db = MetadataDb::open_in_dir(&fixture.dir).expect("open metadata db");
            db.insert_unit(1, 0, 10, 42, &vec![0i8; crate::embed::EMBEDDING_DIM])
                .expect("insert unit");
        }

        let state = fixture
            .index
            .as_ref()
            .expect("index open")
            .state()
            .expect("state");

        let active = crate::embed::ModelRegistry::global();
        assert_eq!(state["model_id"], 42);
        assert_eq!(state["active_model_id"], active.model_id);
        assert_eq!(state["model_name"], active.name);
    }

    #[test]
    fn invalid_arguments_are_invalid_params() {
        let fixture = fixture("invalid-params");
        let response = handle_request(
            fixture.index.as_ref(),
            &request(json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": {"name": "search", "arguments": {"query": "x", "limit": 0}}
            })),
        )
        .expect("tools/call must reply");

        assert_eq!(response["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn read_snippet_windowing_finds_match_and_reports_lines() {
        let mut text = String::new();
        for i in 1..=100 {
            if i == 50 {
                text.push_str("the NEEDLE is here\n");
            } else {
                text.push_str(&format!("line {}\n", i));
            }
        }

        let (line_start, line_end, window, truncated) =
            read_snippet_window(&text, Some("needle"), 32);

        assert!(window.to_lowercase().contains("needle"));
        assert!(window.chars().count() <= 32 * 4);
        assert!(truncated);
        assert!(
            line_start <= 50 && line_end >= 50,
            "{line_start}..{line_end}"
        );
        assert!(line_start > 1);
    }

    #[test]
    fn read_snippet_without_query_starts_at_file_start() {
        let text = "hello world\nsecond line\n";
        let (line_start, line_end, window, truncated) = read_snippet_window(text, None, 256);
        assert_eq!(line_start, 1);
        assert_eq!(line_end, 3);
        assert_eq!(window, text);
        assert!(!truncated);
    }
}
