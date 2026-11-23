// Resolve the theme before first paint to avoid a flash. Mirrors
// useTheme.ts: a stored value (including the pre-1.1 "paper"/"void") wins;
// otherwise follow the OS. If both fail, the hardcoded data-theme="dark" in
// index.html stays, matching the config default.
//
// This lives outside index.html so the webview can run under a strict CSP
// without an 'unsafe-inline' script exemption. It is a classic (blocking)
// script on purpose: a module would run after the first paint.
(function () {
  var paintTheme = null;
  try {
    var stored = localStorage.getItem("kensaku.theme");
    var map = { paper: "light", light: "light", void: "dark", dark: "dark" };
    // A stored but unrecognized value is "dark", exactly like
    // normalizeTheme() in useTheme.ts (never the OS).
    if (stored) paintTheme = map[stored] || "dark";
  } catch (e) {
    /* storage unavailable */
  }
  if (!paintTheme) {
    try {
      paintTheme =
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: light)").matches
          ? "light"
          : "dark";
    } catch (e) {
      /* no matchMedia; keep the hardcoded default */
    }
  }
  if (paintTheme) document.documentElement.dataset.theme = paintTheme;
})();
