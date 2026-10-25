import React from "react";
import { Sun, Moon, Settings, CircleAlert } from "lucide-react";
import { IndexingStatus } from "../types";
import { formatComma } from "../lib/format";
import { latencyToken } from "../lib/format";

/* 28px status bar: `3.2 ms · 1,204,553 files · 2 s old`, semantic + watcher
   dots, indexing text, theme toggle and settings gear. Mono 11px. */

interface StatusBarProps {
  searchTimeMs: number | null;
  resultCount: number;
  indexingStatus: IndexingStatus | null;
  stats: {
    files: number;
    lastReconcileSecsAgo: number | null;
    watcherOk: boolean;
  } | null;
  theme: "paper" | "void";
  onToggleTheme: () => void;
  onOpenSettings: (section?: string) => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  searchTimeMs,
  resultCount,
  indexingStatus,
  stats,
  theme,
  onToggleTheme,
  onOpenSettings,
}) => {
  const indexing = indexingStatus?.is_indexing ?? false;
  const secondsOld = stats?.lastReconcileSecsAgo ?? null;
  const semanticReady = indexingStatus?.semantic_ready ?? false;
  const isEmbedding = indexingStatus?.is_embedding ?? false;

  return (
    <div className="flex h-7 shrink-0 items-center justify-between border-t border-line bg-sunken px-3 font-mono text-[11px] leading-none text-muted select-none">
      {/* Left: performance + index freshness */}
      <div className="flex min-w-0 items-center gap-3">
        {searchTimeMs !== null && (
          <span className={latencyToken(searchTimeMs)}>{searchTimeMs.toFixed(1)} ms</span>
        )}
        {(stats?.files || 0) > 0 && <span>{formatComma(stats!.files)} files</span>}
        {secondsOld !== null && secondsOld > 0 && (
          <span>
            {secondsOld < 60
              ? `${Math.round(secondsOld)} s`
              : secondsOld < 3600
                ? `${Math.round(secondsOld / 60)} min`
                : `${Math.round(secondsOld / 3600)} h`}{" "}
            old
          </span>
        )}
        {searchTimeMs !== null && <span>{formatComma(resultCount)} results</span>}

        {indexing && (
          <span className="flex items-center gap-1.5 text-fg">
            <span className="h-1.5 w-1.5 rounded-full bg-accent dot-pulse" />
            {indexingStatus?.current_status || "indexing"}
            {(indexingStatus?.files_indexed ?? 0) > 0 && (
              <span className="text-subtle">
                ·{formatComma(indexingStatus?.files_indexed ?? 0)} f
              </span>
            )}
          </span>
        )}
      </div>

      {/* Right: semantic + watcher + theme + settings */}
      <div className="flex shrink-0 items-center gap-2.5">
        {isEmbedding ? (
          <button
            type="button"
            onClick={() => onOpenSettings("ai")}
            title="Embedding in background — open AI & Semantic settings"
            className="flex h-4 items-center gap-1 rounded-full bg-accent-soft px-1.5 font-medium text-accent transition-colors hover:bg-accent/20"
          >
            <span className="h-1.5 w-1.5 rounded-full bg-accent dot-pulse" />
            semantic
            {typeof indexingStatus?.units_indexed === "number" && (
              <span className="font-mono">{formatComma(indexingStatus.units_indexed ?? 0)}</span>
            )}
          </button>
        ) : (
          semanticReady && (
            <span
              title="Semantic index ready — results fused via RRF"
              className="flex h-4 items-center gap-1"
            >
              <span className="h-1.5 w-1.5 rounded-full bg-success" />
              semantic
            </span>
          )
        )}

        {stats && !stats.watcherOk && (
          <span
            title="File watcher unavailable — run Doctor (>) to diagnose"
            className="flex h-4 items-center gap-1 text-warning"
          >
            <CircleAlert className="h-3 w-3" />
            watcher
          </span>
        )}

        <button
          type="button"
          title={theme === "paper" ? "Switch to void theme" : "Switch to paper theme"}
          onClick={onToggleTheme}
          className="flex h-5 w-5 items-center justify-center rounded text-muted transition-colors duration-150 hover:bg-elevated hover:text-fg"
        >
          {theme === "paper" ? <Moon className="h-3 w-3" /> : <Sun className="h-3 w-3" />}
        </button>

        <button
          type="button"
          title="Settings (⌘,)"
          onClick={() => onOpenSettings()}
          className="flex h-5 w-5 items-center justify-center rounded text-muted transition-colors duration-150 hover:bg-elevated hover:text-fg"
        >
          <Settings className="h-3 w-3" />
        </button>
      </div>
    </div>
  );
};
