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
    <div className="mx-4 mb-1 shrink-0 overflow-hidden rounded-[14px] border border-line bg-elevated shadow-sm">
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
            className={`flex w-full items-center gap-3 px-3 py-2 text-left transition-colors ${
              active ? "bg-sunken" : "hover:bg-sunken"
            }`}
          >
            <Folder className={`h-3.5 w-3.5 shrink-0 ${active ? "text-accent" : "text-subtle"}`} />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12px] font-medium text-fg">@{name}</span>
              <span className="block truncate font-mono text-[10px] text-subtle">{path}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}