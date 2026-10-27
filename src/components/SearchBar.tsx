import React, { useEffect, useRef, useState } from "react";
import { X, Terminal } from "lucide-react";

/* The query is the hero. No box, no icon: display-size type on the open
   field, with the result count set small to its right. A 1px ink line
   runs under it while a search is in flight. */

interface SearchBarProps {
  query: string;
  setQuery: (q: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  isSearching: boolean;
  onFocusInput: () => void;
  resultCount: number;
  /** Total matches before the display limit, when known. */
  total?: number;
  /** Icon buttons rendered to the right (settings). */
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

  return (
    <div data-tauri-drag-region className="relative z-10 shrink-0 px-8 pb-4 pt-7">
      <div data-tauri-drag-region className="flex items-center gap-4">
        {isCommandMode && <Terminal className="h-6 w-6 shrink-0 text-ink" strokeWidth={1.75} />}
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
          className="t-query min-w-0 flex-1 bg-transparent text-fg outline-none placeholder:text-subtle"
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
            className="btn btn-ghost btn-icon h-8 shrink-0 !rounded-full"
          >
            <X className="h-4 w-4" strokeWidth={1.75} />
          </button>
        )}

        {trailing && <div className="flex shrink-0 items-center">{trailing}</div>}
      </div>

      <div className="absolute inset-x-8 bottom-0 h-px bg-line">
        {isSearching && (
          <div className="progress-track">
            <div className="progress-bar" />
          </div>
        )}
      </div>
    </div>
  );
}
