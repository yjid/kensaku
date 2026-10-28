//! The one place the main search window is created, shown, and hidden.
//!
//! Hiding destroys the webview, so the WebKit processes exit and the hidden
//! app keeps only the Rust side (indexer, watcher, daemon client). Showing
//! creates the window again from the same settings. The last query is kept in
//! Rust so the search comes back where the user left it.

use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const LABEL: &str = "main";

static LAST_QUERY: Mutex<String> = Mutex::new(String::new());

pub fn remember_query(query: &str) {
    if let Ok(mut q) = LAST_QUERY.lock() {
        q.clear();
        q.push_str(query);
    }
}

pub fn last_query() -> String {
    LAST_QUERY.lock().map(|q| q.clone()).unwrap_or_default()
}

fn existing(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Kensaku")
        .inner_size(920.0, 640.0)
        .min_inner_size(720.0, 520.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .center()
        .shadow(true)
        .resizable(true)
        .visible(false)
        .build()?;

    // The close button hides, the same as Esc: the window goes away and the
    // app keeps running in the background.
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            hide(&app_handle);
        }
    });
    Ok(window)
}

/// Shows the search window, creating it if it was hidden (destroyed).
pub fn show(app: &AppHandle) {
    let window = match existing(app) {
        Some(w) => w,
        None => match create(app) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[Kensaku] could not create the window: {e}");
                return;
            }
        },
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Hides the search window by destroying it, then returns freed memory.
pub fn hide(app: &AppHandle) {
    if let Some(window) = existing(app) {
        // Hide first, then destroy: on Windows, tearing down a visible
        // transparent + always-on-top window while WebView2 crashes out (the
        // process list is killed below) can briefly leave a composited ghost
        // frame on screen. Once hidden, nothing of the window is visible.
        let _ = window.hide();
        let _ = window.destroy();
    }
    // WebKit/WebView helpers otherwise stay alive after the window is
    // destroyed, keeping tens to hundreds of MB resident. Kill them and trim
    // the allocator so the hidden app keeps only the Rust side.
    crate::util::memory::kill_webview_processes();
    crate::util::memory::release_memory();
}

/// Shows the window if it is hidden, hides it if it is shown.
pub fn toggle(app: &AppHandle) {
    if existing(app).is_some_and(|w| w.is_visible().unwrap_or(false)) {
        hide(app);
    } else {
        show(app);
    }
}

/// Returns freed heap pages to the OS. glibc otherwise keeps them, so the
/// process can report more memory than it still uses.
pub fn release_free_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::malloc_trim(0);
    }
}
