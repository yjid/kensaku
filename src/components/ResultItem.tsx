import React from "react";
import { SearchResult } from "../types";
import { FileTypeIcon } from "./FileTypeIcon";
import { HighlightedName } from "./EmptyState";
import { isThumbImage, useThumbnail } from "../lib/thumbnails";
import { formatBytes, formatRelativeTime } from "../lib/format";
import { HoverDetails } from "./HoverDetails";

/* Result card. The thumbnail is the hero: a rounded 4:3 tile on a quiet
   surface. The name and a size · age line sit below it, outside the image,
   so text never fights the picture. Hover lifts the tile and reveals the
   detail strip; right-click opens the full menu. */

export type ResultView = "grid" | "list";

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
  ({ result, query, isSelected, onSelect, onOpen, onContextMenu, showThumbs }) => {
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
        title={result.path}
        className="group card-in flex h-full w-full flex-col text-left outline-none"
      >
        {/* Tile */}
        <span
          className={`relative block aspect-[4/3] w-full overflow-hidden rounded-[14px] border bg-sunken transition-[transform,box-shadow,border-color] duration-300 [transition-timing-function:var(--ease-apple)] group-hover:-translate-y-0.5 group-hover:shadow-lg group-focus-visible:ring-2 group-focus-visible:ring-fg/40 ${
            isSelected ? "border-fg shadow-md" : "border-line group-hover:border-line-strong"
          }`}
        >
          {hasThumb ? (
            <img
              src={thumbSrc!}
              alt=""
              draggable={false}
              className="fade-in h-full w-full object-cover transition-transform duration-500 [transition-timing-function:var(--ease-apple)] group-hover:scale-[1.03]"
            />
          ) : (
            <span className="absolute inset-0 flex items-center justify-center">
              <FileTypeIcon result={result} size={60} bare />
            </span>
          )}
          <HoverDetails result={result} />
        </span>

        {/* Labels sit below the tile */}
        <span className="flex min-w-0 flex-col gap-0.5 px-1 pt-2.5">
          <span className="line-clamp-1 text-[13px] font-medium leading-[18px] text-fg">
            <HighlightedName text={result.filename} query={query} />
          </span>
          <span className="truncate font-mono text-[11px] leading-[15px] text-muted">{meta}</span>
        </span>
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
