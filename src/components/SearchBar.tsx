import React, { useEffect, useRef, useState } from "react";
import { X, Terminal, Search } from "lucide-react";

/* Search capsule. One floating control, Spotlight-style: glyph, 22px query,
   result count, clear, settings. Solid surface,
   14px corners. A hairline runs along its base while a search is live. */

interface SearchBarProps {
  query: string;
  setQuery: (q: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  isSearching: boolean;
  onFocusInput: () => void;
  resultCount: number;
  /** Total matches before the display limit, when known. */
  total?: number;
  /** Icon buttons rendered at the right end of the capsule (settings). */
  trailing?: React.ReactNode;
}

const PLACEHOLDERS = ["Search everything", "invoice from last week", "type:pdf in:~/Projects", "screenshots with a chart"];

export function SearchBar({
  query,
  setQuery,
  inputRef,
  isSearching,
  onFocusInput,
  resultCount,
  total,
  trailing,
}: SearchBarProps) {
  const [phIdx, setPhIdx] = useState(0);
  const rotateRef = useRef(false);

  useEffect(() => {
    rotateRef.current = query === "";
    if (!query) {
      const id = window.setInterval(() => {
        if (rotateRef.current) setPhIdx((i) => (i + 1) % PLACEHOLDERS.length);
      }, 5000);
      return () => window.clearInterval(id);
    }
  }, [query]);

  const isCommandMode = query.startsWith(">") || query.startsWith("/");
  const count = total && total > resultCount ? total : resultCount;
  const Glyph = isCommandMode ? Terminal : Search;

  return (
    <div data-tauri-drag-region className="px-4 pb-2 pt-3">
      <div className="control flex h-12 items-center gap-3 overflow-hidden rounded-field pl-4 pr-1.5 transition-shadow duration-200 focus-within:[box-shadow:var(--control-edge),var(--control-shadow),0_0_0_3px_var(--ink-soft)]">
        <Glyph className="h-5 w-5 shrink-0 text-muted" strokeWidth={1.75} />
        <input
          ref={inputRef}
          type="text"
          value={query}
          spellCheck={false}
          autoComplete="off"
          autoFocus
          onChange={(e) => setQuery(e.target.value)}
          placeholder={isCommandMode ? "Command" : PLACEHOLDERS[phIdx]}
          onFocus={onFocusInput}
          aria-label="Search"
          className="min-w-0 flex-1 bg-transparent text-[19px] font-medium leading-none tracking-[-0.022em] text-fg outline-none placeholder:font-normal placeholder:text-subtle"
          style={{ caretColor: "var(--blue)" }}
        />

        {query && !isCommandMode && !isSearching && (
          <span key={count} className="t-meta fade-in shrink-0 text-subtle">
            {count === 0 ? "No results" : `${count.toLocaleString("en-US")} ${count === 1 ? "result" : "results"}`}
          </span>
        )}

        {query && (
          <button
            type="button"
            title="Clear (Esc)"
            onClick={() => {
              setQuery("");
              inputRef.current?.focus();
            }}
            className="btn btn-ghost btn-icon h-9 w-9 shrink-0"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        )}

        {trailing && <div className="flex shrink-0 items-center">{trailing}</div>}

        {isSearching && (
          <div className="pointer-events-none absolute inset-x-8 bottom-0 h-px overflow-hidden">
            <div className="progress-bar" />
          </div>
        )}
      </div>
    </div>
  );
}
