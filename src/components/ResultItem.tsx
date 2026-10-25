import React from "react";
import { SearchResult } from "../types";
import {
  FileText,
  FileCode,
  Folder,
  FileArchive,
  Image,
  File,
  ExternalLink,
  FolderOpen,
  Copy,
  Check,
  Table,
  Presentation,
} from "lucide-react";

interface ResultItemProps {
  result: SearchResult;
  isSelected: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onOpenFolder: () => void;
}

export const ResultItem: React.FC<ResultItemProps> = ({
  result,
  isSelected,
  onSelect,
  onOpen,
  onOpenFolder,
}) => {
  const [copied, setCopied] = React.useState(false);

  const getIcon = () => {
    if (result.is_dir) {
      return <Folder className="w-5 h-5 text-blue-400" />;
    }
    const ext = result.filename.split(".").pop()?.toLowerCase() || "";
    if (
      [
        "rs",
        "ts",
        "tsx",
        "js",
        "jsx",
        "py",
        "go",
        "c",
        "cpp",
        "h",
        "java",
        "html",
        "css",
        "json",
        "toml",
        "yaml",
        "yml",
        "sql",
        "sh",
      ].includes(ext)
    ) {
      return <FileCode className="w-5 h-5 text-emerald-400" />;
    }
    if (["png", "jpg", "jpeg", "gif", "webp", "svg"].includes(ext)) {
      return <Image className="w-5 h-5 text-purple-400" />;
    }
    if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
      return <FileArchive className="w-5 h-5 text-amber-400" />;
    }
    if (["xls", "xlsx", "csv", "tsv", "ods", "numbers"].includes(ext)) {
      return <Table className="w-5 h-5 text-emerald-400" />;
    }
    if (["ppt", "pptx", "odp", "key"].includes(ext)) {
      return <Presentation className="w-5 h-5 text-orange-400" />;
    }
    if (ext === "pdf") {
      return <FileText className="w-5 h-5 text-rose-400" />;
    }
    if (["md", "txt", "doc", "docx", "log", "rtf", "odt", "pages", "epub"].includes(ext)) {
      return <FileText className="w-5 h-5 text-sky-400" />;
    }
    return <File className="w-5 h-5 text-slate-400" />;
  };

  const formatSize = (bytes: number): string => {
    if (result.is_dir) return "Folder";
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  const formatRelativeTime = (seconds: number): string => {
    if (!seconds) return "";
    const now = Math.floor(Date.now() / 1000);
    const diff = Math.max(0, now - seconds);

    if (diff < 60) return "Just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
    return new Date(seconds * 1000).toLocaleDateString();
  };

  const handleCopyPath = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(result.path);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div
      onClick={onSelect}
      onDoubleClick={onOpen}
      className={`group relative flex flex-col p-2.5 rounded-xl transition-all cursor-pointer select-none border ${
        isSelected
          ? "bg-blue-600/20 border-blue-500/50 shadow-md shadow-blue-500/5"
          : "bg-white/[0.02] hover:bg-white/[0.06] border-white/5"
      }`}
    >
      <div className="flex items-center justify-between gap-3">
        {/* Left: Icon + Name + Path */}
        <div className="flex items-center gap-3 min-w-0 flex-1">
          <div className="flex-shrink-0">{getIcon()}</div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="font-medium text-xs text-slate-100 truncate">
                {result.filename}
              </span>

              {/* Match badge */}
              {result.match_type === "Content" && (
                <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded bg-purple-500/20 text-purple-300 border border-purple-500/30">
                  text inside
                </span>
              )}
              {result.match_type === "Both" && (
                <span className="flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
                  name & text
                </span>
              )}
              {result.match_type === "Semantic" && (
                <span
                  title="Semantic similarity match"
                  className="flex-shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded bg-cyan-500/15 text-cyan-300 border border-cyan-500/25 flex items-center gap-0.5"
                >
                  <span className="text-xs leading-none">≈</span>
                  <span>semantic</span>
                </span>
              )}
            </div>

            <div className="text-[11px] text-slate-400 truncate opacity-80 mt-0.5">
              {result.path}
            </div>
          </div>
        </div>

        {/* Right: Meta & Actions */}
        <div className="flex items-center gap-2 flex-shrink-0 text-[11px] text-slate-400">
          <span className="hidden sm:inline-block font-mono text-slate-400">
            {formatSize(result.file_size)}
          </span>
          <span className="hidden sm:inline-block text-slate-500">•</span>
          <span className="hidden sm:inline-block text-slate-400">
            {formatRelativeTime(result.modified)}
          </span>

          {/* Quick hover action buttons */}
          <div className="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity ml-1">
            <button
              type="button"
              title="Copy path"
              onClick={handleCopyPath}
              className="p-1 rounded-md hover:bg-white/10 text-slate-400 hover:text-white"
            >
              {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
            </button>
            <button
              type="button"
              title="Open containing folder"
              onClick={(e) => {
                e.stopPropagation();
                onOpenFolder();
              }}
              className="p-1 rounded-md hover:bg-white/10 text-slate-400 hover:text-white"
            >
              <FolderOpen className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              title="Open file"
              onClick={(e) => {
                e.stopPropagation();
                onOpen();
              }}
              className="p-1 rounded-md hover:bg-white/10 text-slate-400 hover:text-white"
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </div>

      {/* Snippet preview if matched inside content */}
      {result.snippet && (
        <div className="mt-1.5 pt-1.5 border-t border-white/5 pl-8 pr-2">
          <p className="text-[11px] font-mono text-slate-300/90 bg-black/30 rounded-md px-2 py-1 line-clamp-2">
            {result.snippet}
          </p>
        </div>
      )}
    </div>
  );
};
