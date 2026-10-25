import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SearchResult, FilePreview } from "../types";
import {
  ExternalLink,
  FolderOpen,
  Copy,
  Check,
  Ellipsis,
  LoaderCircle,
} from "lucide-react";
import { FileTypeIcon } from "./FileTypeIcon";
import { EmptyState } from "./EmptyState";
import { useThumbnail } from "../lib/thumbnails";
import { formatBytes, queryTerms } from "../lib/format";

/* 360px preview. Text/PDF/doc bodies render as mono lines with the
   semantic best_range marked. Thumbspace is reserved — no layout shift. */

interface PreviewPaneProps {
  result: SearchResult | null;
  searchQuery: string;
  onOpen: () => void;
  onOpenFolder: () => void;
  onOpenActions: () => void;
}

const PREVIEW_W = "w-[360px] shrink-0 border-l border-line bg-bg";

export const PreviewPane = React.memo<PreviewPaneProps>(
  ({ result, searchQuery, onOpen, onOpenFolder, onOpenActions }) => {
    const [preview, setPreview] = useState<FilePreview | null>(null);
    const [loading, setLoading] = useState(false);
    const [copiedPath, setCopiedPath] = useState(false);

    useEffect(() => {
      if (!result) {
        setPreview(null);
        return;
      }
      let cancelled = false;
      setLoading(true);
      invoke<FilePreview>("get_file_preview", { path: result.path })
        .then((p) => {
          if (!cancelled) {
            setPreview(p);
            setLoading(false);
          }
        })
        .catch((err) => {
          console.error("Preview error:", err);
          if (!cancelled) {
            setPreview({
              path: result.path,
              filename: result.filename,
              file_size: result.file_size,
              modified: result.modified,
              is_dir: result.is_dir,
              preview_type: result.is_dir ? "dir" : "binary",
            });
            setLoading(false);
          }
        });
      return () => {
        cancelled = true;
      };
    }, [result?.path]);

    if (!result) {
      return (
        <aside className={`hidden min-[900px]:flex flex-col ${PREVIEW_W}`}>
          <EmptyState variant="empty" caption="Select a file to preview">
            <p className="text-[11px] text-subtle">
              <kbd className="kbd">↑</kbd> <kbd className="kbd">↓</kbd> to navigate ·
              <kbd className="kbd ml-1.5">tab</kbd> to hide this pane
            </p>
          </EmptyState>
        </aside>
      );
    }

    const handleCopyPath = () => {
      navigator.clipboard.writeText(result.path);
      setCopiedPath(true);
      window.setTimeout(() => setCopiedPath(false), 1200);
    };

    const contentRender = renderContent(preview, searchQuery, result);

    return (
      <aside className={`hidden min-[900px]:flex h-full flex-col overflow-hidden ${PREVIEW_W}`}>
        {/* Header */}
        <header className="shrink-0 border-b border-line p-3">
          <div className="flex items-start gap-2.5">
            <FileTypeIcon result={result} size={28} />
            <div className="min-w-0 flex-1">
              <h3 className="truncate text-[13px] font-medium leading-[18px] text-fg" title={result.filename}>
                {result.filename}
              </h3>
              <p className="truncate font-mono text-[10px] leading-[14px] text-subtle" title={result.path}>
                {result.path}
              </p>
            </div>
            <button
              type="button"
              title="Actions (⌘K)"
              onClick={onOpenActions}
              className="-mr-1 flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
            >
              <Ellipsis className="h-4 w-4" />
            </button>
          </div>

          <div className="mt-2.5 flex items-center gap-1.5">
            <button
              type="button"
              onClick={onOpen}
              className="flex h-7 flex-1 items-center justify-center gap-1.5 rounded-[10px] bg-accent px-2 text-[11px] font-medium text-accent-fg transition-colors duration-150 hover:opacity-90"
            >
              <ExternalLink className="h-3 w-3" />
              Open
            </button>
            <button
              type="button"
              title="Reveal in file manager"
              onClick={onOpenFolder}
              className="flex h-7 w-7 items-center justify-center rounded-[10px] border border-line text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              title="Copy path"
              onClick={handleCopyPath}
              className="flex h-7 w-7 items-center justify-center rounded-[10px] border border-line text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
            >
              {copiedPath ? <Check className="h-3.5 w-3.5 text-accent" /> : <Copy className="h-3.5 w-3.5" />}
            </button>
          </div>

          {/* For image previews the meta line moves under the large
              thumbnail in the body, so it isn't duplicated here. */}
          {preview?.preview_type !== "image" && (
            <div className="mt-2 flex items-center gap-2 border-t border-line pt-2 font-mono text-[10px] leading-[14px] text-subtle">
              <span>{formatBytes(result.file_size)}</span>
              <span>·</span>
              <span>{new Date(result.modified * 1000).toLocaleString()}</span>
              {preview?.preview_type === "pdf" && preview.child_count != null && (
                <>
                  <span>·</span>
                  <span className="text-fg">{preview.child_count} pages</span>
                </>
              )}
            </div>
          )}
        </header>

        {/* Body */}
        <div className="min-h-0 flex-1 overflow-y-auto bg-sunken/50 p-2">
          {loading ? (
            <div className="flex h-40 items-center justify-center">
              <LoaderCircle className="h-4 w-4 animate-spin text-subtle" />
            </div>
          ) : (
            contentRender
          )}
        </div>
      </aside>
    );
  },
  (prev, next) => prev.result?.path === next.result?.path && prev.searchQuery === next.searchQuery,
);

function renderContent(
  preview: FilePreview | null,
  searchQuery: string,
  result: SearchResult,
): React.ReactNode {
  if (!preview) {
    return (
      <EmptyState variant="empty" caption="Nothing to preview" />
    );
  }

  const hasText =
    (preview.preview_type === "text" ||
      preview.preview_type === "doc" ||
      preview.preview_type === "presentation" ||
      preview.preview_type === "sheet" ||
      preview.preview_type === "pdf") &&
    preview.content;

  if (hasText) {
    return <TextBody preview={preview} searchQuery={searchQuery} result={result} />;
  }

  if (preview.preview_type === "image") {
    return (
      <ImagePreviewBody
        key={result.path}
        result={result}
        width={preview.width ?? null}
        height={preview.height ?? null}
      />
    );
  }

  if (preview.preview_type === "dir") {
    return (
      <EmptyState variant="empty" caption="A folder, archived">
        <p className="font-mono text-[11px] text-subtle">
          {preview.child_count ?? 0} items inside · <kbd className="kbd">shift</kbd>+
          <kbd className="kbd">enter</kbd> to reveal
        </p>
      </EmptyState>
    );
  }

  if (preview.preview_type === "pdf") {
    return (
      <EmptyState variant="empty" caption="Two-dimensional artifact">
        <p className="font-mono text-[11px] text-subtle">
          <kbd className="kbd">enter</kbd> opens the system viewer
        </p>
      </EmptyState>
    );
  }

  return (
    <EmptyState variant="empty" caption="Binary — no words to show">
      <p className="font-mono text-[11px] text-subtle">preview not supported for this type</p>
    </EmptyState>
  );
}

/* Large thumbnail for image previews — sits above the metadata line.
   Failures (missing get_thumbnail, unreadable file) swap in the empty
   state illustration; nothing logs. */
function ImagePreviewBody({
  result,
  width,
  height,
}: {
  result: SearchResult;
  width: number | null;
  height: number | null;
}) {
  const { src, failed } = useThumbnail(result.path, true);
  const [loadErr, setLoadErr] = useState(false);

  if (failed || loadErr) {
    return (
      <EmptyState variant="empty" caption="Preview unavailable">
        <p className="font-mono text-[11px] text-subtle">image preview not available</p>
      </EmptyState>
    );
  }

  return (
    <div className="flex h-full flex-col p-3">
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <div className="overflow-hidden rounded-[14px] border border-line bg-elevated p-1.5">
          {src ? (
            <img
              src={src}
              alt={result.filename}
              draggable={false}
              className="fade-in mx-auto max-h-[360px] max-w-full rounded-[10px] object-contain"
              onError={() => setLoadErr(true)} />
          ) : (
            <div className="flex h-40 w-56 items-center justify-center">
              <LoaderCircle className="h-4 w-4 animate-spin text-subtle" />
            </div>
          )}
        </div>
      </div>
      <div className="mt-2 flex shrink-0 items-center justify-center gap-2 border-t border-line pt-2 font-mono text-[10px] leading-[14px] text-subtle">
        <span>{formatBytes(result.file_size)}</span>
        <span>·</span>
        <span>{new Date(result.modified * 1000).toLocaleString()}</span>
        {width != null && height != null && (
          <>
            <span>·</span>
            <span>
              {width} × {height}
            </span>
          </>
        )}
      </div>
    </div>
  );
}

/* Mono text body with per-line match marking. */
const TextBody: React.FC<{ preview: FilePreview; searchQuery: string; result: SearchResult }> = ({
  preview,
  searchQuery,
  result,
}) => {
  const terms = queryTerms(searchQuery);
  const [rangeStart, rangeEnd] = result.best_range ?? [null, null];

  let byteOffset = 0;
  const lines = (preview.content ?? "").split("\n");

  return (
    <div className="rounded-[10px] border border-line bg-elevated py-1">
      {lines.slice(0, 400).map((line, i) => {
        const lineStart = byteOffset;
        byteOffset += new TextEncoder().encode(line).length + 1;
        const inSemanticRange =
          rangeStart != null &&
          rangeEnd != null &&
          rangeEnd > rangeStart &&
          lineStart < rangeEnd &&
          lineStart + new TextEncoder().encode(line).length >= rangeStart;
        const termHit =
          !inSemanticRange && terms.some((t) => line.toLowerCase().includes(t));
        return (
          <div
            key={i}
            className={`flex px-2 py-px text-[11px] leading-[17px] ${
              inSemanticRange
                ? "border-l-2 border-accent bg-accent-soft text-fg"
                : termHit
                  ? "bg-accent-soft/60 text-fg"
                  : "text-muted"
            }`}
          >
            <span className="w-7 shrink-0 select-none pr-2 text-right font-mono text-[10px] leading-[17px] text-subtle">
              {i + 1}
            </span>
            <span className="min-w-0 flex-1 whitespace-pre-wrap break-all font-mono">
              {line || " "}
            </span>
          </div>
        );
      })}
      {lines.length > 400 && (
        <p className="px-3 py-2 font-mono text-[10px] text-subtle">
          … {lines.length - 400} more lines — <kbd className="kbd">enter</kbd> opens full file
        </p>
      )}
    </div>
  );
};
