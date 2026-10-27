import React from "react";

import { SearchResult } from "../types";
import { FileTypeIcon } from "./FileTypeIcon";
import { HighlightedName } from "./EmptyState";
import { isThumbImage, useThumbnail, THUMBNAIL_LAZY_LIMIT } from "../lib/thumbnails";
import {
  formatBytes,
  formatRelativeTime,
  queryTerms,
  highlightSegments,
} from "../lib/format";

/* Result row: fixed 56px, tinted type square, name with subtle match
   highlight, mono meta line, hover-only actions + chevron.
   Grid card: fluid 1fr tile, aspect-square enforced so every row of the
   keyboard grid measures true squares — image cards fill the tile with
   the thumbnail under a scrim, classic cards use the tinted-square icon. */

export type ResultView = "list" | "grid";

interface ResultItemProps {
  result: SearchResult;
  query: string;
  index: number;
  isSelected: boolean;
  view: ResultView;
  onSelect: () => void;
  onOpen: () => void;
  onReveal: () => void;
  onCopyPath: () => void;
  onContextMenu?: (x: number, y: number) => void;
}

function MatchBadge({ matchType }: { matchType: SearchResult["match_type"] }) {
  const badge = (() => {
    switch (matchType) {
      case "Content":
        return "content";
      case "ContentFolded":
        return "fuzzy content";
      case "Both":
        return "name + content";
      case "Semantic":
        return "meaning";
      case "Visual":
        return "visual";
      default:
        return null;
    }
  })();
  if (!badge) return null;
  return (
    <span
      className="flex h-4 shrink-0 items-center rounded-full bg-accent-soft px-1.5 font-mono text-[9px] leading-none text-accent"
      title={
        matchType === "Semantic"
          ? "Found via semantic similarity only"
          : matchType === "ContentFolded"
            ? "Found via spelling/OCR-tolerant content matching"
            : "Query matched inside the file contents"
      }
    >
      {badge}
    </span>
  );
}

const RESERVED_SUBLINE = "mt-0.5 h-[15px] truncate font-mono text-[11px] leading-[15px] text-subtle";

export const ResultItem = React.memo<ResultItemProps>(
  ({ result, query, index, isSelected, view, onSelect, onOpen, onContextMenu }) => {

    /* Thumbnails only for image files inside the initial visible window;
       the row is a fixed 56px square either way, so load/failure can't shift
       layout — the icon stays until pixels are on screen. */
    const imageEligible =
      !result.is_dir && isThumbImage(result.filename) && index < THUMBNAIL_LAZY_LIMIT;
    const { src: thumbSrc, failed: thumbFailed } = useThumbnail(result.path, imageEligible);


    // 20ms stagger on the first 8 rows only.
    const stagger =
      view === "list" && index < 8
        ? { ["--stagger-delay" as string]: `${index * 20}ms` }
        : undefined;

    // All list rows are exactly 56px: the snippet (when present) renders in
    // the same reserved sub-line slot as the meta line — zero layout shift
    // as results stream in.
    const hasSnippet = Boolean(result.snippet) && view === "list";

    if (view === "grid") {
      const ext =
        !result.is_dir && result.filename.includes(".")
          ? (result.filename.split(".").pop() ?? "").toLowerCase()
          : "";

      /* Image files get a thumbnail-forward card: the image fills the whole
         square tile under a bottom scrim that carries the name + size in
         light text (companion to the scrim gradient — raw white, tokens
         don't ship an always-light ink). Falls back to the classic icon
         card the moment (or if) the thumbnail can't resolve. */
      if (imageEligible && !thumbFailed) {
        return (
          <button
            type="button"
            onClick={onSelect}
            onDoubleClick={onOpen}
            className={`group relative row-enter aspect-square w-full overflow-hidden rounded-[14px] border text-left transition-colors duration-150 ${
              isSelected
                ? "border-line"
                : "border-line hover:border-line-strong"
            }`}
          >
            <span aria-hidden={!thumbSrc} className="absolute inset-0 block bg-sunken">
              {thumbSrc && (
                <img
                  src={thumbSrc}
                  alt=""
                  draggable={false}
                  className="fade-in absolute inset-0 h-full w-full object-cover transition-transform duration-[180ms] group-hover:scale-[1.02]"
                />
              )}
            </span>
            <span
              aria-hidden="true"
              className="absolute inset-0"
              style={{ background: "linear-gradient(to top, rgba(0,0,0,.72), transparent 55%)" }}
            />
            {/* Selection ring paints above the image — an inset box-shadow on
                the button itself would end up beneath the thumbnail layers. */}
            {isSelected && (
              <span
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 z-10 rounded-[14px] ring-2 ring-inset ring-accent"
              />
            )}
            <span aria-hidden="true" className="absolute left-2 top-2 z-10">
              <FileTypeIcon result={result} size={24} />
            </span>
            <span className="absolute inset-x-0 bottom-0 z-10 p-2.5 pb-2">
              <p className="line-clamp-2 text-[12px] font-medium leading-[16px] text-white">
                <HighlightedName text={result.filename} query={query} />
              </p>
              <p className="truncate pt-0.5 font-mono text-[10px] leading-[14px] text-white/70">
                {formatBytes(result.file_size)}
              </p>
            </span>
          </button>
        );
      }

      /* Classic icon card: 28px tinted type square, 2-line name clamp in
         the middle, size/ext meta on the bottom row. 12px padding. */
      return (
        <button
          type="button"
          onClick={onSelect}
          onDoubleClick={onOpen}
          className={`${index < 8 ? "row-stagger" : "row-enter"} group flex aspect-square w-full flex-col rounded-[14px] border p-3 text-left transition-colors duration-150 ${
            isSelected
              ? "border-line bg-accent-soft ring-2 ring-inset ring-accent"
              : "border-line bg-elevated hover:border-line-strong hover:bg-sunken"
          }`}
        >
          <FileTypeIcon result={result} size={28} />
          <span className="flex min-h-0 w-full flex-1 items-center pt-1">
            <p className="line-clamp-2 text-[12px] font-medium leading-[16px] text-fg">
              <HighlightedName text={result.filename} query={query} />
            </p>
          </span>
          <span className="flex w-full items-center justify-between gap-1">
            <span className="truncate font-mono text-[10px] leading-[14px] text-subtle">
              {result.is_dir ? "folder" : formatBytes(result.file_size)}
            </span>
            {ext && (
              <span className="shrink-0 font-mono text-[10px] leading-[14px] text-subtle">
                {ext}
              </span>
            )}
          </span>
        </button>
      );
    }

    return (
      <div
        style={stagger}
        onClick={onSelect}
        onDoubleClick={onOpen}
        onContextMenu={(e) => {
          if (!onContextMenu) return;
          e.preventDefault();
          onSelect();
          onContextMenu(e.clientX, e.clientY);
        }}
        role="button"
        tabIndex={-1}
        className={`
          group relative flex ${index < 8 ? "row-stagger" : "row-enter"}
          h-[60px] mx-1
          cursor-pointer select-none items-center gap-3 overflow-hidden rounded-[12px] px-3
          transition-colors duration-150
          ${
            isSelected
              ? "bg-sunken"
              : "bg-transparent hover:bg-sunken/60"
          }
        `}
      >
        {thumbSrc ? (
          <img
            src={thumbSrc}
            alt=""
            draggable={false}
            className="fade-in h-8 w-8 shrink-0 rounded-lg border border-line object-cover"
          />
        ) : (
          <FileTypeIcon result={result} />
        )}

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="truncate text-[13px] font-medium leading-[18px] text-fg">
              <HighlightedName text={result.filename} query={query} />
            </span>
            <MatchBadge matchType={result.match_type} />
            {(result.group_count ?? 1) > 1 && (
              <span
                title={(result.group_paths ?? []).join("\n")}
                className="shrink-0 rounded-full bg-sunken px-1.5 py-0.5 font-mono text-[9px] text-muted"
              >
                {result.group_count} versions
              </span>
            )}
          </div>
          {hasSnippet ? (
            <p className={RESERVED_SUBLINE}>
              {highlightSegments(result.snippet!, queryTerms(query))
                .slice(0, 4)
                .map((s, i) =>
                  s.match ? (
                    <mark key={i} className="rounded-sm bg-accent-soft px-0.5 text-fg">
                      {s.text}
                    </mark>
                  ) : (
                    <span key={i}>{s.text}</span>
                  ),
                )}
            </p>
          ) : (
            <p className={RESERVED_SUBLINE}>
              {[
                result.path,
                !result.is_dir && result.file_size > 0 ? formatBytes(result.file_size) : null,
                formatRelativeTime(result.modified) || null,
              ]
                .filter(Boolean)
                .join("  ·  ")}
            </p>
          )}
        </div>

        {/* Hover card: the facts at a glance. Actions and full details are on right-click. */}
        <span
          aria-hidden="true"
          className="pointer-events-none absolute right-3 top-full z-20 mt-1 hidden w-[280px] rounded-[12px] border border-line bg-elevated p-3 text-left shadow-lg group-hover:block"
        >
          <span className="block truncate font-mono text-[11px] text-fg">{result.path}</span>
          <span className="mt-1.5 flex gap-3 text-[11px] text-muted">
            <span>{result.is_dir ? "Folder" : formatBytes(result.file_size)}</span>
            <span>{formatRelativeTime(result.modified)}</span>
          </span>
        </span>
      </div>
    );
  },
  (prev, next) =>
    prev.result.path === next.result.path &&
    prev.result.snippet === next.result.snippet &&
    prev.result.match_type === next.result.match_type &&
    prev.result.group_count === next.result.group_count &&
    prev.isSelected === next.isSelected &&
    prev.view === next.view &&
    prev.query === next.query &&
    prev.index === next.index,
);
