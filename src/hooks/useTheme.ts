import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Theme = "paper" | "void";

const STORAGE_KEY = "kensaku.theme";

function readStored(): Theme {
  try {
    const t = localStorage.getItem(STORAGE_KEY);
    if (t === "paper" || t === "void") return t;
  } catch {
    /* storage unavailable (e.g. webview restrictions) */
  }
  return "void";
}

function apply(t: Theme) {
  document.documentElement.dataset.theme = t;
}

/**
 * Theme control. Persists via localStorage and best-effort mirrors the
 * choice to the backend (`set_theme` may not exist yet — never blocks on
 * failure). `data-theme` is also set pre-paint in index.html.
 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(readStored);

  useEffect(() => {
    apply(theme);
  }, [theme]);

  const setTheme = useCallback((t: Theme) => {
    setThemeState(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {
      /* ignore */
    }
    invoke("set_theme", { theme: t }).catch((err) =>
      console.warn("set_theme not available yet:", err),
    );
  }, []);

  const toggle = useCallback(() => {
    const next: Theme = theme === "paper" ? "void" : "paper";
    setTheme(next);
  }, [theme, setTheme]);

  return { theme, setTheme, toggle };
}
