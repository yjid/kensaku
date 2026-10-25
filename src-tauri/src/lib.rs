pub mod agent;
pub mod cli;
pub mod commands;
pub mod config;
pub mod crashlog;
pub mod doctor;
pub mod embed;
pub mod indexer;
pub mod models;
pub mod query;
pub mod search;

use config::KensakuConfig;
use indexer::IndexerManager;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use tauri::{
    Manager, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    pub indexer: Arc<IndexerManager>,
}

/// Owns the single-instance lock for the whole process lifetime: the OS releases
/// the advisory lock when the process exits, and dropping the `File` earlier
/// would let a second instance start.
static INSTANCE_LOCK: OnceLock<std::fs::File> = OnceLock::new();

fn instance_lock_path() -> PathBuf {
    KensakuConfig::config_dir().join("kensaku.lock")
}

/// Tries to take the process-wide exclusive lock on `<config_dir>/kensaku.lock`.
///
/// * `Ok(Some(file))` — acquired; the caller must keep `file` alive.
/// * `Ok(None)` — another instance already holds the lock.
/// * `Err(_)` — the lock file could not be created or opened. Startup continues
///   without the guard rather than bricking the app on a read-only config dir.
fn acquire_instance_lock() -> Result<Option<std::fs::File>, String> {
    use fs4::fs_std::FileExt;

    let dir = KensakuConfig::config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {}", dir.display(), e))?;

    let path = instance_lock_path();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("{}: {}", path.display(), e))?;

    match file.try_lock_exclusive() {
        Ok(true) => Ok(Some(file)),
        Ok(false) => Ok(None),
        Err(e) => Err(format!("{}: {}", path.display(), e)),
    }
}

/// Acquires the lock, exiting quietly when another instance is running.
fn acquire_instance_lock_or_exit() -> Option<std::fs::File> {
    match acquire_instance_lock() {
        Ok(Some(file)) => Some(file),
        Ok(None) => {
            eprintln!("[Kensaku] Another Kensaku instance is already running; exiting.");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!(
                "[Kensaku] Could not acquire single-instance lock ({}); continuing without it",
                e
            );
            None
        }
    }
}

/// Marker consumed on the next launch to wipe all Kensaku state.
fn reset_marker_path() -> std::path::PathBuf {
    KensakuConfig::config_dir().join(".reset-requested")
}

/// True when a reset was requested but not applied yet.
pub fn reset_pending() -> bool {
    reset_marker_path().exists()
}

/// Clears Kensaku state when a reset was requested by `reset_all`.
///
/// Must run before `KensakuConfig::load()` and before `IndexerManager::new()`:
/// removing the directory while the database or index is open would leave
/// handles pointing at deleted inodes.
fn maybe_apply_pending_reset() {
    let marker = reset_marker_path();
    if !marker.exists() {
        return;
    }

    let dir = KensakuConfig::config_dir();
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!(
                    "[Kensaku] Reset requested but could not recreate {}: {}",
                    dir.display(),
                    e
                );
                return;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
            }
            eprintln!(
                "[Kensaku] Reset requested: cleared all data in {}",
                dir.display()
            );
        }
        Err(e) => eprintln!(
            "[Kensaku] Reset requested but could not clear {}: {}",
            dir.display(),
            e
        ),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install the crash log hook before anything else so startup failures are
    // captured too. It only touches the config directory path; it never loads
    // or mutates the config.
    crashlog::install_panic_hook();

    commands::init_process_start();

    // Single-instance guard. Acquire it before touching any state so a second
    // launch can never race the first one on the database, index or config.
    let mut instance_lock = acquire_instance_lock_or_exit();

    maybe_apply_pending_reset();

    // A pending reset deletes the config directory, which removes the lock
    // file. Re-acquire so the recreated directory is protected as well.
    if instance_lock.is_some() && !instance_lock_path().exists() {
        instance_lock = acquire_instance_lock_or_exit();
    }

    if let Some(file) = instance_lock {
        // Deliberately kept alive until the process exits.
        let _ = INSTANCE_LOCK.set(file);
    }

    let config = KensakuConfig::load();
    // OCR is enforced through the extraction layer's process-wide flag.
    indexer::extract::set_ocr_enabled(config.ocr_enabled);
    let is_setup = config.is_setup_complete;
    let indexer_manager = Arc::new(IndexerManager::new(config.clone()));

    // If setup was completed previously, automatically start indexing on startup
    if is_setup {
        indexer_manager.start_indexing();
    }

    let app_state = AppState {
        indexer: indexer_manager,
    };

    let shortcut_str = config.shortcut.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && let Some(window) = app.get_webview_window("main")
                    {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(),
        )
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::legacy_search,
            commands::get_indexing_status,
            commands::get_stats,
            commands::set_theme,
            commands::open_file,
            commands::open_folder,
            commands::get_file_preview,
            commands::get_thumbnail,
            commands::get_ocr_status,
            commands::download_ocr_language,
            commands::find_duplicates,
            commands::execute_command,
            commands::read_file_content,
            commands::get_config,
            commands::save_config,
            commands::is_setup_complete,
            commands::complete_setup,
            commands::get_suggested_directories,
            commands::hide_window,
            commands::run_doctor,
            commands::reset_all,
            commands::get_reset_pending,
            commands::get_autostart,
            commands::set_autostart,
            models::get_models_status,
            models::download_models,
            models::delete_models,
        ])
        .setup(move |app| {
            app.state::<AppState>()
                .indexer
                .set_app_handle(app.handle().clone());

            // 1. Register global shortcut (e.g. Alt+Space)
            if let Ok(shortcut) = Shortcut::from_str(&shortcut_str) {
                let _ = app.global_shortcut().register(shortcut);
            } else {
                eprintln!(
                    "[Kensaku] Could not parse global shortcut: {}",
                    shortcut_str
                );
            }

            // 2. Setup System Tray
            let show_item = MenuItem::with_id(app, "toggle", "Open Search", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Kensaku", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "toggle" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _ = tray_builder.build(app);

            // 3. Keep app alive in background on window close
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });

                // Show window on launch
                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Kensaku application");
}
