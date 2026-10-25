import React from "react";
import { ChevronRight, Copy, Check, ExternalLink, FolderOpen } from "lucide-react";
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
   Grid card: 160px, 2-line name clamp, reserved meta line — no layout
   shift when selection or hover state changes. */

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
}

function MatchBadge({ matchType }: { matchType: SearchResult["match_type"] }) {
  const badge = (() => {
    switch (matchType) {
      case "Content":
        return "content";
      case "Both":
        return "name + content";
      case "Semantic":
        return "≈ semantic";
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
          : "Query matched inside the file contents"
      }
    >
      {badge}
    </span>
  );
}

const RESERVED_SUBLINE = "mt-0.5 h-[15px] truncate font-mono text-[11px] leading-[15px] text-subtle";

export const ResultItem = React.memo<ResultItemProps>(
  ({ result, query, index, isSelected, view, onSelect, onOpen, onReveal, onCopyPath }) => {
    const [copied, setCopied] = React.useState(false);

    /* Thumbnails only for image files inside the initial visible window;
       the row is a fixed 56px square either way, so load/failure can't shift
       layout — the icon stays until pixels are on screen. */
    const imageEligible =
      !result.is_dir && isThumbImage(result.filename) && index < THUMBNAIL_LAZY_LIMIT;
    const { src: thumbSrc, failed: thumbFailed } = useThumbnail(result.path, imageEligible);

    const handleCopy = (e: React.MouseEvent) => {
      e.stopPropagation();
      onCopyPath();
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    };

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
      /* Image files get a thumbnail-forward card: 128px image area on top,
         one-line name, meta line. Falls back to the classic icon card the
         moment (or if) the thumbnail can't resolve. */
      if (imageEligible && !thumbFailed) {
        return (
          <button
            type="button"
            onClick={onSelect}
            onDoubleClick={onOpen}
            style={stagger}
            className={`${index < 8 ? "row-stagger" : "row-enter"} group relative flex h-[192px] w-[160px] flex-col gap-1.5 rounded-[14px] border p-2.5 text-left transition-colors duration-150 ${
              isSelected
                ? "border-accent bg-accent-soft"
                : "border-line bg-elevated hover:border-line-strong hover:bg-sunken"
            }`}
          >
            <span
              aria-hidden={thumbSrc ? undefined : "true"}
              className="block h-[128px] w-full shrink-0 overflow-hidden rounded-[10px] border border-line bg-sunken"
            >
              {thumbSrc && (
                <img
                  src={thumbSrc}
                  alt=""
                  draggable={false}
                  className="fade-in h-full w-full object-cover transition-transform duration-200 group-hover:scale-[1.04]"
                />
              )}
            </span>
            <p className="truncate text-[12px] font-medium leading-[16px] text-fg">
              <HighlightedName text={result.filename} query={query} />
            </p>
            <div className="flex items-center justify-between gap-1">
              <span className="truncate font-mono text-[10px] leading-[15px] text-subtle">
                {formatBytes(result.file_size)}
              </span>
              <ChevronRight className="h-3.5 w-3.5 text-subtle opacity-0 transition-opacity duration-150 group-hover:opacity-100" />
            </div>
          </button>
        );
      }

      return (
        <button
          type="button"
          onClick={onSelect}
          onDoubleClick={onOpen}
          style={stagger}
          className={`${index < 8 ? "row-stagger" : "row-enter"} group relative flex h-[160px] w-[160px] flex-col gap-2 rounded-[14px] border p-3 text-left transition-colors duration-150 ${
            isSelected
              ? "border-accent bg-accent-soft"
              : "border-line bg-elevated hover:border-line-strong hover:bg-sunken"
          }`}
        >
          <FileTypeIcon result={result} size={40} />
          <div className="min-h-0 flex-1">
            <p className="line-clamp-2 text-[12px] font-medium leading-[16px] text-fg">
              <HighlightedName text={result.filename} query={query} />
            </p>
          </div>
          <div className="flex items-center justify-between gap-1">
            <span className="truncate font-mono text-[10px] text-subtle">
              {result.is_dir ? "folder" : formatBytes(result.file_size)}
            </span>
            <ChevronRight className="h-3.5 w-3.5 text-subtle opacity-0 transition-opacity duration-150 group-hover:opacity-100" />
          </div>
        </button>
      );
    }

    return (
      <div
        style={stagger}
        onClick={onSelect}
        onDoubleClick={onOpen}
        role="button"
        tabIndex={-1}
        className={`
          group relative flex ${index < 8 ? "row-stagger" : "row-enter"}
          h-14
          cursor-pointer select-none items-center gap-3 overflow-hidden rounded-[10px] border-l-2 px-3
          transition-colors duration-150
          ${
            isSelected
              ? "border-accent bg-accent-soft"
              : "border-transparent bg-transparent hover:bg-sunken"
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

        {/* Hover-only actions. Space is reserved via opacity, never width. */}
        <div className="flex shrink-0 items-center gap-0.5">
          <div className="flex items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100">
            <button
              type="button"
              title="Reveal in file manager (Shift+Enter)"
              onClick={(e) => {
                e.stopPropagation();
                onReveal();
              }}
              className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-elevated hover:text-fg"
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              title="Copy path"
              onClick={handleCopy}
              className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-elevated hover:text-fg"
            >
              {copied ? <Check className="h-3.5 w-3.5 text-accent" /> : <Copy className="h-3.5 w-3.5" />}
            </button>
            <button
              type="button"
              title="Open (Enter)"
              onClick={(e) => {
                e.stopPropagation();
                onOpen();
              }}
              className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-elevated hover:text-fg"
            >
              <ExternalLink className="h-3.5 w-3.5" />
            </button>
          </div>
          <ChevronRight
            className={`h-4 w-4 shrink-0 text-subtle transition-opacity duration-150 ${
              isSelected ? "opacity-100" : "opacity-0 group-hover:opacity-60"
            }`}
          />
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.result.path === next.result.path &&
    prev.result.snippet === next.result.snippet &&
    prev.result.match_type === next.result.match_type &&
    prev.isSelected === next.isSelected &&
    prev.view === next.view &&
    prev.query === next.query &&
    prev.index === next.index,
);
