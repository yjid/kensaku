import { useEffect } from "react";
import { SearchResult } from "../types";
import { formatBytes, formatRelativeTime } from "../lib/format";

/**
 * "How we found it" view. Opened from the right-click menu, never inline.
 * Shows the match type, the score, and the facts about the file.
 */
export function DetailsDialog({
  result,
  onClose,
}: {
  result: SearchResult;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const rows: [string, string][] = [
    ["Location", result.path],
    ["Size", formatBytes(result.file_size)],
    ["Modified", formatRelativeTime(result.modified)],
    ["Matched by", matchLabel(result.match_type)],
    ["Score", result.score.toFixed(2)],
  ];

  return (
    <div
      className="overlay-in fixed inset-0 z-[80] flex items-center justify-center bg-scrim p-6"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-label="Details"
        className="w-full max-w-[460px] rounded-[16px] border border-line bg-elevated p-5 shadow-lg"
      >
        <p className="text-[15px] font-semibold text-fg">{result.filename}</p>
        <dl className="mt-4 divide-y divide-line">
          {rows.map(([label, value]) => (
            <div key={label} className="flex items-baseline justify-between gap-6 py-2.5">
              <dt className="shrink-0 text-[12px] text-muted">{label}</dt>
              <dd className="min-w-0 truncate text-right font-mono text-[11.5px] text-fg">
                {value}
              </dd>
            </div>
          ))}
        </dl>
        <div className="mt-4 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="h-8 rounded-[10px] bg-accent px-4 text-[12px] font-medium text-accent-fg"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

function matchLabel(matchType: SearchResult["match_type"]): string {
  switch (matchType) {
    case "FileName":
      return "file name";
    case "Content":
      return "file content";
    case "ContentFolded":
      return "content (fuzzy)";
    case "Semantic":
      return "meaning";
    case "Both":
      return "name and content";
    default:
      return String(matchType);
  }
}
