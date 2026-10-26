//! Minimal client for the local `kensaku-daemon`.
//!
//! The daemon speaks newline-delimited JSON-RPC 2.0 over a Unix domain socket.
//! Every request carries the token stored in `<config>/kensaku/daemon.token`.
//! When the daemon is absent, callers fall back to the in-process read-only
//! index (CLI/MCP) or the in-process indexer (Tauri app).

use crate::config::KensakuConfig;
use serde_json::{Value, json};
use std::path::PathBuf;

pub const PROTOCOL_VERSION: u64 = 1;

pub fn socket_path() -> PathBuf {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime).join("kensaku.sock")
    } else {
        KensakuConfig::config_dir().join("kensaku.sock")
    }
}

pub fn token_path() -> PathBuf {
    KensakuConfig::config_dir().join("daemon.token")
}

pub fn read_token() -> Option<String> {
    std::fs::read_to_string(token_path())
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(unix)]
pub fn request(method: &str, mut params: Value) -> Result<Value, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    if std::env::var_os("KENSAKU_NO_DAEMON").is_some() {
        return Err("daemon disabled by KENSAKU_NO_DAEMON".to_string());
    }
    let token = read_token().ok_or_else(|| "daemon token missing".to_string())?;
    let stream = UnixStream::connect(socket_path())
        .map_err(|e| format!("daemon unavailable: {}", e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut writer = stream;

    if let Value::Object(map) = &mut params {
        map.insert("token".into(), Value::String(token));
        map.insert("protocol_version".into(), json!(PROTOCOL_VERSION));
    }
    let req = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    writeln!(writer, "{}", req).map_err(|e| e.to_string())?;

    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if let Some(err) = response.get("error") {
        return Err(err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("daemon error")
            .to_string());
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(not(unix))]
pub fn request(_method: &str, _params: Value) -> Result<Value, String> {
    Err("daemon client is only implemented on Unix".to_string())
}

pub fn health() -> bool {
    request("health", json!({})).is_ok()
}

/// Validates the token and protocol version carried by an IPC request.
pub fn authenticate(
    expected_token: &str,
    provided_token: Option<&str>,
    protocol_version: Option<u64>,
) -> Result<(), (i32, String)> {
    if provided_token != Some(expected_token) {
        return Err((-32001, "bad token".to_string()));
    }
    if protocol_version != Some(PROTOCOL_VERSION) {
        return Err((
            -32002,
            format!(
                "protocol version mismatch: expected {}, got {:?}",
                PROTOCOL_VERSION, protocol_version
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_token() {
        let err = authenticate("good", Some("bad"), Some(PROTOCOL_VERSION)).unwrap_err();
        assert_eq!(err.0, -32001);
    }

    #[test]
    fn rejects_version_mismatch() {
        let err = authenticate("good", Some("good"), Some(999)).unwrap_err();
        assert_eq!(err.0, -32002);
    }

    #[test]
    fn accepts_matching_handshake() {
        assert!(authenticate("good", Some("good"), Some(PROTOCOL_VERSION)).is_ok());
    }
}

#[derive(Clone, Copy)]
pub struct DaemonClient;

impl DaemonClient {
    #[cfg(unix)]
    pub fn ensure() -> bool {
        if request("ping", serde_json::json!({})).is_ok() {
            return true;
        }
        let Some(exe) = daemon_binary_path() else {
            return false;
        };
        let mut command = std::process::Command::new(exe);
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if command.spawn().is_err() {
            return false;
        }
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if request("ping", serde_json::json!({})).is_ok() {
                return true;
            }
        }
        false
    }

    #[cfg(not(unix))]
    pub fn ensure() -> bool {
        false
    }

    pub fn search_page(
        &self,
        query: String,
        limit: usize,
        offset: usize,
        snippets: bool,
    ) -> Result<crate::search::SearchResponse, String> {
        let value = request(
            "search",
            serde_json::json!({
                "query": query,
                "limit": limit,
                "offset": offset,
                "snippets": snippets,
            }),
        )?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn get_snippets(
        &self,
        paths: Vec<String>,
        query: &str,
    ) -> Result<Vec<crate::commands::SnippetDto>, String> {
        let value = request(
            "get_snippets",
            serde_json::json!({
                "paths": paths,
                "query": query,
            }),
        )?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn read_snippet(
        &self,
        path: &str,
        query: Option<&str>,
        max_tokens: usize,
    ) -> Result<Value, String> {
        request(
            "read_snippet",
            serde_json::json!({
                "path": path,
                "query": query,
                "max_tokens": max_tokens,
            }),
        )
    }

    pub fn get_stats(&self) -> Result<Value, String> {
        request("get_stats", serde_json::json!({}))
    }

    pub fn get_home_data(&self) -> Result<crate::commands::HomeData, String> {
        let value = request("get_home_data", serde_json::json!({}))?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn toggle_pin(&self, path: String) -> Result<bool, String> {
        let value = request("toggle_pin", serde_json::json!({ "path": path }))?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn get_scope_suggestions(
        &self,
        prefix: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, String> {
        let value = request(
            "get_scope_suggestions",
            serde_json::json!({
                "prefix": prefix,
                "limit": limit,
            }),
        )?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn find_duplicates(
        &self,
        max_distance: Option<u32>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::commands::DuplicateGroup>, String> {
        let value = request(
            "find_duplicates",
            serde_json::json!({
                "max_distance": max_distance,
                "limit": limit,
            }),
        )?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn get_status(&self) -> Result<crate::indexer::IndexingStatus, String> {
        let value = request("get_status", serde_json::json!({}))?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn get_config(&self) -> Result<KensakuConfig, String> {
        let value = request("get_config", serde_json::json!({}))?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn save_config(&self, config: KensakuConfig) -> Result<(), String> {
        request(
            "save_config",
            serde_json::json!({ "config": config }),
        )?;
        Ok(())
    }

    pub fn reindex(&self) -> Result<(), String> {
        request("reindex", serde_json::json!({}))?;
        Ok(())
    }
}

#[cfg(unix)]
fn daemon_binary_path() -> Option<std::path::PathBuf> {
    let name = "kensaku-daemon";
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(name));
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            candidates.push(std::path::PathBuf::from(dir).join(name));
        }
    }
    candidates.push(std::path::PathBuf::from("src-tauri/target/debug").join(name));
    candidates.push(std::path::PathBuf::from("src-tauri/target/release").join(name));
    candidates.push(std::path::PathBuf::from("cli/target/debug").join(name));
    candidates.into_iter().find(|path| path.is_file())
}
