# Themes and window materials

Light and dark are opaque themes. Frost is a clear, pale glass treatment over
a softly lit sky-blue backdrop with fine line detail, transparent layered panels and
gentle glossy reflections. Liquid Glass uses neutral translucent
surfaces over a bundled, muted multicolor artwork backdrop. Both use deeper
blur and specular edges. When a native material is
active, the window remains transparent so the platform can sample the desktop
behind it. When no native material is available, Liquid Glass uses the in-app
artwork as its backdrop; this CSS fallback does not sample the desktop.

| Platform | Frost | Liquid Glass |
| --- | --- | --- |
| macOS | AppKit `NSVisualEffectView` vibrancy | `NSGlassEffectView` Liquid Glass on macOS 26+; older supported versions use AppKit vibrancy |
| Windows 10 1809+ / Windows 11 | Windows Acrylic | Neutral-tinted Windows Acrylic with translucent UI |
| Linux with KDE Plasma on X11 | KWin blur-region request | KWin blur-region request with neutral translucent UI |
| Other Linux compositors, including Wayland sessions | Opaque fallback | In-app artwork with WebView blur; no desktop sampling |

These are platform-specific translations, not identical effects. In particular,
Windows does not expose Apple's Liquid Glass material, and KWin may ignore the
X11 blur request when its Blur effect or compositing is disabled. A successful
request is not proof that the compositor rendered the effect. The upstream
Windows Acrylic integration notes possible resize/drag slowdown on Windows 10
1903+ and Windows 11 build 22000. Liquid Glass also blurs translucent surfaces
and controls over the in-app artwork; native desktop material rendering and
performance need platform-specific validation.

The app reads macOS's `accessibilityDisplayShouldReduceTransparency` setting
and Windows' `EnableTransparency` preference before applying a material. A
WebView that exposes `prefers-reduced-transparency` is also honored. Reduced
transparency disables the artwork and blur and uses opaque surfaces. Linux has
no standardized system-wide reduced-transparency signal; use Light or Dark for
an explicit opaque appearance.

The app-wide artwork persists behind search, results, settings, and dialogs.
WebView surfaces blur that artwork when native materials are unavailable;
this is an in-app visual fallback, not desktop sampling. The Liquid Glass
preview uses the same artwork; it does not sample the desktop wallpaper. The
status below the previews reports whether a native material request is active
for the current app window.

The macOS build enables Tauri's `macOSPrivateApi` option for the native window
integration. These builds are intended for direct distribution, not the Mac
App Store.
