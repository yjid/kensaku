import React from "react";
import { FileTypeIcon } from "./FileTypeIcon";
import { HighlightedName } from "./EmptyState";
import { isThumbImage, useThumbnail } from "../lib/thumbnails";
import { formatBytes, formatRelativeTime } from "../lib/format";

/* One file per row, fixed height. Used for list results and recents, so the
   two lists look identical. Columns: icon, name and path, size, age. */

export interface FileRowItem {
  path: string;
  filename: string;
  file_size: number;
  modified: number;
  is_dir: boolean;
}

export const FILE_ROW_HEIGHT = 56;

export const FileRow = React.memo(function FileRow({
  item,
  query = "",
  selected = false,
  showThumbs = false,
  onSelect,
  onOpen,
  onContextMenu,
}: {
  item: FileRowItem;
  query?: string;
  selected?: boolean;
  showThumbs?: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onContextMenu?: (x: number, y: number) => void;
}) {
  const eligible = showThumbs && !item.is_dir && isThumbImage(item.filename);
  const { src, failed } = useThumbnail(item.path, eligible);
  const thumb = eligible && src && !failed ? src : null;

  return (
    <div
      role="row"
      aria-selected={selected}
      title={item.path}
      onClick={onSelect}
      onDoubleClick={onOpen}
      onContextMenu={(e) => {
        if (!onContextMenu) return;
        e.preventDefault();
        onSelect();
        onContextMenu(e.clientX, e.clientY);
      }}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "copy";
        e.dataTransfer.setData("text/plain", item.path);
        e.dataTransfer.setData("text/uri-list", `file://${item.path}`);
      }}
      style={{ height: FILE_ROW_HEIGHT }}
      className={`flex w-full cursor-default items-center gap-3 rounded-[10px] px-3 transition-colors duration-150 [transition-timing-function:var(--ease-apple)] ${
        selected ? "bg-sunken" : "hover:bg-sunken/70"
      }`}
    >
      {thumb ? (
        <img src={thumb} alt="" draggable={false} className="h-9 w-9 shrink-0 rounded-[8px] border border-line object-cover" />
      ) : (
        <FileTypeIcon result={item} size={36} />
      )}
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-medium text-fg">
          <HighlightedName text={item.filename} query={query} />
        </p>
        <p className="truncate font-mono text-[11px] text-muted">{item.path}</p>
      </div>
      <span className="w-20 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted">
        {item.is_dir ? "Folder" : formatBytes(item.file_size)}
      </span>
      <span className="w-20 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted">
        {formatRelativeTime(item.modified)}
      </span>
    </div>
  );
});
