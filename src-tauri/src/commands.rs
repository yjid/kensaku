use crate::config::KensakuConfig;
use crate::indexer::IndexingStatus;
use crate::search::{SearchDispatcher, SearchResult};
use crate::AppState;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn search(query: String, state: State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let indexer = &state.indexer;
    let metadata = indexer.metadata.read();
    let content_index = indexer.content_index.read();
    let max_results = indexer.config.read().max_results;

    let results = SearchDispatcher::search(&query, &metadata, &content_index, max_results);
    Ok(results)
}

#[tauri::command]
pub async fn get_indexing_status(state: State<'_, AppState>) -> Result<IndexingStatus, String> {
    Ok(state.indexer.get_status())
}

#[tauri::command]
pub async fn open_file(path: String) -> Result<(), String> {
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
    if let Some(sc) = shortcut {
        if !sc.trim().is_empty() {
            config.shortcut = sc;
        }
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
