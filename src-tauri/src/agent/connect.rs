//! One-click agent connect: generate and install Kensaku MCP configs for the
//! common clients.
//!
//! Kensaku is one binary with two surfaces (`kensaku-cli mcp` over stdio, plus
//! the loopback HTTP transport); every client config points at the same
//! read-only server. Installation means a **merge** into the client's own
//! config file — never a rewrite of the whole file — with a `.bak` backup
//! left next to it. The manual JSON fallback stays available in the UI and
//! via `kensaku-cli connect <client> --print`.

use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    ClaudeDesktop,
    ClaudeCode,
    Cursor,
}

impl Client {
    pub const ALL: [Self; 3] = [Self::ClaudeDesktop, Self::ClaudeCode, Self::Cursor];

    pub fn id(&self) -> &'static str {
        match self {
            Self::ClaudeDesktop => "claude-desktop",
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ClaudeDesktop => "Claude Desktop",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
        }
    }

    fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|client| client.id() == id)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientStatus {
    pub id: &'static str,
    pub name: &'static str,
    /// Absolute config path when the client's config dir is locatable.
    pub config_path: Option<String>,
    /// True when the config file already exists on disk.
    pub installed: bool,
    /// True when the file exists and already wires `kensaku`.
    pub configured: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallOutcome {
    pub status: ClientStatus,
    pub wrote: bool,
    pub backup: Option<String>,
    pub message: String,
}

/// Candidate config paths for a client on this OS. The first existing file
/// wins for `installed`/`configured`; the first locatable path is the install
/// target.
fn config_paths(client: Client) -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    match client {
        Client::ClaudeDesktop => {
            let mut paths = Vec::new();
            if let Some(data) = dirs::data_dir() {
                #[cfg(target_os = "macos")]
                paths.push(data.join("Claude/claude_desktop_config.json"));
                #[cfg(target_os = "linux")]
                paths.push(data.join("Claude/claude_desktop_config.json"));
            }
            if let Some(config) = dirs::config_dir() {
                #[cfg(target_os = "windows")]
                paths.push(config.join("Claude/claude_desktop_config.json"));
                #[cfg(not(target_os = "windows"))]
                let _ = config;
            }
            // Documented Windows location, resolved manually because
            // `dirs::data_dir` is not Roaming AppData on Windows.
            #[cfg(target_os = "windows")]
            if let Some(roaming) = std::env::var_os("APPDATA") {
                paths.push(PathBuf::from(roaming).join("Claude/claude_desktop_config.json"));
            }
            paths.push(home.join(".claude/claude_desktop_config.json"));
            paths
        }
        Client::ClaudeCode => vec![home.join(".claude.json")],
        Client::Cursor => vec![dirs::home_dir().unwrap_or(home).join(".cursor/mcp.json")],
    }
}

fn primary_path(client: Client) -> Option<PathBuf> {
    config_paths(client).into_iter().next()
}

/// The MCP server entry for Kensaku, with the best absolute path we can find
/// for `kensaku-cli`.
pub fn server_entry() -> Value {
    json!({
        "command": cli_command(),
        "args": ["mcp"],
    })
}

/// Best-effort absolute path to the CLI binary shipped next to this process.
pub fn cli_command() -> String {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("kensaku-cli"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".cargo/bin/kensaku-cli"));
        candidates.push(home.join(".local/bin/kensaku-cli"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("kensaku-cli"));
        }
    }
    candidates.push(PathBuf::from("kensaku-cli"));
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kensaku-cli".to_string())
}

/// Status for every supported client (locatable config path, installed,
/// configured). Never fails: unlocatable clients report `None`.
pub fn detect_clients() -> Vec<ClientStatus> {
    Client::ALL
        .iter()
        .map(|client| {
            let path = primary_path(*client);
            let installed = path
                .as_ref()
                .map(|path| path.is_file())
                .unwrap_or(false);
            let configured = installed
                && path
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .map(|raw| is_configured(&raw))
                    .unwrap_or(false);
            ClientStatus {
                id: client.id(),
                name: client.name(),
                config_path: path.map(|path| path.to_string_lossy().into_owned()),
                installed,
                configured,
            }
        })
        .collect()
}

fn is_configured(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("mcpServers")
                .cloned()
                .or_else(|| value.get("projects").cloned().map(|_| Value::Null))
                .map(|servers| {
                    servers
                        .get("kensaku")
                        .map(|entry| entry.get("command").is_some())
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false)
}

/// The ready-to-paste JSON snippet for a client's config file.
pub fn config_snippet(client: Client) -> Result<String, String> {
    let pretty = serde_json::to_string_pretty(&json!({
        "mcpServers": {
            "kensaku": server_entry(),
        }
    }))
    .map_err(|e| e.to_string())?;
    let _ = client;
    Ok(pretty)
}

/// Merges the Kensaku entry into the client's config file.
///
/// Creates missing parent dirs and the file itself, backs up any previous
/// content to `<path>.bak`, and never removes unrelated entries. Returns the
/// post-install status.
pub fn install(client: Client) -> Result<InstallOutcome, String> {
    let Some(path) = primary_path(client) else {
        return Err(format!(
            "could not locate the {} config directory on this machine",
            client.name()
        ));
    };

    let previous = if path.is_file() {
        Some(std::fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?)
    } else {
        None
    };

    let mut root = match &previous {
        Some(raw) => serde_json::from_str::<Value>(raw)
            .map_err(|e| format!("{} is not valid JSON: {}", path.display(), e))?,
        None => json!({}),
    };
    if !root.is_object() {
        return Err(format!("{} is not a JSON object", path.display()));
    }

    let servers = root
        .as_object_mut()
        .expect("checked object")
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        return Err(format!(
            "{} has a non-object \"mcpServers\" key; merge manually",
            path.display()
        ));
    }
    servers
        .as_object_mut()
        .expect("checked object")
        .insert("kensaku".to_string(), server_entry());

    let merged = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {}", e))?;
    }
    let mut backup = None;
    if let Some(previous) = &previous {
        let backup_path = path.with_extension("json.bak");
        std::fs::write(&backup_path, previous)
            .map_err(|e| format!("write backup: {}", e))?;
        backup = Some(backup_path.to_string_lossy().into_owned());
    }
    std::fs::write(&path, &merged).map_err(|e| format!("write {}: {}", path.display(), e))?;

    let status = ClientStatus {
        id: client.id(),
        name: client.name(),
        config_path: Some(path.to_string_lossy().into_owned()),
        installed: true,
        configured: true,
    };
    Ok(InstallOutcome {
        status,
        wrote: true,
        backup,
        message: format!(
            "Kensaku MCP added to {}. Restart {} to pick it up.",
            path.display(),
            client.name()
        ),
    })
}

/// CLI handler: `connect <client> [--write]`. Default prints the snippet.
pub fn run_cli(args: &[String]) -> i32 {
    let write = args.iter().any(|arg| arg == "--write");
    let client = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .and_then(|arg| Client::parse(arg));
    let Some(client) = client else {
        eprintln!(
            "kensaku-cli: connect requires one of: {}",
            Client::ALL
                .iter()
                .map(|client| client.id())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return crate::cli::EXIT_USAGE;
    };

    if write {
        match install(client) {
            Ok(outcome) => {
                println!("{}", outcome.message);
                if let Some(backup) = outcome.backup {
                    println!("backup: {}", backup);
                }
                crate::cli::EXIT_OK
            }
            Err(message) => {
                eprintln!("kensaku-cli: {}", message);
                crate::cli::EXIT_FAILURE
            }
        }
    } else {
        match config_snippet(client) {
            Ok(snippet) => {
                println!("{}", snippet);
                println!();
                println!(
                    "# paste into {} (or run with --write to install)",
                    primary_path(client)
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_else(|| client.name().to_string())
                );
                crate::cli::EXIT_OK
            }
            Err(message) => {
                eprintln!("kensaku-cli: {}", message);
                crate::cli::EXIT_FAILURE
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_well_formed_and_local() {
        let statuses = detect_clients();
        assert_eq!(statuses.len(), 3);
        for status in &statuses {
            assert!(!status.name.is_empty());
            if status.configured {
                assert!(status.installed, "configured implies installed");
            }
        }
    }

    #[test]
    fn snippet_is_valid_json_with_kensaku_entry() {
        let snippet = config_snippet(Client::Cursor).unwrap();
        let value: Value = serde_json::from_str(&snippet).unwrap();
        assert_eq!(value["mcpServers"]["kensaku"]["args"][0], "mcp");
        assert!(!value["mcpServers"]["kensaku"]["command"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn is_configured_matches_only_real_entries() {
        assert!(!is_configured("{}"));
        assert!(!is_configured("not json"));
        assert!(is_configured(
            r#"{"mcpServers": {"kensaku": {"command": "kensaku-cli", "args": ["mcp"]}}}"#
        ));
        assert!(!is_configured(
            r#"{"mcpServers": {"other": {"command": "x"}}}"#
        ));
    }

    #[test]
    fn cli_connect_rejects_unknown_clients() {
        assert_eq!(run_cli(&["nope".to_string()]), crate::cli::EXIT_USAGE);
        assert_eq!(run_cli(&[]), crate::cli::EXIT_USAGE);
    }
}
