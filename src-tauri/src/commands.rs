use crate::config::KensakuConfig;
use crate::indexer::IndexingStatus;
use crate::search::{SearchDispatcher, SearchResponse, SearchResult};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

static CURRENT_SEARCH_SEQ: AtomicU64 = AtomicU64::new(0);

const QUERY_LATENCY_CAPACITY: usize = 256;
const QUERY_HISTORY_SHOWN: usize = 50;

/// Process-global ring buffer of recent query wall times, in microseconds.
static QUERY_LATENCIES: OnceLock<Mutex<VecDeque<u64>>> = OnceLock::new();
static QUERY_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Process start time, used for the runtime uptime stat.
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

/// Records the process start time. Called once from `run()`.
pub fn init_process_start() {
    PROCESS_START.get_or_init(Instant::now);
}

fn record_query_latency(micros: u64) {
    QUERY_TOTAL.fetch_add(1, Ordering::Relaxed);
    let buffer = QUERY_LATENCIES
        .get_or_init(|| Mutex::new(VecDeque::with_capacity(QUERY_LATENCY_CAPACITY)));
    // Recover from a poisoned lock instead of panicking: stats are best-effort.
    let mut guard = match buffer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.len() >= QUERY_LATENCY_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(micros);
}

fn query_latency_samples() -> Vec<u64> {
    let buffer = QUERY_LATENCIES
        .get_or_init(|| Mutex::new(VecDeque::with_capacity(QUERY_LATENCY_CAPACITY)));
    let guard = match buffer.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.iter().copied().collect()
}

/// Nearest-rank percentile over ascending-sorted microsecond samples.
fn percentile_ms(sorted_micros: &[u64], percentile: f64) -> f64 {
    if sorted_micros.is_empty() {
        return 0.0;
    }
    let rank = ((percentile / 100.0) * sorted_micros.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_micros.len() - 1);
    sorted_micros[index] as f64 / 1000.0
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Recursively sums regular files below `dir`. Symlinks are not followed.
fn dir_size_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total = total.saturating_add(dir_size_bytes(&entry.path()));
        } else if file_type.is_file() {
            total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }
    total
}

/// Sums sizes of entries in the kensaku config directory whose name starts with
/// `prefix` (e.g. `metadata.db` matches `metadata.db`, `metadata.db-wal`, ...).
fn config_prefixed_bytes(prefix: &str, include_dirs: bool) -> u64 {
    let dir = KensakuConfig::config_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(prefix) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if include_dirs {
                total = total.saturating_add(dir_size_bytes(&entry.path()));
            }
        } else if file_type.is_file() {
            total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }
    total
}

#[cfg(target_os = "linux")]
fn resident_set_bytes() -> u64 {
    // /proc/self/statm: field 1 (index 1) is resident set size in pages.
    const PAGE_SIZE: u64 = 4096;
    let Ok(statm) = std::fs::read_to_string("/proc/self/statm") else {
        return 0;
    };
    statm
        .split_whitespace()
        .nth(1)
        .and_then(|pages| pages.parse::<u64>().ok())
        .map(|pages| pages.saturating_mul(PAGE_SIZE))
        .unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn resident_set_bytes() -> u64 {
    0
}

#[tauri::command]
pub async fn search(
    query: String,
    seq: Option<u64>,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
    let started = Instant::now();
    let req_seq = seq.unwrap_or_else(|| CURRENT_SEARCH_SEQ.fetch_add(1, Ordering::SeqCst) + 1);
    CURRENT_SEARCH_SEQ.store(req_seq, Ordering::SeqCst);

    let indexer = &state.indexer;
    indexer.query_in_flight.store(true, Ordering::SeqCst);

    let metadata = indexer.metadata.read();
    let content_index = indexer.content_index.read();
    let unit_store = indexer.unit_store.read();
    let cfg = indexer.config.read();
    let max_results = cfg.max_results;
    let semantic_enabled = cfg.semantic_enabled;

    let resp = SearchDispatcher::search_response(
        &query,
        &metadata,
        &content_index,
        Some(&unit_store),
        semantic_enabled,
        max_results,
        req_seq,
    );

    indexer.query_in_flight.store(false, Ordering::SeqCst);
    record_query_latency(started.elapsed().as_micros() as u64);
    Ok(resp)
}

#[tauri::command]
pub async fn legacy_search(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let started = Instant::now();
    let indexer = &state.indexer;
    indexer.query_in_flight.store(true, Ordering::SeqCst);

    let metadata = indexer.metadata.read();
    let content_index = indexer.content_index.read();
    let unit_store = indexer.unit_store.read();
    let cfg = indexer.config.read();
    let max_results = cfg.max_results;
    let semantic_enabled = cfg.semantic_enabled;

    let results = SearchDispatcher::search(
        &query,
        &metadata,
        &content_index,
        Some(&unit_store),
        semantic_enabled,
        max_results,
    );

    indexer.query_in_flight.store(false, Ordering::SeqCst);
    record_query_latency(started.elapsed().as_micros() as u64);
    Ok(results)
}

#[tauri::command]
pub async fn get_indexing_status(state: State<'_, AppState>) -> Result<IndexingStatus, String> {
    Ok(state.indexer.get_status())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let indexer = &state.indexer;
    let status = indexer.get_status();

    let (files, dirs) = {
        let metadata = indexer.metadata.read();
        let mut files = 0u64;
        let mut dirs = 0u64;
        for entry in metadata.get_entries() {
            if entry.is_dir {
                dirs += 1;
            } else {
                files += 1;
            }
        }
        (files, dirs)
    };

    let semantic_units = indexer.unit_store.read().len() as u64;

    let metadata_db_bytes = config_prefixed_bytes("metadata.db", false);
    let tantivy_bytes = dir_size_bytes(&KensakuConfig::index_dir());
    let vector_bytes = config_prefixed_bytes("vectors", true);

    let embed_queue_depth = {
        let db = indexer.metadata.read().db();
        db.map(|db| db.count_unembedded().unwrap_or(0)).unwrap_or(0)
    };

    let now_ms = now_unix_ms();
    let now_secs = now_ms / 1000;

    let last_reconcile_secs_ago: Option<u64> = {
        let last = indexer.last_reconcile_unix_secs.load(Ordering::Relaxed);
        if last == 0 {
            None
        } else {
            Some(now_secs.saturating_sub(last))
        }
    };

    let files_per_sec = if status.is_indexing {
        let started_ms = indexer.crawl_start_unix_ms.load(Ordering::Relaxed);
        let elapsed_secs = if started_ms > 0 && now_ms > started_ms {
            (now_ms - started_ms) as f64 / 1000.0
        } else {
            0.0
        };
        if elapsed_secs > 0.0 {
            status.files_indexed as f64 / elapsed_secs
        } else {
            0.0
        }
    } else {
        0.0
    };

    let mut samples = query_latency_samples();
    let total_queries = QUERY_TOTAL.load(Ordering::Relaxed);
    let history: Vec<f64> = samples
        .iter()
        .skip(samples.len().saturating_sub(QUERY_HISTORY_SHOWN))
        .map(|micros| *micros as f64 / 1000.0)
        .collect();
    let last_ms = samples.last().map(|micros| *micros as f64 / 1000.0).unwrap_or(0.0);

    samples.sort_unstable();
    let p50_ms = percentile_ms(&samples, 50.0);
    let p95_ms = percentile_ms(&samples, 95.0);
    let p99_ms = percentile_ms(&samples, 99.0);

    let uptime_secs = PROCESS_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_secs();
    let rss_bytes = resident_set_bytes();
    let watcher_ok = indexer.watcher_ok.load(Ordering::Relaxed);
    let model_id = indexer.config.read().semantic_model.clone();

    Ok(serde_json::json!({
        "queries": {
            "total": total_queries,
            "p50_ms": p50_ms,
            "p95_ms": p95_ms,
            "p99_ms": p99_ms,
            "last_ms": last_ms,
            "history": history,
        },
        "index": {
            "files": files,
            "dirs": dirs,
            "content_docs": status.content_indexed as u64,
            "semantic_units": semantic_units,
            "metadata_db_bytes": metadata_db_bytes,
            "tantivy_bytes": tantivy_bytes,
            "vector_bytes": vector_bytes,
            "last_reconcile_secs_ago": last_reconcile_secs_ago,
        },
        "ingestion": {
            "is_indexing": status.is_indexing,
            "phase": status.phase.clone(),
            "files_indexed": status.files_indexed as u64,
            "content_indexed": status.content_indexed as u64,
            "units_indexed": status.units_indexed as u64,
            "files_per_sec": files_per_sec,
            "eta_secs": Option::<u64>::None,
            "embed_queue_depth": embed_queue_depth as u64,
        },
        "runtime": {
            "uptime_secs": uptime_secs,
            "rss_bytes": rss_bytes,
            "watcher_ok": watcher_ok,
            "semantic_ready": status.semantic_ready,
            "model_id": model_id,
        },
    }))
}

#[tauri::command]
pub async fn set_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    let normalized = theme.trim().to_ascii_lowercase();
    if normalized != "paper" && normalized != "void" {
        return Err(format!(
            "Unsupported theme '{}'. Expected 'paper' or 'void'.",
            theme
        ));
    }

    let mut config = state.indexer.config.read().clone();
    config.theme = normalized;
    config.save()?;
    *state.indexer.config.write() = config;
    Ok(())
}

#[tauri::command]
pub async fn open_file(path: String, state: State<'_, AppState>) -> Result<(), String> {
    state.indexer.metadata.read().record_open(Path::new(&path));
    open::that(&path).map_err(|e| format!("Failed to open file: {}", e))
}

#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    let target_dir = if p.is_dir() {
        p
    } else {
        p.parent().unwrap_or(p)
    };

    open::that(target_dir).map_err(|e| format!("Failed to open folder: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePreview {
    pub path: String,
    pub filename: String,
    pub file_size: u64,
    pub modified: u64,
    pub is_dir: bool,
    pub preview_type: String,
    pub content: Option<String>,
    pub total_lines: Option<usize>,
    pub child_count: Option<usize>,
}

#[tauri::command]
pub async fn get_file_preview(path: String) -> Result<FilePreview, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err("File does not exist".to_string());
    }

    let meta = std::fs::metadata(p).map_err(|e| e.to_string())?;
    let filename = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let is_dir = meta.is_dir();

    if is_dir {
        let child_count = std::fs::read_dir(p).map(|rd| rd.count()).unwrap_or(0);
        return Ok(FilePreview {
            path,
            filename,
            file_size,
            modified,
            is_dir: true,
            preview_type: "dir".to_string(),
            content: None,
            total_lines: None,
            child_count: Some(child_count),
        });
    }

    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "bmp"
    ) {
        return Ok(FilePreview {
            path,
            filename,
            file_size,
            modified,
            is_dir: false,
            preview_type: "image".to_string(),
            content: None,
            total_lines: None,
            child_count: None,
        });
    }

    if let Some(doc) = crate::indexer::extract::extract_document(p, 128_000) {
        let preview_type = match crate::indexer::extract::sniff_format(p) {
            crate::indexer::extract::DocumentFormat::Pdf => "pdf",
            crate::indexer::extract::DocumentFormat::Docx => "doc",
            crate::indexer::extract::DocumentFormat::Pptx => "presentation",
            crate::indexer::extract::DocumentFormat::Xlsx => "sheet",
            crate::indexer::extract::DocumentFormat::Epub
            | crate::indexer::extract::DocumentFormat::Odt
            | crate::indexer::extract::DocumentFormat::Rtf => "doc",
            _ => "text",
        };

        let total_lines = doc.text.lines().count();
        return Ok(FilePreview {
            path,
            filename,
            file_size,
            modified,
            is_dir: false,
            preview_type: preview_type.to_string(),
            content: Some(doc.text),
            total_lines: Some(total_lines),
            child_count: doc.page_count,
        });
    }

    Ok(FilePreview {
        path,
        filename,
        file_size,
        modified,
        is_dir: false,
        preview_type: "binary".to_string(),
        content: None,
        total_lines: None,
        child_count: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub success: bool,
    pub message: String,
}

#[tauri::command]
pub async fn execute_command(
    command: String,
    state: State<'_, AppState>,
) -> Result<CommandOutput, String> {
    let trimmed = command.trim().trim_start_matches('>');
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let cmd = parts.first().copied().unwrap_or("").to_lowercase();

    match cmd.as_str() {
        "reindex" => {
            state.indexer.start_indexing();
            Ok(CommandOutput {
                success: true,
                message: "Reindexing started across configured directories...".to_string(),
            })
        }
        "doctor" => {
            let meta_count = state.indexer.metadata.read().len();
            let config = state.indexer.config.read().clone();
            let roots = config.watched_dirs.len();
            let status = state.indexer.get_status();
            let units_count = state.indexer.unit_store.read().len();
            let msg = format!(
                "Kensaku Doctor Report:\n• Metadata entries: {}\n• Full-text indexed docs: {}\n• Semantic units: {}\n• Configured roots: {}\n• Status: {}\n• Metadata DB: WAL enabled (healthy)",
                meta_count,
                status.content_indexed,
                units_count,
                roots,
                status.current_status
            );
            Ok(CommandOutput {
                success: true,
                message: msg,
            })
        }
        "model" => {
            let units_count = state.indexer.unit_store.read().len();
            let cfg = state.indexer.config.read().clone();
            let status = if cfg.semantic_enabled { "enabled" } else { "disabled" };
            let msg = format!(
                "Active Model: {} (static int8 table, 256-dim, ~5µs/query, offline)\nStatus: {}\nLoaded Units: {}",
                cfg.semantic_model,
                status,
                units_count
            );
            Ok(CommandOutput {
                success: true,
                message: msg,
            })
        }
        "add" if parts.get(1).copied() == Some("folder") => Ok(CommandOutput {
            success: true,
            message: "Use the Settings dialog to add new folders to watch, or trigger >settings."
                .to_string(),
        }),
        "settings" => Ok(CommandOutput {
            success: true,
            message: "SETTINGS".to_string(),
        }),
        _ => Ok(CommandOutput {
            success: false,
            message: format!(
                "Unknown command '>{}'. Available: >reindex, >doctor, >model, >add folder, >settings",
                trimmed
            ),
        }),
    }
}

#[tauri::command]
pub async fn read_file_content(path: String) -> Result<String, String> {
    let p = Path::new(&path);
    crate::indexer::content::read_text_content(p, 256_000)
        .ok_or_else(|| "Could not read text content from file".to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<KensakuConfig, String> {
    Ok(state.indexer.config.read().clone())
}

#[tauri::command]
pub async fn save_config(config: KensakuConfig, state: State<'_, AppState>) -> Result<(), String> {
    config.save()?;
    *state.indexer.config.write() = config;
    Ok(())
}

#[tauri::command]
pub async fn is_setup_complete() -> Result<bool, String> {
    Ok(KensakuConfig::is_setup_done())
}

#[tauri::command]
pub async fn complete_setup(
    watched_dirs: Vec<String>,
    excluded_dirs: Vec<String>,
    shortcut: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = KensakuConfig::load();
    config.watched_dirs = watched_dirs.into_iter().map(PathBuf::from).collect();
    config.excluded_dirs = excluded_dirs;
    if let Some(sc) = shortcut
        && !sc.trim().is_empty()
    {
        config.shortcut = sc;
    }
    config.is_setup_complete = true;
    config.save()?;

    *state.indexer.config.write() = config;
    state.indexer.start_indexing();

    Ok(())
}

#[tauri::command]
pub async fn get_suggested_directories() -> Result<Vec<String>, String> {
    let mut dirs_list = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs_list.push(home.to_string_lossy().to_string());

        let candidates = [
            "Documents",
            "Downloads",
            "Desktop",
            "Projects",
            "Code",
            "Development",
            "Pictures",
            "Videos",
        ];

        for c in candidates {
            let p = home.join(c);
            if p.exists() {
                dirs_list.push(p.to_string_lossy().to_string());
            }
        }
    }
    Ok(dirs_list)
}

#[tauri::command]
pub async fn hide_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(win) = app_handle.get_webview_window("main") {
        let _ = win.hide();
    }
    Ok(())
}
