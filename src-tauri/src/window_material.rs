use serde::Serialize;
use tauri::WebviewWindow;

#[derive(Clone, Debug, Serialize)]
pub struct MaterialStatus {
    pub active: bool,
    pub effect: Option<&'static str>,
    pub reason: Option<&'static str>,
}

impl MaterialStatus {
    fn active(effect: &'static str, reason: Option<&'static str>) -> Self {
        Self {
            active: true,
            effect: Some(effect),
            reason,
        }
    }

    pub fn unavailable(reason: &'static str) -> Self {
        Self {
            active: false,
            effect: None,
            reason: Some(reason),
        }
    }
}

pub fn apply(window: &WebviewWindow, theme: &str, reduce_transparency: bool) -> MaterialStatus {
    clear(window);

    if !matches!(theme, "frost" | "liquid-glass") {
        return MaterialStatus::unavailable("solid-theme");
    }
    if reduce_transparency || os_prefers_reduced_transparency() {
        return MaterialStatus::unavailable("reduced-transparency");
    }

    #[cfg(target_os = "macos")]
    {
        apply_macos(window, theme)
    }

    #[cfg(target_os = "windows")]
    {
        apply_windows(window, theme)
    }

    #[cfg(target_os = "linux")]
    {
        apply_linux(window)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = window;
        MaterialStatus::unavailable("unsupported-platform")
    }
}

fn clear(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        if let Err(error) = window_vibrancy::clear_liquid_glass(window)
            && !matches!(error, window_vibrancy::Error::UnsupportedPlatformVersion(_))
        {
            eprintln!("[Kensaku] could not clear macOS Liquid Glass: {error:?}");
        }
        if let Err(error) = window_vibrancy::clear_vibrancy(window) {
            eprintln!("[Kensaku] could not clear macOS vibrancy: {error:?}");
        }
    }

    #[cfg(target_os = "windows")]
    if let Err(error) = window_vibrancy::clear_acrylic(window)
        && !matches!(error, window_vibrancy::Error::UnsupportedPlatformVersion(_))
    {
        eprintln!("[Kensaku] could not clear Windows Acrylic: {error:?}");
    }

    #[cfg(target_os = "linux")]
    if let Err(error) = clear_linux_blur(window) {
        eprintln!("[Kensaku] could not clear KWin blur request: {error}");
    }
}

#[cfg(target_os = "macos")]
fn os_prefers_reduced_transparency() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency()
}

#[cfg(target_os = "windows")]
fn os_prefers_reduced_transparency() -> bool {
    use std::process::Command;

    let output = Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "EnableTransparency",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .last();
            match value {
                Some("0x0") => true,
                Some("0x1") => false,
                _ => {
                    eprintln!("[Kensaku] could not parse Windows EnableTransparency preference");
                    false
                }
            }
        }
        Ok(_) => false,
        Err(error) => {
            eprintln!("[Kensaku] could not read Windows transparency preference: {error}");
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn os_prefers_reduced_transparency() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn apply_macos(window: &WebviewWindow, theme: &str) -> MaterialStatus {
    use window_vibrancy::{
        LiquidGlassOptions, NSGlassEffectViewStyle, NSVisualEffectMaterial, apply_liquid_glass,
        apply_vibrancy,
    };

    if theme == "liquid-glass" {
        let options = LiquidGlassOptions::new(NSGlassEffectViewStyle::Regular)
            .radius(22.0)
            .tint_color((255, 255, 255, 14))
            .opaque(false);
        match apply_liquid_glass(window, options) {
            Ok(()) => return MaterialStatus::active("macOS Liquid Glass", None),
            Err(glass_error) => {
                match apply_vibrancy(window, NSVisualEffectMaterial::HudWindow, None, None) {
                    Ok(()) => {
                        return MaterialStatus::active(
                            "macOS vibrancy",
                            Some("legacy-liquid-glass"),
                        );
                    }
                    Err(vibrancy_error) => {
                        eprintln!(
                            "[Kensaku] native glass failed ({glass_error:?}); \
                             macOS vibrancy fallback failed ({vibrancy_error:?})"
                        );
                    }
                }
            }
        }
    } else {
        match apply_vibrancy(window, NSVisualEffectMaterial::Sidebar, None, None) {
            Ok(()) => return MaterialStatus::active("macOS vibrancy", None),
            Err(error) => {
                eprintln!("[Kensaku] could not apply macOS Frost material: {error:?}");
            }
        }
    }

    MaterialStatus::unavailable("native-effect-failed")
}

#[cfg(target_os = "windows")]
fn apply_windows(window: &WebviewWindow, theme: &str) -> MaterialStatus {
    let color = if theme == "frost" {
        (208, 238, 255, 210)
    } else {
        (235, 240, 246, 40)
    };

    match window_vibrancy::apply_acrylic(window, Some(color)) {
        Ok(()) => MaterialStatus::active("Windows Acrylic", None),
        Err(error) => {
            eprintln!("[Kensaku] could not apply Windows Acrylic: {error:?}");
            MaterialStatus::unavailable("native-effect-failed")
        }
    }
}

#[cfg(target_os = "linux")]
fn apply_linux(window: &WebviewWindow) -> MaterialStatus {
    if !is_kwin_x11(window) {
        return MaterialStatus::unavailable("unsupported-platform");
    }

    match set_linux_blur(window, false) {
        Ok(()) => {
            LINUX_BLUR_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
            MaterialStatus::active("KWin X11 blur request", None)
        }
        Err(error) => {
            eprintln!("[Kensaku] could not request KWin X11 blur: {error}");
            MaterialStatus::unavailable("native-effect-failed")
        }
    }
}

#[cfg(target_os = "linux")]
pub fn update_linux_blur(window: &WebviewWindow) {
    if LINUX_BLUR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
        && is_kwin_x11(window)
        && let Err(error) = set_linux_blur(window, false)
    {
        eprintln!("[Kensaku] could not update KWin blur region: {error}");
    }
}

#[cfg(target_os = "linux")]
static LINUX_BLUR_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
fn is_kwin_x11(window: &WebviewWindow) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    std::env::var_os("KDE_FULL_SESSION").is_some()
        && window
            .window_handle()
            .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Xlib(_)))
}

#[cfg(target_os = "linux")]
fn clear_linux_blur(window: &WebviewWindow) -> Result<(), String> {
    LINUX_BLUR_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    if is_kwin_x11(window) {
        set_linux_blur(window, true)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_linux_blur(window: &WebviewWindow, clear: bool) -> Result<(), String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, PropMode};
    use x11rb::wrapper::ConnectionExt;

    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Xlib(xlib) = handle.as_raw() else {
        return Err("the window is not using X11".to_string());
    };
    let (connection, _) = x11rb::connect(None).map_err(|error| error.to_string())?;
    let blur_atom = connection
        .intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION")
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?
        .atom;

    if clear {
        connection
            .delete_property(xlib.window, blur_atom)
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
    } else {
        let size = window.inner_size().map_err(|error| error.to_string())?;
        let cardinal_atom = AtomEnum::CARDINAL.into();
        connection
            .change_property32(
                PropMode::REPLACE,
                xlib.window,
                blur_atom,
                cardinal_atom,
                &[0, 0, size.width.max(1), size.height.max(1)],
            )
            .map_err(|error| error.to_string())?
            .check()
            .map_err(|error| error.to_string())?;
    }

    connection.flush().map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn os_prefers_reduced_transparency() -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
#[allow(unused_variables)]
pub fn update_linux_blur(window: &WebviewWindow, theme: &str) {}
