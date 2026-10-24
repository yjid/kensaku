import React, { useState, useEffect } from "react";
import { SearchResult } from "../types";
import {
  ExternalLink,
  FolderOpen,
  Copy,
  FileText,
  Sparkles,
  Search,
  X,
} from "lucide-react";

interface ActionPaletteProps {
  result: SearchResult;
  onClose: () => void;
  onOpen: () => void;
  onOpenFolder: () => void;
  onCopyPath: () => void;
  onCopyContent: () => void;
  onFindSimilar: (query: string) => void;
  onAsk: () => void;
}

export const ActionPalette: React.FC<ActionPaletteProps> = ({
  result,
  onClose,
  onOpen,
  onOpenFolder,
  onCopyPath,
  onCopyContent,
  onFindSimilar,
  onAsk,
}) => {
  const [selectedIndex, setSelectedIndex] = useState(0);

  const actions = [
    {
      id: "open",
      label: "Open file",
      shortcut: "↵",
      icon: <ExternalLink className="w-4 h-4 text-blue-400" />,
      run: onOpen,
    },
    {
      id: "reveal",
      label: "Reveal in File Manager",
      shortcut: "⇧↵",
      icon: <FolderOpen className="w-4 h-4 text-amber-400" />,
      run: onOpenFolder,
    },
    {
      id: "copy-path",
      label: "Copy absolute path",
      shortcut: "⌘C",
      icon: <Copy className="w-4 h-4 text-emerald-400" />,
      run: onCopyPath,
    },
    ...(!result.is_dir
      ? [
          {
            id: "copy-content",
            label: "Copy file text content",
            shortcut: "⌥C",
            icon: <FileText className="w-4 h-4 text-purple-400" />,
            run: onCopyContent,
          },
        ]
      : []),
    {
      id: "find-similar",
      label: "Find similar files",
      shortcut: "⌥S",
      icon: <Search className="w-4 h-4 text-sky-400" />,
      run: () => {
        const ext = result.filename.split(".").pop();
        if (ext && ext !== result.filename) {
          onFindSimilar(`ext:${ext}`);
        } else {
          onFindSimilar(result.filename.slice(0, 4));
        }
      },
    },
    {
      id: "ask",
      label: "Ask about this file",
      shortcut: "⌥A",
      icon: <Sparkles className="w-4 h-4 text-pink-400" />,
      run: onAsk,
    },
  ];

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % actions.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + actions.length) % actions.length);
      } else if (e.key === "Enter") {
        e.preventDefault();
        actions[selectedIndex]?.run();
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedIndex, actions, onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md bg-[#121620] border border-white/10 rounded-2xl shadow-2xl overflow-hidden text-slate-100"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 bg-white/[0.02]">
          <div className="flex items-center gap-2 min-w-0">
            <span className="text-xs font-semibold text-slate-200">Actions</span>
            <span className="text-[11px] text-slate-500 truncate max-w-[200px]">
              {result.filename}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-2 space-y-1">
          {actions.map((act, idx) => (
            <div
              key={act.id}
              onClick={() => {
                act.run();
                onClose();
              }}
              onMouseEnter={() => setSelectedIndex(idx)}
              className={`flex items-center justify-between p-2.5 rounded-xl text-xs cursor-pointer select-none transition-all ${
                idx === selectedIndex
                  ? "bg-blue-600/20 text-white border border-blue-500/40"
                  : "text-slate-300 hover:bg-white/5 border border-transparent"
              }`}
            >
              <div className="flex items-center gap-2.5">
                {act.icon}
                <span className="font-medium">{act.label}</span>
              </div>
              <kbd className="px-1.5 py-0.5 rounded bg-white/10 font-mono text-[10px] text-slate-400">
                {act.shortcut}
              </kbd>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
