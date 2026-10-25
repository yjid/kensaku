import React, { useEffect, useMemo, useRef, useState } from "react";
import { Search, X, CornerDownLeft, ArrowUp, ArrowDown } from "lucide-react";

/* One palette overlay reused everywhere:
   - Global command palette (⌘K): Actions / Navigate / Filters / Settings groups.
   - Result action palette (⌘K on a selection): "File actions" group first.

   Groups are filtered client-side, arrow-nav works across flattened
   visible rows, Enter runs, Esc closes, mouse hover synchronizes. */

export interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  shortcut?: string[];
  icon?: React.ComponentType<{ className?: string }>;
  run: () => void;
}

export interface PaletteGroup {
  label: string;
  items: PaletteItem[];
}

interface PaletteProps {
  title: string;
  placeholder: string;
  groups: PaletteGroup[];
  onClose: () => void;
}

export const Palette: React.FC<PaletteProps> = ({ title, placeholder, groups, onClose }) => {
  const [filter, setFilter] = useState("");
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const flat = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return groups
      .map((g) => ({
        label: g.label,
        items: f
          ? g.items.filter((i) => i.label.toLowerCase().includes(f) || (i.hint ?? "").toLowerCase().includes(f))
          : g.items,
      }))
      .filter((g) => g.items.length > 0);
  }, [groups, filter]);

  const rows = useMemo(
    () => flat.flatMap((g) => g.items.map((item) => ({ group: g.label, item }))),
    [flat],
  );

  useEffect(() => {
    setActive(0);
  }, [filter]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-row="${active}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [active]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => Math.min(i + 1, rows.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        rows[active]?.item.run();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rows, active, onClose]);

  return (
    <div
      className="absolute inset-0 z-50 flex items-start justify-center p-6 backdrop-blur-[2px]"
      style={{ background: "var(--scrim)" }}
      onMouseDown={onClose}
    >
      <div
        className="overlay-in mt-12 flex w-full max-w-[560px] flex-col overflow-hidden rounded-[14px] border border-line-strong bg-elevated"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex h-12 shrink-0 items-center gap-2.5 border-b border-line px-3.5">
          <Search className="h-4 w-4 shrink-0 text-subtle" />
          <input
            ref={inputRef}
            type="text"
            value={filter}
            spellCheck={false}
            autoComplete="off"
            placeholder={placeholder}
            onChange={(e) => setFilter(e.target.value)}
            className="min-w-0 flex-1 bg-transparent text-[13px] text-fg outline-none placeholder:text-subtle"
          />
          {filter && (
            <button
              type="button"
              title="Clear"
              onClick={() => setFilter("")}
              className="flex h-6 w-6 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-fg"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
          <kbd className="kbd">esc</kbd>
        </div>

        {/* Grouped list */}
        <div ref={listRef} className="max-h-[380px] min-h-[60px] overflow-y-auto p-1.5">
          {rows.length === 0 ? (
            <p className="px-3 py-6 text-center text-[12px] text-subtle">No matching commands</p>
          ) : (
            flat.map((g) => (
              <div key={g.label} className="mb-1 last:mb-0">
                <div className="eyebrow px-2.5 pb-1 pt-2">{g.label}</div>
                {g.items.map((item) => {
                  const idx = rows.findIndex((r) => r.item.id === item.id);
                  const isActive = idx === active;
                  const Icon = item.icon;
                  return (
                    <div
                      key={item.id}
                      data-row={idx}
                      onMouseMove={() => {
                        if (!isActive && idx >= 0) setActive(idx);
                      }}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => {
                        item.run();
                        onClose();
                      }}
                      className={`flex h-9 items-center gap-2.5 rounded-[10px] px-2.5 text-[13px] transition-colors duration-150 ${
                        isActive
                          ? "bg-accent-soft text-fg"
                          : "text-muted hover:bg-sunken hover:text-fg"
                      }`}
                    >
                      {Icon ? (
                        <Icon className="h-4 w-4 shrink-0" />
                      ) : (
                        <span className="h-4 w-4 shrink-0" />
                      )}
                      <span className="min-w-0 flex-1 truncate text-fg">{item.label}</span>
                      {item.hint && (
                        <span className="shrink-0 truncate font-mono text-[10px] text-subtle">
                          {item.hint}
                        </span>
                      )}
                      {item.shortcut && (
                        <span className="flex shrink-0 items-center gap-0.5">
                          {item.shortcut.map((k) => (
                            <kbd key={k} className="kbd">
                              {k}
                            </kbd>
                          ))}
                        </span>
                      )}
                      {isActive && <CornerDownLeft className="h-3 w-3 shrink-0 text-subtle" />}
                    </div>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="flex h-8 shrink-0 items-center justify-between border-t border-line bg-sunken px-3 text-[10px] text-subtle">
          <span>{title}</span>
          <span className="flex items-center gap-2">
            <span className="flex items-center gap-0.5">
              <ArrowUp className="h-3 w-3" />
              <ArrowDown className="h-3 w-3" />
              navigate
            </span>
            <span className="flex items-center gap-0.5">
              <CornerDownLeft className="h-3 w-3" />
              run
            </span>
          </span>
        </div>
      </div>
    </div>
  );
};
