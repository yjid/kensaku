import React from "react";
import type { IndexingStatus } from "../types";
import { formatComma } from "../lib/format";

/* Status line. Index health on the left, the few keys worth knowing on the
   right. Quiet by default: it only takes the ink while indexing. */

const HINTS: { keys: string; label: string }[] = [
  { keys: "↵", label: "Open" },
  { keys: "Space", label: "Preview" },
  { keys: "⇧↵", label: "Reveal" },
  { keys: "⌘I", label: "Details" },
];

function statusText(s: IndexingStatus | null | undefined): { text: string; busy: boolean } {
  if (!s) return { text: "", busy: false };
  if (s.paused) return { text: "Indexing paused", busy: false };
  if (s.is_indexing || (s.phase && s.phase !== "idle")) {
    const done = s.extract_done ?? s.files_indexed;
    const total = s.extract_total ?? s.total_files;
    return {
      text: total ? `Indexing ${formatComma(done)} of ${formatComma(total)}` : `Indexing ${formatComma(done)} files`,
      busy: true,
    };
  }
  return { text: `${formatComma(s.files_indexed)} files`, busy: false };
}

export const KeyHintStrip: React.FC<{ status?: IndexingStatus | null }> = ({ status }) => {
  const { text, busy } = statusText(status);
  return (
    <div className="relative z-10 flex h-7 shrink-0 items-center justify-between gap-4 px-3.5 text-[11px] text-subtle">
      <span className="flex items-center gap-2">
        {text && (
          <>
            <span className={`h-1.5 w-1.5 rounded-full ${busy ? "dot-pulse bg-ink" : "bg-success/70"}`} />
            <span className="t-meta">{text}</span>
          </>
        )}
      </span>
      <span className="hidden items-center gap-4 sm:flex">
        {HINTS.map((h) => (
          <span key={h.label} className="flex items-center gap-1.5">
            <span className="kbd">{h.keys}</span>
            <span>{h.label}</span>
          </span>
        ))}
      </span>
    </div>
  );
};
