pub mod commands;
pub mod config;
pub mod embed;
pub mod indexer;
pub mod query;
pub mod search;

use config::KensakuConfig;
use indexer::IndexerManager;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    pub indexer: Arc<IndexerManager>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::init_process_start();
    let config = KensakuConfig::load();
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
            commands::execute_command,
            commands::read_file_content,
            commands::get_config,
            commands::save_config,
            commands::is_setup_complete,
            commands::complete_setup,
            commands::get_suggested_directories,
            commands::hide_window,
        ])
        .setup(move |app| {
            app.state::<AppState>().indexer.set_app_handle(app.handle().clone());

            // 1. Register global shortcut (e.g. Alt+Space)
            if let Ok(shortcut) = Shortcut::from_str(&shortcut_str) {
                let _ = app.global_shortcut().register(shortcut);
            } else {
                eprintln!("[Kensaku] Could not parse global shortcut: {}", shortcut_str);
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
