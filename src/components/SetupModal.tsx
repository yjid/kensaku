import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, FolderX, Keyboard, Sparkles, Check, Plus, X } from "lucide-react";

interface SetupModalProps {
  onSetupCompleted: () => void;
}

export const SetupModal: React.FC<SetupModalProps> = ({ onSetupCompleted }) => {
  const [suggestedDirs, setSuggestedDirs] = useState<string[]>([]);
  const [selectedDirs, setSelectedDirs] = useState<string[]>([]);
  const [customDirInput, setCustomDirInput] = useState("");

  const [excludedDirs, setExcludedDirs] = useState<string[]>([
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".output",
    ".cache",
    ".venv",
    "venv",
    "__pycache__",
    "vendor",
    "Pods",
    ".gradle",
    ".idea",
    ".vscode",
    ".Trash",
    "lost+found",
  ]);
  const [customExcludeInput, setCustomExcludeInput] = useState("");
  const [shortcut, setShortcut] = useState("Alt+Space");
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    invoke<string[]>("get_suggested_directories")
      .then((dirs) => {
        setSuggestedDirs(dirs);
        // Default: select the first 1-3 suggested directories (like Home or Documents/Projects)
        if (dirs.length > 0) {
          setSelectedDirs(dirs.slice(0, 3));
        }
      })
      .catch((err) => console.error("Failed to load suggested dirs:", err));
  }, []);

  const toggleDir = (dir: string) => {
    if (selectedDirs.includes(dir)) {
      setSelectedDirs(selectedDirs.filter((d) => d !== dir));
    } else {
      setSelectedDirs([...selectedDirs, dir]);
    }
  };

  const addCustomDir = () => {
    const trimmed = customDirInput.trim();
    if (trimmed && !selectedDirs.includes(trimmed)) {
      setSelectedDirs([...selectedDirs, trimmed]);
      setCustomDirInput("");
    }
  };

  const addExcludeDir = () => {
    const trimmed = customExcludeInput.trim();
    if (trimmed && !excludedDirs.includes(trimmed)) {
      setExcludedDirs([...excludedDirs, trimmed]);
      setCustomExcludeInput("");
    }
  };

  const removeExcludeDir = (name: string) => {
    setExcludedDirs(excludedDirs.filter((d) => d !== name));
  };

  const handleFinishSetup = async () => {
    if (selectedDirs.length === 0) {
      alert("Please select at least one folder to index.");
      return;
    }

    setIsSubmitting(true);
    try {
      await invoke("complete_setup", {
        watchedDirs: selectedDirs,
        excludedDirs: excludedDirs,
        shortcut: shortcut,
      });
      onSetupCompleted();
    } catch (err) {
      console.error("Setup failed:", err);
      alert("Setup failed: " + err);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="flex flex-col h-full bg-[#12141a]/95 text-slate-100 rounded-2xl border border-white/10 shadow-2xl overflow-hidden backdrop-blur-xl">
      {/* Header */}
      <div className="p-6 border-b border-white/10 bg-gradient-to-r from-blue-900/30 via-indigo-900/20 to-purple-900/30 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-blue-600/30 border border-blue-400/30 flex items-center justify-center text-blue-400 shadow-lg shadow-blue-500/10">
            <Sparkles className="w-5 h-5" />
          </div>
          <div>
            <h1 className="text-xl font-bold tracking-tight text-white flex items-center gap-2">
              Kensaku <span className="text-xs px-2 py-0.5 rounded-full bg-blue-500/20 text-blue-400 border border-blue-500/30 font-medium">検索</span>
            </h1>
            <p className="text-xs text-slate-400">Blazing-fast local file and text search</p>
          </div>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* Step 1: Folders to Index */}
        <div className="space-y-3">
          <label className="text-sm font-semibold text-slate-200 flex items-center gap-2">
            <Folder className="w-4 h-4 text-blue-400" />
            Folders to Index
          </label>
          <p className="text-xs text-slate-400">
            Select the folders you want Kensaku to index. Files and text content will be instantly searchable.
          </p>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {suggestedDirs.map((dir) => {
              const isSelected = selectedDirs.includes(dir);
              const name = dir.split("/").filter(Boolean).pop() || dir;
              return (
                <button
                  type="button"
                  key={dir}
                  onClick={() => toggleDir(dir)}
                  className={`flex items-center justify-between p-3 rounded-xl border text-left text-xs transition-all ${
                    isSelected
                      ? "bg-blue-600/20 border-blue-500/50 text-white shadow-sm"
                      : "bg-white/5 border-white/5 text-slate-400 hover:bg-white/10"
                  }`}
                >
                  <div className="truncate mr-2">
                    <div className="font-medium text-slate-200 truncate">{name}</div>
                    <div className="text-[10px] text-slate-500 truncate">{dir}</div>
                  </div>
                  <div
                    className={`w-5 h-5 rounded-md flex items-center justify-center border transition-all ${
                      isSelected
                        ? "bg-blue-500 border-blue-400 text-white"
                        : "border-white/20 bg-black/20"
                    }`}
                  >
                    {isSelected && <Check className="w-3.5 h-3.5" />}
                  </div>
                </button>
              );
            })}
          </div>

          {/* Custom path input */}
          <div className="flex gap-2 pt-1">
            <input
              type="text"
              placeholder="Or enter custom directory path (e.g. /home/user/workspace)"
              value={customDirInput}
              onChange={(e) => setCustomDirInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addCustomDir()}
              className="flex-1 bg-white/5 border border-white/10 rounded-xl px-3 py-2 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="button"
              onClick={addCustomDir}
              className="px-3 py-2 bg-white/10 hover:bg-white/15 text-slate-200 rounded-xl text-xs flex items-center gap-1 font-medium transition-colors"
            >
              <Plus className="w-3.5 h-3.5" /> Add
            </button>
          </div>
        </div>

        {/* Step 2: Excluded Folders */}
        <div className="space-y-3">
          <label className="text-sm font-semibold text-slate-200 flex items-center gap-2">
            <FolderX className="w-4 h-4 text-red-400" />
            Excluded Folders
          </label>
          <p className="text-xs text-slate-400">
            These folder names will be skipped completely to keep searches instantaneous.
          </p>

          <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto p-1">
            {excludedDirs.map((dir) => (
              <span
                key={dir}
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/5 border border-white/10 text-[11px] text-slate-300"
              >
                {dir}
                <button
                  type="button"
                  onClick={() => removeExcludeDir(dir)}
                  className="text-slate-500 hover:text-red-400 transition-colors"
                >
                  <X className="w-3 h-3" />
                </button>
              </span>
            ))}
          </div>

          <div className="flex gap-2">
            <input
              type="text"
              placeholder="Add folder to ignore (e.g. .cache or builds)"
              value={customExcludeInput}
              onChange={(e) => setCustomExcludeInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addExcludeDir()}
              className="flex-1 bg-white/5 border border-white/10 rounded-xl px-3 py-2 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="button"
              onClick={addExcludeDir}
              className="px-3 py-2 bg-white/10 hover:bg-white/15 text-slate-200 rounded-xl text-xs flex items-center gap-1 font-medium transition-colors"
            >
              <Plus className="w-3.5 h-3.5" /> Add
            </button>
          </div>
        </div>

        {/* Step 3: Global Shortcut */}
        <div className="space-y-2">
          <label className="text-sm font-semibold text-slate-200 flex items-center gap-2">
            <Keyboard className="w-4 h-4 text-emerald-400" />
            Global Shortcut
          </label>
          <div className="flex items-center gap-3">
            <input
              type="text"
              value={shortcut}
              onChange={(e) => setShortcut(e.target.value)}
              className="w-48 bg-white/5 border border-white/10 rounded-xl px-3 py-2 text-xs font-mono text-emerald-400 focus:outline-none focus:border-emerald-500"
            />
            <span className="text-xs text-slate-400">Press anytime to toggle Kensaku search</span>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-white/10 bg-black/30 flex items-center justify-between">
        <span className="text-xs text-slate-500">
          {selectedDirs.length} directory{selectedDirs.length === 1 ? "" : "ies"} selected
        </span>
        <button
          type="button"
          disabled={isSubmitting || selectedDirs.length === 0}
          onClick={handleFinishSetup}
          className="px-5 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 active:scale-95 disabled:opacity-50 text-white font-medium text-xs shadow-lg shadow-blue-600/30 transition-all flex items-center gap-2"
        >
          {isSubmitting ? "Starting Indexing..." : "Start Indexing & Launch"}
        </button>
      </div>
    </div>
  );
};
