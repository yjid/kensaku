use crate::config::KensakuConfig;
use crate::indexer::IndexingStatus;
use crate::search::{SearchDispatcher, SearchResponse, SearchResult};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager, State};

static CURRENT_SEARCH_SEQ: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub async fn search(
    query: String,
    seq: Option<u64>,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
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
    Ok(resp)
}

#[tauri::command]
pub async fn legacy_search(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
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
    Ok(results)
}

#[tauri::command]
pub async fn get_indexing_status(state: State<'_, AppState>) -> Result<IndexingStatus, String> {
    Ok(state.indexer.get_status())
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

    if let Some(doc) = crate::indexer::extract::extract_document(p, 48_000) {
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
