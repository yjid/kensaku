import { Search } from "lucide-react";
import type { HomeData } from "../types";
import { FileRow } from "./FileRow";

/* Home shows recent files as a clean list, the same rows as list results.
   With nothing recent yet, it offers a few starting searches instead. */

const SUGGESTIONS = ["latest resume", "screenshots from last week", "big videos", "invoice"];

export function HomeView({
  data,
  onSearch,
  onOpen,
  showThumbs,
}: {
  data: HomeData | null;
  onSearch: (query: string) => void;
  onOpen: (path: string) => void;
  showThumbs?: boolean;
}) {
  if (!data) return null;
  const recent = data.recent.slice(0, 30);

  if (recent.length > 0) {
    return (
      <div className="mx-auto w-full max-w-4xl px-3 py-4">
        <p className="px-3 pb-2 text-[11px] font-medium uppercase tracking-[0.06em] text-muted">Recent</p>
        <div className="flex flex-col gap-0.5">
          {recent.map((r) => (
            <FileRow
              key={r.path}
              item={{ path: r.path, filename: r.filename || r.path, file_size: r.size, modified: r.modified, is_dir: r.is_dir }}
              showThumbs={showThumbs}
              onSelect={() => onOpen(r.path)}
              onOpen={() => onOpen(r.path)}
            />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto flex w-full max-w-md flex-col items-center gap-4 p-8 text-center">
      <p className="text-[13px] text-muted">Try a search</p>
      <div className="flex flex-wrap justify-center gap-2">
        {SUGGESTIONS.map((q) => (
          <button
            key={q}
            type="button"
            onClick={() => onSearch(q)}
            className="btn btn-secondary h-8 gap-1.5 rounded-full px-3 text-[12px] text-muted hover:text-fg"
          >
            <Search className="h-3 w-3" />
            {q}
          </button>
        ))}
      </div>
    </div>
  );
}
