import React from "react";
import { SearchResult } from "../types";
import { FileTypeIcon } from "./FileTypeIcon";
import { HighlightedName } from "./EmptyState";
import { isThumbImage, useThumbnail } from "../lib/thumbnails";
import { formatBytes, formatRelativeTime } from "../lib/format";
import { HoverDetails } from "./HoverDetails";

/* Result card. Grid only. Every card is a true square. Files with a
   thumbnail show it full-bleed; everything else shows its type glyph.
   Hovering reveals a small detail panel after a short pause, so moving
   across the grid stays quiet. Right-click opens the full menu. */

export type ResultView = "grid";

interface ResultItemProps {
  result: SearchResult;
  query: string;
  index: number;
  isSelected: boolean;
  view: ResultView;
  onSelect: () => void;
  onOpen: () => void;
  onReveal?: () => void;
  onCopyPath?: () => void;
  onContextMenu?: (x: number, y: number) => void;
  showThumbs?: boolean;
}

export const ResultItem = React.memo<ResultItemProps>(
  ({ result, query, isSelected, onSelect, onOpen, onReveal, onCopyPath, onContextMenu, showThumbs }) => {
    /* Thumbnails only for image files in the initial window. */
    // The virtual grid mounts only visible cards, so every image card may ask.
    const imageEligible = Boolean(showThumbs) && !result.is_dir && isThumbImage(result.filename);
    const { src: thumbSrc, failed: thumbFailed } = useThumbnail(result.path, imageEligible);
    const hasThumb = Boolean(thumbSrc) && !thumbFailed;

    const meta = result.is_dir
      ? "Folder"
      : `${formatBytes(result.file_size)} · ${formatRelativeTime(result.modified)}`;

    return (
      <button
        type="button"
        onClick={onSelect}
        onDoubleClick={onOpen}
        onContextMenu={(e) => {
          if (!onContextMenu) return;
          e.preventDefault();
          onSelect();
          onContextMenu(e.clientX, e.clientY);
        }}
        aria-selected={isSelected}
        className={`group relative row-enter aspect-square w-full overflow-hidden rounded-[16px] border text-left transition-[transform,box-shadow,border-color] duration-200 [transition-timing-function:var(--ease-apple)] hover:-translate-y-px hover:shadow-md ${
          isSelected
            ? "border-fg bg-elevated shadow-sm"
            : "border-line bg-elevated hover:border-line-strong"
        }`}
      >
        {/* Media: thumbnail full-bleed, or the type glyph on a quiet tile. */}
        {hasThumb ? (
          <span className="absolute inset-0 block bg-sunken">
            <img
              src={thumbSrc!}
              alt=""
              draggable={false}
              className="fade-in h-full w-full object-cover transition-transform duration-300 [transition-timing-function:var(--ease-apple)] group-hover:scale-[1.03]"
            />
          </span>
        ) : (
          <span className="absolute inset-x-3 top-3 flex">
            <FileTypeIcon result={result} size={40} />
          </span>
        )}

        {/* Name and meta. Over a thumbnail they sit on a scrim. */}
        <span
          className={`absolute inset-x-0 bottom-0 flex flex-col gap-0.5 p-3 ${
            hasThumb ? "bg-gradient-to-t from-black/70 via-black/30 to-transparent pt-10" : ""
          }`}
        >
          <span
            className={`line-clamp-2 text-[13px] font-medium leading-[17px] ${
              hasThumb ? "text-white" : "text-fg"
            }`}
          >
            <HighlightedName text={result.filename} query={query} />
          </span>
          <span
            className={`truncate font-mono text-[11px] leading-[15px] ${
              hasThumb ? "text-white/75" : "text-muted"
            }`}
          >
            {meta}
          </span>
        </span>

        {/* Hover details: facts, actions, and a primary Open. */}
        <HoverDetails
          result={result}
          onOpen={onOpen}
          onReveal={onReveal ?? onOpen}
          onCopy={onCopyPath ?? (() => undefined)}
          onMore={(x, y) => onContextMenu?.(x, y)}
        />
      </button>
    );
  },
  (prev, next) =>
    prev.result.path === next.result.path &&
    prev.result.snippet === next.result.snippet &&
    prev.result.match_type === next.result.match_type &&
    prev.result.group_count === next.result.group_count &&
    prev.isSelected === next.isSelected &&
    prev.query === next.query &&
    prev.index === next.index &&
    prev.showThumbs === next.showThumbs,
);
