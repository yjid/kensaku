use kensaku_lib::agent::HeadlessIndex;
use kensaku_lib::config::KensakuConfig;
use kensaku_lib::indexer::IndexerManager;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn socket_path(config_dir: &Path) -> PathBuf {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        Path::new(&runtime).join("kensaku.sock")
    } else {
        config_dir.join("kensaku.sock")
    }
}

fn token_path(config_dir: &Path) -> PathBuf {
    config_dir.join("daemon.token")
}

fn ensure_token(config_dir: &Path) -> std::io::Result<String> {
    let path = token_path(config_dir);
    if let Ok(existing) = fs::read_to_string(&path) {
        let t = existing.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    // Generate a simple token from random-ish bytes using process id + time.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("{:x}-{:x}", std::process::id(), nanos);
    fs::write(&path, &token)?;
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    Ok(token)
}

fn main() {
    let config_dir = KensakuConfig::config_dir();
    let _ = fs::create_dir_all(&config_dir);
    let token = match ensure_token(&config_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kensaku-daemon: token: {}", e);
            std::process::exit(1);
        }
    };

    // Single instance lock
    let lock_path = config_dir.join("kensaku.lock");
    let lock_file = match fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("kensaku-daemon: lock: {}", e);
            std::process::exit(1);
        }
    };
    use fs4::fs_std::FileExt;
    match lock_file.try_lock_exclusive() {
        Ok(true) => {}
        _ => {
            eprintln!("kensaku-daemon: another daemon is running; exiting");
            std::process::exit(0);
        }
    }

    let sock = socket_path(&config_dir);
    let _ = fs::remove_file(&sock);
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kensaku-daemon: bind: {}", e);
            std::process::exit(1);
        }
    };
    let _ = fs::set_permissions(&sock, fs::Permissions::from_mode(0o600));

    let pid_path = config_dir.join("daemon.pid");
    let _ = fs::write(&pid_path, std::process::id().to_string());

    // Start indexer
    let cfg = KensakuConfig::load();
    let manager = Arc::new(IndexerManager::new(cfg));
    manager.start_indexing();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let token = token.clone();
                let manager = Arc::clone(&manager);
                std::thread::spawn(move || handle_client(stream, &token, &manager));
            }
            Err(_) => continue,
        }
    }
}

fn handle_client(mut stream: UnixStream, token: &str, _manager: &Arc<IndexerManager>) {
    let reader = stream.try_clone().unwrap();
    let buf = BufReader::new(reader);
    for line in buf.lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let _ = writeln!(stream, "{}", json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}));
                continue;
            }
        };
        // Handshake/auth: require params.token
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Token check
        let provided = params.get("token").and_then(|t| t.as_str()).unwrap_or("");
        if provided != token {
            let _ = writeln!(stream, "{}", json!({"jsonrpc":"2.0","id":id,"error":{"code":-32001,"message":"bad token"}}));
            continue;
        }

        let protocol_version = params.get("protocol_version").and_then(|v| v.as_u64()).unwrap_or(0);
        if protocol_version != 1 && method != "health" {
            // allow missing for now
        }

        let result = match method {
            "health" => Ok(json!({"ok": true})),
            "index_state" => match HeadlessIndex::open() {
                Ok(idx) => idx.state_local(),
                Err(e) => Err(e),
            },
            "shutdown" => {
                std::process::exit(0);
            }
            _ => Err(format!("unknown method {}", method)),
        };

        let resp = match result {
            Ok(r) => json!({"jsonrpc":"2.0","id":id,"result":r}),
            Err(e) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":e}}),
        };
        let _ = writeln!(stream, "{}", resp);
    }
}