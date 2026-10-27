import React from "react";

/* Bottom key-hint strip shown on the main search surface. */

const HINTS: { keys: string; label: string }[] = [
  { keys: "Enter", label: "open" },
  { keys: "Space", label: "preview" },
  { keys: "Shift+Enter", label: "show in folder" },
  { keys: "⌘/Ctrl+C", label: "copy path" },
  { keys: "⌘/Ctrl+I", label: "details" },
  { keys: "Esc", label: "clear / hide" },
];

export const KeyHintStrip: React.FC = () => {
  return (
    <div className="flex h-7 shrink-0 items-center justify-center gap-4 border-t border-line px-4 text-[11px] text-subtle">
      {HINTS.map((h) => (
        <span key={h.label} className="flex items-center gap-1">
          <kbd className="rounded-[4px] border border-line px-1.5 py-0.5 font-mono text-[10px] text-muted">
            {h.keys}
          </kbd>
          <span>{h.label}</span>
        </span>
      ))}
    </div>
  );
};
