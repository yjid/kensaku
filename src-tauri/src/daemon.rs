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
