import type { HomeData } from "../types";
import { formatRelativeTime } from "../lib/format";
import { isThumbImage, useThumbnail } from "../lib/thumbnails";
import { TypeTile } from "./TypeTile";

/* Home. The field above is the invitation; below it, what you touched
   last, as tiles. Saved searches sit on one quiet line above. With nothing
   recent, a single serif line and a few starting searches. */

const SUGGESTIONS = ["latest resume", "screenshots from last week", "big videos", "invoice"];

function RecentTile({
  item,
  index,
  showThumbs,
  onOpen,
}: {
  item: HomeData["recent"][number];
  index: number;
  showThumbs?: boolean;
  onOpen: () => void;
}) {
  const eligible = Boolean(showThumbs) && !item.is_dir && isThumbImage(item.filename);
  const { src, failed } = useThumbnail(item.path, eligible);
  const name = item.filename || item.path;
  return (
    <button
      type="button"
      onClick={onOpen}
      title={item.path}
      style={{ ["--stagger-delay" as string]: `${Math.min(index, 11) * 32}ms` }}
      className="group card-in flex min-w-0 flex-col text-left outline-none"
    >
      <span className="surface relative block aspect-[4/3] w-full overflow-hidden transition-[transform,box-shadow] duration-[280ms] [transition-timing-function:var(--ease)] group-hover:-translate-y-[3px] group-hover:[box-shadow:var(--panel-edge),var(--shadow-float)] group-focus-visible:outline-2 group-focus-visible:outline-offset-[3px] group-focus-visible:outline-ink">
        {eligible && src && !failed ? (
          <img src={src} alt="" draggable={false} className="fade-in h-full w-full object-cover transition-transform duration-[520ms] [transition-timing-function:var(--ease)] group-hover:scale-[1.04]" />
        ) : (
          <TypeTile filename={name} isDir={item.is_dir} />
        )}
      </span>
      <span className="flex min-w-0 flex-col px-0.5 pt-3">
        <span className="truncate text-[13px] font-medium leading-[18px] text-fg">{name}</span>
        <span className="t-meta truncate text-subtle">{formatRelativeTime(item.modified)}</span>
      </span>
    </button>
  );
}

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
  const recent = data.recent.slice(0, 18);
  const saved = data.saved_searches ?? [];

  if (recent.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-start justify-end gap-6 px-8 pb-16">
        <p className="t-moment rise-in max-w-md text-muted">Ask for a file the way you'd describe it to a friend.</p>
        <div className="flex flex-wrap gap-1.5">
          {SUGGESTIONS.map((q, i) => (
            <button
              key={q}
              type="button"
              onClick={() => onSearch(q)}
              style={{ ["--stagger-delay" as string]: `${120 + i * 50}ms` }}
              className="btn btn-secondary rise-in h-8 !rounded-full"
            >
              {q}
            </button>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="px-8 pb-10 pt-6">
      {saved.length > 0 && (
        <div className="mb-8 flex flex-wrap items-baseline gap-x-5 gap-y-2">
          <span className="t-meta text-subtle">Saved</span>
          {saved.map((s) => (
            <button
              key={s.name}
              type="button"
              onClick={() => onSearch(s.query)}
              className="text-[13px] text-muted underline decoration-line-strong decoration-1 underline-offset-4 transition-colors hover:text-fg hover:decoration-ink"
            >
              {s.name}
            </button>
          ))}
        </div>
      )}
      <p className="t-meta mb-4 text-subtle">Recent</p>
      <div className="grid gap-x-5 gap-y-6" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(188px, 1fr))" }}>
        {recent.map((r, i) => (
          <RecentTile key={r.path} item={r} index={i} showThumbs={showThumbs} onOpen={() => onOpen(r.path)} />
        ))}
      </div>
    </div>
  );
}
