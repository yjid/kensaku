import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SearchResult, IndexingStatus } from "./types";
import { SetupModal } from "./components/SetupModal";
import { ResultItem } from "./components/ResultItem";
import { SettingsModal } from "./components/SettingsModal";
import { Search, X, Settings, Loader2, Sparkles, FolderSearch } from "lucide-react";
import "./App.css";

function App() {
  const [isSetupComplete, setIsSetupComplete] = useState<boolean | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [indexingStatus, setIndexingStatus] = useState<IndexingStatus | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [searchTimeMs, setSearchTimeMs] = useState<number | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);

  // 1. Check if setup is already complete
  const checkSetup = useCallback(async () => {
    try {
      const isDone = await invoke<boolean>("is_setup_complete");
      setIsSetupComplete(isDone);
    } catch (err) {
      console.error("Failed to check setup:", err);
      setIsSetupComplete(false);
    }
  }, []);

  useEffect(() => {
    checkSetup();
  }, [checkSetup]);

  // 2. Poll indexing status periodically
  useEffect(() => {
    if (!isSetupComplete) return;

    const fetchStatus = async () => {
      try {
        const st = await invoke<IndexingStatus>("get_indexing_status");
        setIndexingStatus(st);
      } catch (err) {
        console.error("Failed to get indexing status:", err);
      }
    };

    fetchStatus();
    const interval = setInterval(fetchStatus, 1500);
    return () => clearInterval(interval);
  }, [isSetupComplete]);

  // 3. Search query execution with debounce
  useEffect(() => {
    if (!isSetupComplete || query.trim() === "") {
      setResults([]);
      setSelectedIndex(0);
      setSearchTimeMs(null);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(async () => {
      const startTime = performance.now();
      try {
        const res = await invoke<SearchResult[]>("search", { query });
        setResults(res);
        setSelectedIndex(0);
        setSearchTimeMs(Math.round(performance.now() - startTime));
      } catch (err) {
        console.error("Search error:", err);
      } finally {
        setIsSearching(false);
      }
    }, 40);

    return () => clearTimeout(timer);
  }, [query, isSetupComplete]);

  // 4. Focus input on load
  useEffect(() => {
    if (isSetupComplete) {
      inputRef.current?.focus();
    }
  }, [isSetupComplete]);

  // 5. Scroll selected item into view
  useEffect(() => {
    if (!resultsContainerRef.current) return;
    const items = resultsContainerRef.current.children;
    if (items[selectedIndex]) {
      (items[selectedIndex] as HTMLElement).scrollIntoView({
        block: "nearest",
        behavior: "smooth",
      });
    }
  }, [selectedIndex]);

  // 6. Action handlers
  const handleOpenFile = async (path: string) => {
    try {
      await invoke("open_file", { path });
      await invoke("hide_window");
    } catch (err) {
      console.error("Open file error:", err);
    }
  };

  const handleOpenFolder = async (path: string) => {
    try {
      await invoke("open_folder", { path });
      await invoke("hide_window");
    } catch (err) {
      console.error("Open folder error:", err);
    }
  };

  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (showSettings) {
      if (e.key === "Escape") {
        setShowSettings(false);
      }
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (results.length > 0 ? (prev + 1) % results.length : 0));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) =>
        results.length > 0 ? (prev - 1 + results.length) % results.length : 0
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (results[selectedIndex]) {
        if (e.shiftKey || e.ctrlKey) {
          handleOpenFolder(results[selectedIndex].path);
        } else {
          handleOpenFile(results[selectedIndex].path);
        }
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (query) {
        setQuery("");
      } else {
        await invoke("hide_window");
      }
    }
  };

  if (isSetupComplete === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-[#0d0f14] text-slate-400">
        <Loader2 className="w-6 h-6 animate-spin text-blue-500" />
      </div>
    );
  }

  return (
    <div
      onKeyDown={handleKeyDown}
      className="flex flex-col h-screen w-screen bg-[#0e1118]/90 text-slate-100 overflow-hidden font-sans border border-white/10 rounded-2xl shadow-2xl backdrop-blur-2xl select-none"
    >
      {/* Onboarding Setup if not complete */}
      {!isSetupComplete && (
        <SetupModal
          onSetupCompleted={() => {
            setIsSetupComplete(true);
            setTimeout(() => inputRef.current?.focus(), 100);
          }}
        />
      )}

      {/* Main Search Experience */}
      {isSetupComplete && (
        <>
          {/* Top Search Input Bar */}
          <div className="relative flex items-center px-4 py-3.5 border-b border-white/10 bg-white/[0.03]">
            <Search className="w-5 h-5 text-blue-400 mr-3 flex-shrink-0" />

            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Type to search files, code, or text contents..."
              className="flex-1 bg-transparent text-sm sm:text-base text-white placeholder-slate-500 focus:outline-none"
              autoFocus
            />

            {/* Clear / Spinner / Settings */}
            <div className="flex items-center gap-2 flex-shrink-0">
              {isSearching && (
                <Loader2 className="w-4 h-4 animate-spin text-blue-400 mr-1" />
              )}

              {query && (
                <button
                  type="button"
                  onClick={() => {
                    setQuery("");
                    inputRef.current?.focus();
                  }}
                  className="p-1 rounded-md text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
                >
                  <X className="w-4 h-4" />
                </button>
              )}

              <button
                type="button"
                title="Settings"
                onClick={() => setShowSettings(!showSettings)}
                className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
              >
                <Settings className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Results Area */}
          <div
            ref={resultsContainerRef}
            className="flex-1 overflow-y-auto p-2 space-y-1"
          >
            {results.length > 0 ? (
              results.map((res, idx) => (
                <ResultItem
                  key={res.path + idx}
                  result={res}
                  isSelected={idx === selectedIndex}
                  onSelect={() => setSelectedIndex(idx)}
                  onOpen={() => handleOpenFile(res.path)}
                  onOpenFolder={() => handleOpenFolder(res.path)}
                />
              ))
            ) : query.trim() !== "" ? (
              <div className="flex flex-col items-center justify-center h-full text-center py-12 text-slate-400">
                <FolderSearch className="w-10 h-10 text-slate-600 mb-2" />
                <p className="text-xs text-slate-400">No matching files or contents found</p>
                <p className="text-[11px] text-slate-600 mt-1">Try another search keyword or check excluded folders</p>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center h-full text-center py-10 text-slate-500">
                <div className="w-12 h-12 rounded-2xl bg-white/5 border border-white/5 flex items-center justify-center text-blue-400 mb-3 shadow-inner">
                  <Sparkles className="w-6 h-6" />
                </div>
                <h3 className="text-sm font-medium text-slate-300">Kensaku Ready</h3>
                <p className="text-xs text-slate-500 mt-1 max-w-sm">
                  Start typing to find any file or text inside files instantly.
                </p>
                {indexingStatus && (
                  <div className="mt-4 px-3 py-1.5 rounded-full bg-white/[0.04] border border-white/5 text-[11px] text-slate-400 flex items-center gap-2">
                    {indexingStatus.is_indexing && (
                      <span className="w-2 h-2 rounded-full bg-blue-400 animate-pulse" />
                    )}
                    <span>{indexingStatus.current_status}</span>
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Bottom Bar: Shortcuts + Meta */}
          <div className="flex items-center justify-between px-4 py-2 border-t border-white/10 bg-black/40 text-[11px] text-slate-400">
            <div className="flex items-center gap-3">
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">↑↓</kbd>
                <span>Navigate</span>
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">↵</kbd>
                <span>Open</span>
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">⇧↵</kbd>
                <span>Folder</span>
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">esc</kbd>
                <span>Dismiss</span>
              </span>
            </div>

            <div className="flex items-center gap-2 text-slate-400">
              {searchTimeMs !== null && (
                <span>
                  {results.length} result{results.length === 1 ? "" : "s"} ({searchTimeMs}ms)
                </span>
              )}
              {indexingStatus?.is_indexing && (
                <span className="flex items-center gap-1 text-blue-400">
                  <Loader2 className="w-3 h-3 animate-spin" />
                  <span>Indexing...</span>
                </span>
              )}
            </div>
          </div>

          {/* Settings Drawer/Modal */}
          {showSettings && (
            <SettingsModal
              onClose={() => setShowSettings(false)}
              onConfigSaved={() => {
                // Refresh status
                invoke<IndexingStatus>("get_indexing_status").then(setIndexingStatus);
              }}
            />
          )}
        </>
      )}
    </div>
  );
}

export default App;
