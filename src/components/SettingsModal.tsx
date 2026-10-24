import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { KensakuConfig } from "../types";
import { Folder, FolderX, Keyboard, X, Plus, Save, RefreshCw } from "lucide-react";

interface SettingsModalProps {
  onClose: () => void;
  onConfigSaved: () => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ onClose, onConfigSaved }) => {
  const [config, setConfig] = useState<KensakuConfig | null>(null);
  const [customDirInput, setCustomDirInput] = useState("");
  const [customExcludeInput, setCustomExcludeInput] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    invoke<KensakuConfig>("get_config")
      .then(setConfig)
      .catch((err) => console.error("Failed to load config:", err));
  }, []);

  if (!config) return null;

  const removeDir = (dir: string) => {
    setConfig({
      ...config,
      watched_dirs: config.watched_dirs.filter((d) => d !== dir),
    });
  };

  const addDir = () => {
    const trimmed = customDirInput.trim();
    if (trimmed && !config.watched_dirs.includes(trimmed)) {
      setConfig({
        ...config,
        watched_dirs: [...config.watched_dirs, trimmed],
      });
      setCustomDirInput("");
    }
  };

  const removeExclude = (dir: string) => {
    setConfig({
      ...config,
      excluded_dirs: config.excluded_dirs.filter((d) => d !== dir),
    });
  };

  const addExclude = () => {
    const trimmed = customExcludeInput.trim();
    if (trimmed && !config.excluded_dirs.includes(trimmed)) {
      setConfig({
        ...config,
        excluded_dirs: [...config.excluded_dirs, trimmed],
      });
      setCustomExcludeInput("");
    }
  };

  const handleSave = async (reindex: boolean = false) => {
    setIsSaving(true);
    try {
      await invoke("save_config", { config });
      if (reindex) {
        await invoke("complete_setup", {
          watchedDirs: config.watched_dirs,
          excludedDirs: config.excluded_dirs,
          shortcut: config.shortcut,
        });
      }
      onConfigSaved();
      onClose();
    } catch (err) {
      alert("Failed to save settings: " + err);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="absolute inset-0 bg-[#0f1117]/95 z-50 flex flex-col rounded-2xl border border-white/10 backdrop-blur-2xl">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-white/10 bg-white/[0.02]">
        <h2 className="text-sm font-semibold text-white">Kensaku Settings</h2>
        <button
          type="button"
          onClick={onClose}
          className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-5 space-y-5 text-xs">
        {/* Shortcut */}
        <div className="space-y-2">
          <label className="font-medium text-slate-300 flex items-center gap-1.5">
            <Keyboard className="w-3.5 h-3.5 text-emerald-400" /> Global Shortcut
          </label>
          <input
            type="text"
            value={config.shortcut}
            onChange={(e) => setConfig({ ...config, shortcut: e.target.value })}
            className="w-48 bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-emerald-400 font-mono focus:outline-none focus:border-emerald-500"
          />
        </div>

        {/* Watched Dirs */}
        <div className="space-y-2">
          <label className="font-medium text-slate-300 flex items-center gap-1.5">
            <Folder className="w-3.5 h-3.5 text-blue-400" /> Indexed Directories
          </label>
          <div className="space-y-1.5 max-h-32 overflow-y-auto pr-1">
            {config.watched_dirs.map((dir) => (
              <div
                key={dir}
                className="flex items-center justify-between bg-white/5 border border-white/5 rounded-lg px-2.5 py-1.5 text-slate-300"
              >
                <span className="truncate">{dir}</span>
                <button
                  type="button"
                  onClick={() => removeDir(dir)}
                  className="text-slate-500 hover:text-red-400 ml-2"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
          <div className="flex gap-2">
            <input
              type="text"
              placeholder="Add folder path..."
              value={customDirInput}
              onChange={(e) => setCustomDirInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addDir()}
              className="flex-1 bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="button"
              onClick={addDir}
              className="px-2.5 py-1.5 bg-white/10 hover:bg-white/15 text-slate-200 rounded-lg flex items-center gap-1"
            >
              <Plus className="w-3 h-3" /> Add
            </button>
          </div>
        </div>

        {/* Excluded Dirs */}
        <div className="space-y-2">
          <label className="font-medium text-slate-300 flex items-center gap-1.5">
            <FolderX className="w-3.5 h-3.5 text-red-400" /> Excluded Directory Names
          </label>
          <div className="flex flex-wrap gap-1.5 max-h-24 overflow-y-auto pr-1">
            {config.excluded_dirs.map((dir) => (
              <span
                key={dir}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-[11px] text-slate-300"
              >
                {dir}
                <button
                  type="button"
                  onClick={() => removeExclude(dir)}
                  className="text-slate-500 hover:text-red-400"
                >
                  <X className="w-3 h-3" />
                </button>
              </span>
            ))}
          </div>
          <div className="flex gap-2">
            <input
              type="text"
              placeholder="Add folder name to exclude..."
              value={customExcludeInput}
              onChange={(e) => setCustomExcludeInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addExclude()}
              className="flex-1 bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500"
            />
            <button
              type="button"
              onClick={addExclude}
              className="px-2.5 py-1.5 bg-white/10 hover:bg-white/15 text-slate-200 rounded-lg flex items-center gap-1"
            >
              <Plus className="w-3 h-3" /> Add
            </button>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="p-3 border-t border-white/10 bg-black/20 flex items-center justify-between">
        <button
          type="button"
          onClick={() => handleSave(true)}
          disabled={isSaving}
          className="px-3 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-slate-300 text-xs flex items-center gap-1.5 border border-white/10 transition-colors"
        >
          <RefreshCw className="w-3 h-3" /> Save & Re-index
        </button>
        <button
          type="button"
          onClick={() => handleSave(false)}
          disabled={isSaving}
          className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium text-xs flex items-center gap-1.5 shadow-md shadow-blue-500/20 transition-all"
        >
          <Save className="w-3 h-3" /> Save
        </button>
      </div>
    </div>
  );
};
