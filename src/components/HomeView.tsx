import { Pin, Search, Star } from "lucide-react";
import type { HomeData, HomeItem } from "../types";

function fmtAge(secs: number): string {
  if (!secs) return "";
  const age = Math.max(0, Date.now() / 1000 - secs);
  if (age < 3600) return `${Math.max(1, Math.round(age / 60))}m ago`;
  if (age < 86400) return `${Math.round(age / 3600)}h ago`;
  if (age < 2592000) return `${Math.round(age / 86400)}d ago`;
  return new Date(secs * 1000).toLocaleDateString();
}

function ItemRow({
  item,
  onOpen,
  onPin,
  pinned,
}: {
  item: HomeItem;
  onOpen: () => void;
  onPin?: () => void;
  pinned?: boolean;
}) {
  const name = item.filename || item.path;
  return (
    <div className="group flex items-center gap-2 rounded-[10px] px-2 py-1.5 transition-colors hover:bg-sunken">
      <button
        type="button"
        onClick={onOpen}
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
      >
        <span className="min-w-0">
          <span className="block truncate text-[12px] font-medium text-fg">{name}</span>
          <span className="block truncate font-mono text-[10px] text-subtle">{item.path}</span>
        </span>
      </button>
      <span className="shrink-0 font-mono text-[10px] text-subtle">{fmtAge(item.modified)}</span>
      {onPin && (
        <button
          type="button"
          title={pinned ? "Unpin" : "Pin"}
          onClick={onPin}
          className={`flex h-6 w-6 shrink-0 items-center justify-center rounded transition-colors ${
            pinned ? "text-accent" : "text-subtle opacity-0 group-hover:opacity-100"
          } hover:bg-elevated`}
        >
          <Pin className="h-3 w-3" />
        </button>
      )}
    </div>
  );
}

export function HomeView({
  data,
  onSearch,
  onOpen,
  onTogglePin,
}: {
  data: HomeData | null;
  onSearch: (query: string, sort?: string) => void;
  onOpen: (path: string) => void;
  onTogglePin: (path: string) => void;
}) {
  if (!data) return null;
  const pinned = new Set(data.pins.map((p) => p.path));

  return (
    <div className="mx-auto grid max-w-4xl gap-5 p-6 md:grid-cols-2">
      <section>
        <h2 className="eyebrow pb-2">Pinned</h2>
        {data.pins.length === 0 ? (
          <p className="px-2 py-1 text-[11px] text-subtle">
            Pin a result with the pin action to keep it here.
          </p>
        ) : (
          <div className="space-y-0.5">
            {data.pins.map((p) => (
              <ItemRow
                key={p.path}
                item={p}
                pinned
                onOpen={() => onOpen(p.path)}
                onPin={() => onTogglePin(p.path)}
              />
            ))}
          </div>
        )}

        <h2 className="eyebrow pb-2 pt-5">Recent</h2>
        <div className="space-y-0.5">
          {data.recent.map((r) => (
            <ItemRow
              key={r.path}
              item={r}
              onOpen={() => onOpen(r.path)}
              onPin={() => onTogglePin(r.path)}
              pinned={pinned.has(r.path)}
            />
          ))}
          {data.recent.length === 0 && (
            <p className="px-2 py-1 text-[11px] text-subtle">Nothing indexed yet.</p>
          )}
        </div>
      </section>

      <section>
        <h2 className="eyebrow pb-2">Saved searches</h2>
        {data.saved_searches.length === 0 ? (
          <p className="px-2 py-1 text-[11px] text-subtle">
            Save the current query with the book button next to Sort.
          </p>
        ) : (
          <div className="space-y-0.5">
            {data.saved_searches.map((s, i) => (
              <button
                key={`${s.name}-${i}`}
                type="button"
                onClick={() => onSearch(s.query, s.sort ?? "best")}
                className="flex w-full items-center gap-2 rounded-[10px] px-2 py-1.5 text-left transition-colors hover:bg-sunken"
              >
                <Star className="h-3 w-3 shrink-0 text-accent" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[12px] font-medium text-fg">{s.name}</span>
                  <span className="block truncate font-mono text-[10px] text-subtle">{s.query}</span>
                </span>
              </button>
            ))}
          </div>
        )}

        <h2 className="eyebrow pb-2 pt-5">Try</h2>
        <div className="flex flex-wrap gap-1.5">
          {[
            "latest resume",
            "screenshots from last week",
            "big videos in Projects",
            "/dupes",
          ].map((q) => (
            <button
              key={q}
              type="button"
              onClick={() => onSearch(q)}
              className="flex h-7 items-center gap-1.5 rounded-full border border-line px-3 text-[11px] text-muted transition-colors hover:border-line-strong hover:text-fg"
            >
              <Search className="h-3 w-3" />
              {q}
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}