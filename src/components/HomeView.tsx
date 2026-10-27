import { Search } from "lucide-react";
import type { HomeData, HomeItem } from "../types";
import { FileTypeIcon } from "./FileTypeIcon";
import { formatBytes, formatRelativeTime } from "../lib/format";

/* Home shows one thing: recent files as the same square cards the results
   use. With nothing recent yet, it offers a few starting searches instead. */

const SUGGESTIONS = [
  "latest resume",
  "screenshots from last week",
  "big videos",
  "invoice",
];

function RecentCard({ item, onOpen }: { item: HomeItem; onOpen: () => void }) {
  const name = item.filename || item.path;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="group flex aspect-square w-full flex-col justify-between rounded-[16px] border border-line bg-elevated p-3 text-left transition-[transform,border-color] duration-200 [transition-timing-function:var(--ease-apple)] hover:-translate-y-px hover:border-line-strong"
    >
      <FileTypeIcon result={{ is_dir: item.is_dir, filename: name }} size={40} />
      <span className="min-w-0">
        <span className="line-clamp-2 block text-[13px] font-medium leading-[17px] text-fg">{name}</span>
        <span className="block truncate font-mono text-[11px] leading-[15px] text-muted">
          {item.is_dir ? "Folder" : formatBytes(item.size)} · {formatRelativeTime(item.modified)}
        </span>
      </span>
    </button>
  );
}

export function HomeView({
  data,
  onSearch,
  onOpen,
}: {
  data: HomeData | null;
  onSearch: (query: string) => void;
  onOpen: (path: string) => void;
}) {
  if (!data) return null;
  const recent = data.recent.slice(0, 12);

  if (recent.length > 0) {
    return (
      <div className="mx-auto w-full max-w-5xl p-4">
        <p className="px-1 pb-3 text-[12px] font-medium text-muted">Recent</p>
        <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
          {recent.map((r) => (
            <RecentCard key={r.path} item={r} onOpen={() => onOpen(r.path)} />
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
            className="flex h-8 items-center gap-1.5 rounded-full border border-line px-3 text-[12px] text-muted transition-colors duration-150 hover:border-line-strong hover:text-fg"
          >
            <Search className="h-3 w-3" />
            {q}
          </button>
        ))}
      </div>
    </div>
  );
}
