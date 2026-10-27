import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileTypeIcon } from "./FileTypeIcon";
import { isThumbImage } from "../lib/thumbnails";
import type { SearchResult } from "../types";

/* Quick look (Space). Shows the file's preview at a readable size: the
   first page of a PDF, a video frame, or the image itself. Plain text and
   code show their first lines. Anything else shows its type icon. Escape or
   a click outside closes it. */

const TEXT_EXTS = new Set([
  "txt", "md", "log", "csv", "json", "toml", "yaml", "yml", "xml", "html", "css",
  "js", "jsx", "ts", "tsx", "rs", "py", "go", "sh", "sql", "conf", "ini", "env",
]);

function ext(name: string): string {
  return name.includes(".") ? (name.split(".").pop() ?? "").toLowerCase() : "";
}

export function PreviewDialog({
  result,
  onOpen,
  onClose,
}: {
  result: SearchResult;
  onOpen: () => void;
  onClose: () => void;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const previewable = !result.is_dir && (isThumbImage(result.filename));
  const textual = !result.is_dir && TEXT_EXTS.has(ext(result.filename));

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setSrc(null);
    setText(null);
    const work = async () => {
      if (previewable) {
        try {
          const data = await invoke<string | null>("get_thumbnail_data", { path: result.path });
          if (!cancelled) setSrc(data);
        } catch {
          /* falls back to the icon */
        }
      } else if (textual) {
        try {
          const body = await invoke<string>("read_file_content", { path: result.path });
          if (!cancelled) setText(body.split("\n").slice(0, 60).join("\n"));
        } catch {
          /* no text available */
        }
      }
      if (!cancelled) setLoading(false);
    };
    void work();
    return () => {
      cancelled = true;
    };
  }, [result.path, previewable, textual]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === " ") {
        e.preventDefault();
        onClose();
      }
      if (e.key === "Enter") {
        e.preventDefault();
        onOpen();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, onOpen]);

  return (
    <div
      className="overlay-in fixed inset-0 z-[80] flex items-center justify-center bg-scrim p-8"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div role="dialog" aria-label="Preview" className="elevated flex max-h-full w-full max-w-3xl flex-col overflow-hidden">
        <div className="flex items-center justify-between gap-4 border-b border-line px-5 py-3">
          <div className="min-w-0">
            <p className="truncate text-[13px] font-semibold text-fg">{result.filename}</p>
            <p className="truncate font-mono text-[11px] text-muted">{result.path}</p>
          </div>
          <button type="button" onClick={onOpen} className="btn btn-primary h-8 shrink-0 px-4">
            Open
          </button>
        </div>
        <div className="flex min-h-[320px] flex-1 items-center justify-center overflow-auto bg-sunken p-6">
          {src ? (
            <img src={src} alt="" className="fade-in max-h-[70vh] max-w-full rounded-[10px] object-contain shadow-sm" />
          ) : text ? (
            <pre className="w-full self-stretch overflow-auto whitespace-pre-wrap font-mono text-[12px] leading-[18px] text-fg">
              {text}
            </pre>
          ) : (
            <div className="flex flex-col items-center gap-3 text-muted">
              <FileTypeIcon result={result} size={72} bare />
              <p className="text-[12px]">{loading ? "Loading preview…" : "No preview for this file type"}</p>
            </div>
          )}
        </div>
        <div className="flex items-center justify-between px-5 py-2.5 text-[11px] text-muted">
          <span>Space or Esc to close · Enter to open</span>
        </div>
      </div>
    </div>
  );
}
