//! The one place the main search window is created, shown, and hidden.
//!
//! Hiding destroys the webview, so the WebKit processes exit and the hidden
//! app keeps only the Rust side (indexer, watcher, daemon client). Showing
//! creates the window again from the same settings. The last query is kept in
//! Rust so the search comes back where the user left it.

use std::collections::VecDeque;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const LABEL: &str = "main";

static LAST_QUERY: Mutex<String> = Mutex::new(String::new());
static RECENT_QUERIES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

pub fn remember_query(query: &str) {
    if let Ok(mut q) = LAST_QUERY.lock() {
        q.clear();
        q.push_str(query);
    }
}

pub fn record_recent_query(query: &str) {
    let trimmed = query.trim();
    if trimmed.starts_with('/') {
        return;
    }
    let count = trimmed.chars().count();
    if count == 0 {
        return;
    }
    if count == 1 {
        let ch = trimmed.chars().next().unwrap_or_default();
        if !crate::indexer::tokenizer::is_cjk(ch) {
            return;
        }
    }
    let mut list = match RECENT_QUERIES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let trimmed_lower = trimmed.to_lowercase();
    // Case-insensitive deduplication and supersede exact prefix extensions
    list.retain(|q| {
        let q_lower = q.to_lowercase();
        q_lower != trimmed_lower && !trimmed_lower.starts_with(&format!("{q_lower} "))
    });
    list.push_front(trimmed.to_string());
    if list.len() > 5 {
        list.pop_back();
    }
}

pub fn recent_queries() -> Vec<String> {
    let list = match RECENT_QUERIES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    list.iter().cloned().collect()
}

pub fn clear_recent_queries() {
    let mut list = match RECENT_QUERIES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    list.clear();
}

pub fn last_query() -> String {
    LAST_QUERY.lock().map(|q| q.clone()).unwrap_or_default()
}

fn existing(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// `true` when the error is WebView2's `ERROR_BUSY` (`0x800700AA`).
///
/// WebView2 raises this while another process still holds its user-data
/// folder: usually a previous Kensaku instance whose `msedgewebview2.exe`
/// children outlived a crash or a fast hide/show cycle. It clears by itself
/// once those processes exit, so the caller retries instead of calling it a
/// crash (#176).
pub(crate) fn is_webview_busy(error: &str) -> bool {
    error.to_ascii_lowercase().contains("0x800700aa")
}

/// Delays before each retry of a busy window creation. Short at first (a
/// hide/show cycle clears in well under a second), longer for the rare case
/// where a crashed instance's WebView2 processes take a moment to exit.
const BUSY_RETRY_DELAYS: [std::time::Duration; 3] = [
    std::time::Duration::from_millis(300),
    std::time::Duration::from_millis(900),
    std::time::Duration::from_millis(2000),
];

fn present(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    // In "app" mode the window is a normal application window: decorated,
    // opaque, in the dock/taskbar, and not pinned above other windows. In
    // "launcher" mode it stays frameless, off the taskbar, and floats like a
    // palette. (`app_mode` itself defaults to launcher in the config so
    // existing installs keep their behavior; Setup defaults new users to app.)
    //
    // Before setup is complete the window is always created as an ordinary
    // app window. The first-run wizard can index for a long time, so it has to
    // be minimisable and findable in the dock/taskbar; a frameless, off-taskbar,
    // always-on-top palette cannot be minimised or restored, and that is what a
    // fresh install used to get while the wizard was running. The user's mode
    // choice is applied when the wizard closes (`apply_window_mode`).
    let config = crate::config::KensakuConfig::load();
    let in_setup = !config.is_setup_complete;
    let launcher = !in_setup && config.app_mode != "app";
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Kensaku")
        .inner_size(920.0, 640.0)
        .min_inner_size(720.0, 520.0)
        .decorations(!launcher)
        .transparent(launcher)
        .always_on_top(launcher)
        .skip_taskbar(launcher)
        .visible_on_all_workspaces(launcher)
        .center()
        .shadow(true)
        .resizable(true)
        .visible(false)
        .build()?;

    // Closing the window hides it and keeps the app running in the tray.
    //
    // App mode used to call `exit(0)` here, which threw away the loaded
    // metadata database, full-text index and semantic units, and paid for them
    // again on the next open. On a large corpus that is several seconds per
    // open, for a window the user is about to close again. Hiding keeps the
    // Rust side warm; `hide` destroys the webview and trims memory, so what
    // stays resident is the index, not a UI.
    //
    // Quitting stays explicit: the tray's Quit item, or `close_to_background`
    // turned off. Both modes now behave the same way, because both are
    // background-resident apps; the mode only decides what the window looks
    // like.
    //
    // Config is read inside the handler so a Settings change applies to the
    // live window without recreating it.
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if !crate::config::KensakuConfig::load().close_to_background {
                app_handle.exit(0);
                return;
            }
            api.prevent_close();
            hide(&app_handle);
        }
    });
    Ok(window)
}

/// Applies the parts of `app_mode` that can change on a live window. App mode
/// gets native decorations (title bar, minimize/close) so it behaves like a
/// normal desktop window; launcher mode stays frameless.
pub fn apply_mode(window: &WebviewWindow, app_mode: &str) {
    // While the setup wizard is up, the window stays an ordinary app window
    // whatever mode is configured: a frameless, off-taskbar, always-on-top
    // palette cannot be minimised or restored, and the wizard sits there for
    // the whole first scan. `apply_window_mode` runs again when it closes.
    // `is_setup_complete` rather than `is_setup_done`: the latter also waits
    // for the first scan to finish, and the wizard closes before that, so the
    // user's chosen mode would be ignored until the next launch.
    let setup_complete = crate::config::KensakuConfig::load().is_setup_complete;
    let launcher = app_mode != "app" && setup_complete;
    let _ = window.set_decorations(!launcher);
    let _ = window.set_skip_taskbar(launcher);
    let _ = window.set_always_on_top(launcher);
    let _ = window.set_visible_on_all_workspaces(launcher);
}

/// Tells the user why the app has no window, even when release builds run
/// without a console. The log path is included so a bug report is actionable.
fn report_creation_failure(app: &AppHandle, error: &str) {
    crate::crashlog::record_error("main window creation", error);
    crate::say_err!("[Kensaku] could not create the window: {error}");

    use tauri_plugin_dialog::DialogExt;
    let hint = if cfg!(windows) {
        if is_webview_busy(error) {
            "\n\nAnother copy of Kensaku, or a WebView2 helper left behind by a previous run, is still holding Kensaku's WebView2 profile. Quit Kensaku from the tray, wait a few seconds, then start it again. If this keeps happening, end any remaining \"Microsoft Edge WebView2\" processes in Task Manager."
        } else {
            "\n\nThis usually means the Microsoft Edge WebView2 runtime is missing or broken. Install it from Microsoft, then try again."
        }
    } else {
        ""
    };
    // `blocking_show` must not run on the main thread, and this is called from
    // `.setup` and tray handlers. `show` is non-blocking and posts the dialog
    // to the platform's event loop.
    app.dialog()
        .message(format!(
            "Kensaku could not open its window.\n\n{error}{hint}\n\nDetails were written to:\n{}",
            crate::crashlog::path().display()
        ))
        .title("Kensaku could not start")
        .show(|_| {});
}

/// Shows the search window, creating it if it was hidden (destroyed).
pub fn show(app: &AppHandle) {
    if let Some(window) = existing(app) {
        present(&window);
        return;
    }
    match create(app) {
        Ok(window) => present(&window),
        Err(e) => {
            let error = e.to_string();
            if is_webview_busy(&error) {
                // Transient: the previous WebView2 profile owner is exiting.
                retry_create_when_idle(app, error);
            } else {
                report_creation_failure(app, &error);
            }
        }
    }
}

/// Retries window creation while WebView2 reports `ERROR_BUSY`, without
/// blocking the caller (setup or a tray handler). Creation has to happen on
/// the main thread, so each attempt is posted there and the worker waits for
/// its result.
fn retry_create_when_idle(app: &AppHandle, first_error: String) {
    let app_for_thread = app.clone();
    let first = first_error.clone();
    let spawned = std::thread::Builder::new()
        .name("window-retry".to_string())
        .spawn(move || {
            let mut last = first;
            for delay in BUSY_RETRY_DELAYS {
                std::thread::sleep(delay);
                let (tx, rx) = std::sync::mpsc::channel();
                let handle = app_for_thread.clone();
                let main = handle.clone();
                let queued = handle.run_on_main_thread(move || {
                    let result = match existing(&main) {
                        Some(window) => {
                            present(&window);
                            Ok(())
                        }
                        None => create(&main).map(|window| present(&window)),
                    };
                    let _ = tx.send(result.map_err(|e| e.to_string()));
                });
                if queued.is_err() {
                    break;
                }
                match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                    Ok(Ok(())) => return,
                    Ok(Err(e)) => last = e,
                    // The main thread never answered; stop rather than retry
                    // blind while the user is left without a window.
                    Err(_) => break,
                }
            }
            report_creation_failure(&app_for_thread, &last);
        });
    if spawned.is_err() {
        report_creation_failure(app, &first_error);
    }
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
///
/// Desktop-app windows are never hidden by a toggle: the tray item, a stray
/// `--toggle`, or a shortcut registered before a mode change should bring the
/// window forward, not make a normal app vanish from the taskbar.
pub fn toggle(app: &AppHandle) {
    let visible = existing(app).is_some_and(|w| w.is_visible().unwrap_or(false));
    if visible && crate::config::KensakuConfig::load().app_mode != "app" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_busy_is_recognised_in_the_tauri_error_text() {
        // The exact shape `create()` produced in #176, including the localized
        // message from the reporter's system.
        let reported = "runtime error: failed to create webview: WebView2 error: \
                        WindowsError(Error { code: HRESULT(0x800700AA), message: \
                        \"Требуемый ресурс занят.\" })";
        assert!(is_webview_busy(reported));
        assert!(is_webview_busy("hrresult(0x800700aa)"));
        assert!(!is_webview_busy(
            "WebView2 runtime is missing or broken (0x8007007E)"
        ));
    }

    #[test]
    fn recent_queries_records_deduplicates_and_caps_at_five() {
        clear_recent_queries();
        assert!(recent_queries().is_empty());

        // Single Latin character and empty strings are ignored
        record_recent_query("");
        record_recent_query("   ");
        record_recent_query("a");
        record_recent_query("  b  ");
        assert!(recent_queries().is_empty());

        // Slash commands are ignored
        record_recent_query("/settings");
        record_recent_query("/doctor");
        assert!(recent_queries().is_empty());

        // Single CJK characters are valid and retained
        record_recent_query("税");
        assert_eq!(recent_queries(), vec!["税"]);
        record_recent_query("书");
        assert_eq!(recent_queries(), vec!["书", "税"]);

        clear_recent_queries();

        // Valid queries
        record_recent_query("alpha");
        record_recent_query("beta");
        record_recent_query("gamma");
        assert_eq!(recent_queries(), vec!["gamma", "beta", "alpha"]);

        // Case-insensitive deduplication moves to front
        record_recent_query("Alpha");
        assert_eq!(recent_queries(), vec!["Alpha", "gamma", "beta"]);

        // Prefix extension superseding
        record_recent_query("Alpha project");
        assert_eq!(recent_queries(), vec!["Alpha project", "gamma", "beta"]);

        // Cap at 5
        record_recent_query("delta");
        record_recent_query("epsilon");
        record_recent_query("zeta");
        assert_eq!(
            recent_queries(),
            vec!["zeta", "epsilon", "delta", "Alpha project", "gamma"]
        );
        assert_eq!(recent_queries().len(), 5);

        clear_recent_queries();
        assert!(recent_queries().is_empty());
    }
}
