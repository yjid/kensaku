import { Folder } from "lucide-react";

/* `@` scope menu — resolves a typed prefix against indexed directories.
   Picking one inserts an `in:"<path>"` filter into the query. */

export function ScopeMenu({
  items,
  activeIndex,
  onPick,
}: {
  items: string[];
  activeIndex: number;
  onPick: (path: string) => void;
}) {
  if (items.length === 0) return null;
  return (
    <div className="elevated menu-pop menu-pop-center relative z-20 mx-8 mb-2 mt-3 shrink-0 overflow-hidden p-1.5">
      {items.map((path, i) => {
        const active = i === activeIndex;
        const name = path.split(/[\\/]/).filter(Boolean).pop() || path;
        return (
          <button
            key={path}
            type="button"
            onMouseDown={(e) => {
              e.preventDefault();
              onPick(path);
            }}
            className={`flex w-full items-center gap-3 rounded-control px-3 py-2 text-left transition-colors ${
              active ? "bg-ink-soft" : "hover:bg-accent-soft active:bg-ink-soft"
            }`}
          >
            <Folder className={`h-4 w-4 shrink-0 ${active ? "text-ink" : "text-subtle"}`} />
            <span className="min-w-0 flex-1">
              <span className="t-body block truncate font-medium text-fg">@{name}</span>
              <span className="t-meta block truncate text-subtle">{path}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}