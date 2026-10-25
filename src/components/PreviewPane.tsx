import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { SearchResult, FilePreview } from "../types";
import {
  FileText,
  Folder,
  File,
  ExternalLink,
  FolderOpen,
  Copy,
  Check,
  MoreHorizontal,
  Loader2,
} from "lucide-react";

interface PreviewPaneProps {
  result: SearchResult | null;
  searchQuery: string;
  onOpen: () => void;
  onOpenFolder: () => void;
  onOpenActions: () => void;
}

export const PreviewPane: React.FC<PreviewPaneProps> = ({
  result,
  searchQuery,
  onOpen,
  onOpenFolder,
  onOpenActions,
}) => {
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [copiedPath, setCopiedPath] = useState(false);
  const [copiedContent, setCopiedContent] = useState(false);

  useEffect(() => {
    if (!result) {
      setPreview(null);
      return;
    }

    let isCancelled = false;
    setLoading(true);

    invoke<FilePreview>("get_file_preview", { path: result.path })
      .then((prev) => {
        if (!isCancelled) {
          setPreview(prev);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!isCancelled) {
          console.error("Preview error:", err);
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
      isCancelled = true;
    };
  }, [result?.path]);

  if (!result) {
    return (
      <div className="hidden lg:flex flex-col items-center justify-center h-full text-center p-8 text-slate-500 border-l border-white/10 bg-black/20">
        <FileText className="w-10 h-10 text-slate-600 mb-2 stroke-[1.5]" />
        <p className="text-xs text-slate-400">Select an item to preview</p>
        <p className="text-[11px] text-slate-600 mt-1">
          Use ↑ and ↓ arrows to navigate
        </p>
      </div>
    );
  }

  const formatSize = (bytes: number): string => {
    if (result.is_dir) return "Directory";
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const formatModified = (seconds: number): string => {
    if (!seconds) return "";
    return new Date(seconds * 1000).toLocaleString();
  };

  const handleCopyPath = () => {
    navigator.clipboard.writeText(result.path);
    setCopiedPath(true);
    setTimeout(() => setCopiedPath(false), 1500);
  };

  const handleCopyContent = async () => {
    try {
      const content = await invoke<string>("read_file_content", {
        path: result.path,
      });
      navigator.clipboard.writeText(content);
      setCopiedContent(true);
      setTimeout(() => setCopiedContent(false), 1500);
    } catch (e) {
      console.error(e);
    }
  };

  const renderContentWithHighlights = (text: string) => {
    const terms = searchQuery
      .split(/\s+/)
      .map((t) => t.trim().toLowerCase())
      .filter((t) => t.length > 1 && !t.includes(":"));

    const [rangeStart, rangeEnd] = result?.best_range ?? [null, null];
    let currentByteOffset = 0;

    const lines = text.split("\n");
    return lines.map((line, lineIdx) => {
      const lineByteLength = new TextEncoder().encode(line).length + 1;
      const lineStart = currentByteOffset;
      const lineEnd = currentByteOffset + lineByteLength;
      currentByteOffset = lineEnd;

      let highlighted = false;
      let isSemanticRange = false;

      if (rangeStart !== null && rangeEnd !== null && rangeEnd > rangeStart) {
        if (lineEnd > rangeStart && lineStart < rangeEnd) {
          isSemanticRange = true;
          highlighted = true;
        }
      }

      if (!highlighted) {
        const lowerLine = line.toLowerCase();
        for (const t of terms) {
          if (lowerLine.includes(t)) {
            highlighted = true;
            break;
          }
        }
      }

      return (
        <div
          key={lineIdx}
          className={`flex text-[11px] font-mono leading-relaxed px-2 py-0.5 rounded transition-colors ${
            isSemanticRange
              ? "bg-cyan-500/20 text-cyan-200 border-l-2 border-cyan-400 pl-1.5"
              : highlighted
              ? "bg-blue-500/15 text-blue-200"
              : "text-slate-300"
          }`}
        >
          <span className="w-8 select-none text-slate-600 flex-shrink-0 text-right pr-3 font-mono">
            {lineIdx + 1}
          </span>
          <span className="whitespace-pre-wrap break-all flex-1">{line}</span>
        </div>
      );
    });
  };

  return (
    <div className="hidden lg:flex flex-col h-full border-l border-white/10 bg-[#090c12]/80 backdrop-blur-md overflow-hidden w-96 flex-shrink-0">
      {/* Header */}
      <div className="p-3 border-b border-white/10 bg-white/[0.02]">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <h3 className="font-semibold text-xs text-white truncate" title={result.filename}>
              {result.filename}
            </h3>
            <p className="text-[10px] text-slate-400 font-mono truncate mt-0.5" title={result.path}>
              {result.path}
            </p>
          </div>
          <button
            type="button"
            title="More Actions (Ctrl+K / ⌘K)"
            onClick={onOpenActions}
            className="p-1 rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition-colors flex-shrink-0"
          >
            <MoreHorizontal className="w-4 h-4" />
          </button>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-1.5 mt-2.5">
          <button
            type="button"
            onClick={onOpen}
            className="flex-1 flex items-center justify-center gap-1.5 px-2.5 py-1 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-[11px] font-medium transition-colors"
          >
            <ExternalLink className="w-3 h-3" />
            <span>Open</span>
          </button>

          <button
            type="button"
            onClick={onOpenFolder}
            className="p-1.5 rounded-md bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white border border-white/10 transition-colors"
            title="Reveal in folder"
          >
            <FolderOpen className="w-3.5 h-3.5" />
          </button>

          <button
            type="button"
            onClick={handleCopyPath}
            className="p-1.5 rounded-md bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white border border-white/10 transition-colors"
            title="Copy path"
          >
            {copiedPath ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
          </button>

          {!result.is_dir && (
            <button
              type="button"
              onClick={handleCopyContent}
              className="px-2 py-1 rounded-md bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white border border-white/10 text-[10px] transition-colors"
              title="Copy full text content"
            >
              {copiedContent ? "Copied!" : "Copy Text"}
            </button>
          )}
        </div>

        {/* Meta badges */}
        <div className="flex flex-wrap items-center gap-2 mt-2 pt-2 border-t border-white/5 text-[10px] text-slate-400">
          <span>{formatSize(result.file_size)}</span>
          <span>•</span>
          <span>{formatModified(result.modified)}</span>
          {preview?.preview_type === "pdf" && preview.child_count && (
            <>
              <span>•</span>
              <span className="text-rose-300 font-medium">{preview.child_count} pages</span>
            </>
          )}
          {preview?.preview_type === "presentation" && (
            <>
              <span>•</span>
              <span className="text-orange-300 font-medium">Slide deck</span>
            </>
          )}
          {preview?.preview_type === "sheet" && (
            <>
              <span>•</span>
              <span className="text-emerald-300 font-medium">Spreadsheet</span>
            </>
          )}
          {preview?.preview_type === "doc" && (
            <>
              <span>•</span>
              <span className="text-sky-300 font-medium">Document</span>
            </>
          )}
          {result.snippet && (
            <>
              <span>•</span>
              <span className="text-purple-300">Snippet match</span>
            </>
          )}
        </div>
      </div>

      {/* Body Preview */}
      <div className="flex-1 overflow-y-auto p-2">
        {loading ? (
          <div className="flex items-center justify-center h-48 text-slate-500">
            <Loader2 className="w-5 h-5 animate-spin text-blue-400" />
          </div>
        ) : (preview?.preview_type === "text" || preview?.preview_type === "doc" || preview?.preview_type === "presentation" || preview?.preview_type === "sheet" || preview?.preview_type === "pdf") && preview.content ? (
          <div>
            {preview.preview_type === "pdf" && (
              <div className="mb-2 px-2 py-1 rounded bg-rose-500/10 border border-rose-500/20 text-rose-300 text-[10px] flex items-center justify-between">
                <span>PDF Text Layer • {preview.child_count ?? 1} pages</span>
                <span className="text-slate-400 text-[9px]">Press Enter to open PDF</span>
              </div>
            )}
            {preview.preview_type === "presentation" && (
              <div className="mb-2 px-2 py-1 rounded bg-orange-500/10 border border-orange-500/20 text-orange-300 text-[10px]">
                Presentation Slides
              </div>
            )}
            {preview.preview_type === "sheet" && (
              <div className="mb-2 px-2 py-1 rounded bg-emerald-500/10 border border-emerald-500/20 text-emerald-300 text-[10px]">
                Spreadsheet Content
              </div>
            )}
            {preview.preview_type === "doc" && (
              <div className="mb-2 px-2 py-1 rounded bg-sky-500/10 border border-sky-500/20 text-sky-300 text-[10px]">
                Extracted Document Content
              </div>
            )}
            <div className="py-1">{renderContentWithHighlights(preview.content)}</div>
          </div>
        ) : preview?.preview_type === "image" ? (
          <div className="flex flex-col items-center justify-center p-4">
            <img
              src={convertFileSrc(result.path)}
              alt={result.filename}
              className="max-h-64 object-contain rounded-lg border border-white/10 shadow-lg bg-black/40"
              onError={(e) => {
                (e.target as HTMLElement).style.display = "none";
              }}
            />
          </div>
        ) : preview?.preview_type === "pdf" ? (
          <div className="flex flex-col items-center justify-center h-48 text-center p-4">
            <FileText className="w-10 h-10 text-rose-400 mb-2" />
            <p className="text-xs text-white font-medium">PDF Document</p>
            <p className="text-[11px] text-slate-500 mt-1">
              Press Enter to open in system viewer
            </p>
          </div>
        ) : preview?.preview_type === "dir" ? (
          <div className="flex flex-col items-center justify-center h-48 text-center p-4">
            <Folder className="w-10 h-10 text-amber-400 mb-2" />
            <p className="text-xs text-white font-medium">Directory</p>
            <p className="text-[11px] text-slate-400 mt-1">
              {preview.child_count ?? 0} items inside
            </p>
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-48 text-center p-4 text-slate-500">
            <File className="w-10 h-10 text-slate-600 mb-2" />
            <p className="text-xs text-slate-400">Binary or non-text file</p>
            <p className="text-[11px] text-slate-600 mt-1">
              Preview not supported for this file type
            </p>
          </div>
        )}
      </div>
    </div>
  );
};
