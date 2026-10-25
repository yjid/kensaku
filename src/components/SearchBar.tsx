import React, { useEffect, useRef, useState } from "react";
import { Search, X, SlidersHorizontal, Terminal, LoaderCircle } from "lucide-react";

/* 60px search pill. 20px input, focus ring on the whole pill, right side:
   filter button + clear + Esc kbd hint. Placeholder rotates contextually. */

interface SearchBarProps {
  query: string;
  setQuery: (q: string) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  isSearching: boolean;
  onOpenPalette: () => void;
  onFocusInput: () => void;
  resultCount: number;
}

const PLACEHOLDERS = [
  "Search files, content, images…",
  "Try type:pdf in:~/Projects",
  "Search inside documents too…",
];

export function SearchBar({
  query,
  setQuery,
  inputRef,
  isSearching,
  onOpenPalette,
  onFocusInput,
  resultCount,
}: SearchBarProps) {
  const [phIdx, setPhIdx] = useState(0);
  const rotateRef = useRef(false);

  // Rotate placeholder slowly only while the input is empty & untouched.
  useEffect(() => {
    rotateRef.current = query === "";
    if (!query) {
      const id = window.setInterval(() => {
        if (rotateRef.current) setPhIdx((i) => (i + 1) % PLACEHOLDERS.length);
      }, 6000);
      return () => window.clearInterval(id);
    }
  }, [query]);

  const isCommandMode = query.startsWith(">");

  return (
    <div className="flex h-[60px] shrink-0 items-center gap-3 px-4">
      <div
        className={`flex h-11 min-w-0 flex-1 items-center gap-2.5 rounded-[14px] border bg-elevated px-3 transition-colors duration-150 ${
          isCommandMode
            ? "border-line-strong"
            : "border-line focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/30"
        }`}
      >
        {isCommandMode ? (
          <Terminal className="h-[18px] w-[18px] shrink-0 text-warning" />
        ) : (
          <Search className="h-[18px] w-[18px] shrink-0 text-subtle" />
        )}
        <input
          ref={inputRef}
          type="text"
          value={query}
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => setQuery(e.target.value)}
          placeholder={
            isCommandMode
              ? "kensaku command…"
              : resultCount > 0
                ? PLACEHOLDERS[0]
                : PLACEHOLDERS[phIdx]
          }
          onFocus={onFocusInput}
          className="min-w-0 flex-1 bg-transparent text-[20px] leading-none text-fg outline-none placeholder:text-fg-subtle caret-accent"
          style={{ letterSpacing: "-0.02em" }}
        />

        {isSearching && (
          <LoaderCircle className="h-4 w-4 shrink-0 animate-spin text-subtle" />
        )}

        {query && (
          <button
            type="button"
            title="Clear (Esc)"
            onClick={() => {
              setQuery("");
              inputRef.current?.focus();
            }}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-fg-subtle transition-colors duration-150 hover:bg-sunken hover:text-fg"
          >
            <X className="h-4 w-4" />
          </button>
        )}

        <button
          type="button"
          title="Filters & commands (⌘K)"
          onClick={onOpenPalette}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-fg-subtle transition-colors duration-150 hover:bg-sunken hover:text-fg"
        >
          <SlidersHorizontal className="h-4 w-4" />
        </button>

        <kbd className="kbd mr-1 shrink-0">esc</kbd>
      </div>
    </div>
  );
}
