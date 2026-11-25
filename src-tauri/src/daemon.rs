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

/// Unix socket paths to try, most specific first.
///
/// The daemon listens on `$XDG_RUNTIME_DIR/kensaku.sock` when that variable is
/// set, otherwise under the config dir. A systemd *system* unit does not always
/// export `XDG_RUNTIME_DIR`, while an interactive shell does, so the two sides
/// used to disagree about where the socket was even when both were healthy:
/// the CLI decided no daemon was running and opened the index directly.
/// Connecting to each candidate in turn costs nothing and makes the transport
/// independent of which context each process was started from.
fn socket_candidates(
    runtime: Option<&std::path::Path>,
    config_dir: &std::path::Path,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(runtime) = runtime {
        paths.push(runtime.join("kensaku.sock"));
    }
    let fallback = config_dir.join("kensaku.sock");
    if !paths.contains(&fallback) {
        paths.push(fallback);
    }
    paths
}

pub fn socket_paths() -> Vec<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    socket_candidates(runtime.as_deref(), &KensakuConfig::config_dir())
}

/// The preferred socket path (see [`socket_paths`]).
pub fn socket_path() -> PathBuf {
    socket_paths()
        .into_iter()
        .next()
        .unwrap_or_else(|| KensakuConfig::config_dir().join("kensaku.sock"))
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

pub fn request(method: &str, mut params: Value) -> Result<Value, String> {
    use interprocess::local_socket::prelude::*;
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

    if std::env::var_os("KENSAKU_NO_DAEMON").is_some() {
        return Err("daemon disabled by KENSAKU_NO_DAEMON".to_string());
    }
    let token = read_token().ok_or_else(|| "daemon token missing".to_string())?;
    let mut stream = None;
    let mut last_error = String::from("no candidate socket");
    for path in socket_paths() {
        #[cfg(unix)]
        let name = {
            use interprocess::local_socket::{GenericFilePath, ToFsName};
            match path.clone().to_fs_name::<GenericFilePath>() {
                Ok(name) => name,
                Err(e) => {
                    last_error = format!("daemon socket name: {}", e);
                    continue;
                }
            }
        };
        #[cfg(windows)]
        let name = {
            use interprocess::local_socket::{GenericNamespaced, ToNsName};
            match "kensaku".to_ns_name::<GenericNamespaced>() {
                Ok(name) => name,
                Err(e) => {
                    last_error = format!("daemon socket name: {}", e);
                    continue;
                }
            }
        };
        match LocalSocketStream::connect(name) {
            Ok(connected) => {
                stream = Some(connected);
                break;
            }
            Err(e) => last_error = format!("daemon unavailable at {}: {}", path.display(), e),
        }
    }
    let stream = stream.ok_or(last_error)?;
    stream
        .set_recv_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(&stream);
    let mut writer = &stream;

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

    #[test]
    fn socket_candidates_prefer_runtime_and_fall_back_to_config_dir() {
        let runtime = PathBuf::from("/run/user/1000");
        let config = PathBuf::from("/home/u/.config/kensaku");
        assert_eq!(
            socket_candidates(Some(&runtime), &config),
            vec![runtime.join("kensaku.sock"), config.join("kensaku.sock")]
        );
        assert_eq!(
            socket_candidates(None, &config),
            vec![config.join("kensaku.sock")]
        );
        // When both resolve to the same directory the path is tried once.
        assert_eq!(
            socket_candidates(Some(&config), &config),
            vec![config.join("kensaku.sock")]
        );
    }

    #[cfg(unix)]
    #[test]
    fn macos_does_not_spawn_a_helper_daemon() {
        assert!(!daemon_spawn_allowed_on("macos"));
        assert!(daemon_spawn_allowed_on("linux"));
    }

    #[cfg(unix)]
    #[test]
    fn no_daemon_env_is_honoured_and_empty_means_unset() {
        assert!(!no_daemon_requested(None));
        assert!(!no_daemon_requested(Some(std::ffi::OsStr::new(""))));
        assert!(no_daemon_requested(Some(std::ffi::OsStr::new("1"))));
    }
}

/// Whether the GUI may spawn its own helper daemon for `os`.
///
/// macOS keeps indexing in the app process: the GUI received the TCC folder
/// grant from the native picker, and a separately launched helper has a
/// different process identity, so macOS can prompt again for the same folders.
/// Linux keeps the warm-sidecar path.
#[cfg(unix)]
fn daemon_spawn_allowed_on(os: &str) -> bool {
    os != "macos"
}

#[cfg(unix)]
fn daemon_spawn_allowed() -> bool {
    daemon_spawn_allowed_on(std::env::consts::OS)
}

/// True when the environment explicitly asks the app not to start a daemon.
///
/// An empty value counts as unset: shells and desktop files often carry
/// `VAR=` for a variable they mean to clear.
#[cfg(unix)]
fn no_daemon_requested(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

#[derive(Clone, Copy)]
pub struct DaemonClient;

impl DaemonClient {
    #[cfg(unix)]
    pub fn ensure() -> bool {
        // An already-running daemon is always preferred: it owns the index and
        // gives the GUI/CLI/MCP the warm path. On macOS we only ever *use* such
        // a daemon; we never spawn one.
        if request("ping", serde_json::json!({})).is_ok() {
            return true;
        }
        // `KENSAKU_NO_DAEMON` means "do not start one", not "ignore a running
        // one": the existing daemon already owns the index. Before this check
        // the GUI spawned a daemon anyway and then ran a second writable
        // indexer in-process.
        if no_daemon_requested(std::env::var_os("KENSAKU_NO_DAEMON").as_deref()) {
            return false;
        }
        if !daemon_spawn_allowed() {
            return false;
        }

        #[cfg(not(target_os = "macos"))]
        {
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

    /// Keyset-paginated exhaustive search past the 10k cap (agent surfaces).
    pub fn search_page_after(
        &self,
        query: String,
        limit: usize,
        start_after: &str,
    ) -> Result<crate::search::SearchResponse, String> {
        let value = request(
            "search",
            serde_json::json!({
                "query": query,
                "limit": limit,
                "offset": 0,
                "snippets": false,
                "start_after": start_after,
                "mode": "exhaustive",
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

    pub fn find_similar_images(
        &self,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Vec<crate::embed::visual::SimilarImageResult>, String> {
        let value = request(
            "find_similar_images",
            serde_json::json!({
                "path": path,
                "limit": limit.unwrap_or(20),
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
        request("save_config", serde_json::json!({ "config": config }))?;
        Ok(())
    }

    pub fn reindex(&self) -> Result<(), String> {
        request("reindex", serde_json::json!({}))?;
        Ok(())
    }

    /// Re-scans the watched folders. GUI-initiated: unlike the MCP `reindex`
    /// tool this needs no `mcp_allow_reindex` opt-in — the request comes from
    /// the app's own UI, not from an agent.
    pub fn rescan(&self) -> Result<(), String> {
        request("rescan", serde_json::json!({}))?;
        Ok(())
    }

    /// Stops the daemon's first-run crawl and returns setup to folder selection.
    pub fn cancel_setup_scan(&self) -> Result<(), String> {
        request("cancel_setup_scan", serde_json::json!({}))?;
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
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
