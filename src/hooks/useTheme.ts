import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type Theme = "light" | "dark";

const STORAGE_KEY = "kensaku.theme";

/** Older builds stored "paper" and "void". Map them so nobody loses their pick. */
export function normalizeTheme(raw: string | null | undefined): Theme {
  if (raw === "light" || raw === "paper") return "light";
  if (raw === "dark" || raw === "void") return "dark";
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
 * Theme control. Two finishes: light and dark. localStorage holds the
 * pre-paint pick, but the backend config is the durable source of truth —
 * the launcher destroys its webview on hide, so localStorage (which may not
 * survive that) must never be trusted alone. On mount the persisted config
 * wins, unless the user has already picked in this session.
 */
let lastLocalChange = 0;

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(readStored);

  useEffect(() => {
    apply(theme);
  }, [theme]);

  useEffect(() => {
    const startedAt = Date.now();
    let cancelled = false;
    invoke<{ theme?: string }>("get_config")
      .then((cfg) => {
        if (cancelled || lastLocalChange > startedAt) return;
        const persisted = normalizeTheme(cfg?.theme);
        setThemeState((current) => {
          if (persisted === current) return current;
          try {
            localStorage.setItem(STORAGE_KEY, persisted);
          } catch {
            /* ignore */
          }
          return persisted;
        });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const setTheme = useCallback((t: Theme) => {
    lastLocalChange = Date.now();
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
    setTheme(theme === "light" ? "dark" : "light");
  }, [theme, setTheme]);

  return { theme, setTheme, toggle };
}
