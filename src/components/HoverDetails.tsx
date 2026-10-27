import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, ExternalLink, FolderOpen, MoreHorizontal } from "lucide-react";
import { formatBytes, formatRelativeTime } from "../lib/format";
import type { SearchResult } from "../types";

/* Hover preview. Opens over the card after a short pause, from the card's
   bottom edge. Shows the facts, icon actions, and a primary Open. Image
   dimensions load here, on hover only, never during search. */

interface Details {
  width?: number | null;
  height?: number | null;
}

export function HoverDetails({
  result,
  onOpen,
  onReveal,
  onCopy,
  onMore,
}: {
  result: SearchResult;
  onOpen: () => void;
  onReveal: () => void;
  onCopy: () => void;
  onMore: (x: number, y: number) => void;
}) {
  const [details, setDetails] = useState<Details>({});

  useEffect(() => {
    if (result.is_dir) return;
    let cancelled = false;
    invoke<Details>("get_file_details", { path: result.path })
      .then((d) => {
        if (!cancelled) setDetails(d);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [result.path, result.is_dir]);

  const rows: [string, string][] = [
    ["Modified", formatRelativeTime(result.modified)],
    ["Size", result.is_dir ? "Folder" : formatBytes(result.file_size)],
  ];
  if (details.width && details.height) {
    rows.push(["Dimensions", `${details.width} × ${details.height}`]);
  }

  const stop = (fn: () => void) => (e: React.MouseEvent) => {
    e.stopPropagation();
    fn();
  };

  return (
    <div
      role="group"
      aria-label="Details"
      onClick={(e) => e.stopPropagation()}
      className="hover-details absolute inset-x-2 bottom-2 z-20 hidden cursor-default rounded-[16px] border border-line bg-elevated/95 p-3.5 text-left shadow-lg backdrop-blur-md group-hover:block"
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-fg">{result.filename}</p>
          <p className="truncate font-mono text-[11px] text-muted">{result.path}</p>
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          <IconAction label="Copy path" onClick={stop(onCopy)}>
            <Copy className="h-3.5 w-3.5" />
          </IconAction>
          <IconAction label="Show in folder" onClick={stop(onReveal)}>
            <FolderOpen className="h-3.5 w-3.5" />
          </IconAction>
          <IconAction label="More" onClick={(e) => { e.stopPropagation(); onMore(e.clientX, e.clientY); }}>
            <MoreHorizontal className="h-3.5 w-3.5" />
          </IconAction>
        </div>
      </div>

      <dl className="mt-3 divide-y divide-line">
        {rows.map(([label, value]) => (
          <div key={label} className="flex items-center justify-between py-1.5">
            <dt className="text-[11.5px] text-muted">{label}</dt>
            <dd className="font-mono text-[11.5px] text-fg">{value}</dd>
          </div>
        ))}
      </dl>

      <button
        type="button"
        onClick={stop(onOpen)}
        className="mt-2.5 flex h-8 w-full items-center justify-center gap-1.5 rounded-[10px] bg-fg text-[12px] font-medium text-bg transition-opacity duration-150 hover:opacity-85"
      >
        <ExternalLink className="h-3.5 w-3.5" />
        Open
      </button>
    </div>
  );
}

function IconAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) {
  return (
    <span
      role="button"
      tabIndex={-1}
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
    >
      {children}
    </span>
  );
}
