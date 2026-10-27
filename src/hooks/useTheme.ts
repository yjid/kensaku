import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Theme = "light" | "dark" | "glass";

const STORAGE_KEY = "kensaku.theme";

/** Older builds stored "paper" and "void". Map them so nobody loses their pick. */
export function normalizeTheme(raw: string | null | undefined): Theme {
  if (raw === "light" || raw === "paper") return "light";
  if (raw === "dark" || raw === "void") return "dark";
  if (raw === "glass") return "glass";
  return "dark";
}

function readStored(): Theme {
  try {
    return normalizeTheme(localStorage.getItem(STORAGE_KEY));
  } catch {
    /* storage unavailable (e.g. webview restrictions) */
    return "dark";
  }
}

function apply(t: Theme) {
  document.documentElement.dataset.theme = t;
}

/**
 * Theme control. Three finishes: light, dark, glass. Persists via localStorage
 * and best-effort mirrors the choice to the backend (`set_theme` may not exist
 * yet, never blocks on failure). `data-theme` is also set pre-paint in index.html.
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

  /** Cycles light, dark, glass. */
  const toggle = useCallback(() => {
    const order: Theme[] = ["light", "dark", "glass"];
    setTheme(order[(order.indexOf(theme) + 1) % order.length]);
  }, [theme, setTheme]);

  return { theme, setTheme, toggle };
}
