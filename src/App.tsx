import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SearchResult, SearchResponse, ActiveFilter, IndexingStatus, CommandOutput } from "./types";
import { SetupModal } from "./components/SetupModal";
import { ResultItem } from "./components/ResultItem";
import { SettingsModal } from "./components/SettingsModal";
import { FilterChips } from "./components/FilterChips";
import { PreviewPane } from "./components/PreviewPane";
import { ActionPalette } from "./components/ActionPalette";
import {
  Search,
  X,
  Settings,
  Loader2,
  Sparkles,
  FolderSearch,
  Terminal,
  ArrowRight,
  Info,
} from "lucide-react";
import "./App.css";

const COMMAND_SUGGESTIONS = [
  { cmd: ">reindex", desc: "Rebuild and re-scan index for watched folders" },
  { cmd: ">doctor", desc: "Run health diagnostics and check database state" },
  { cmd: ">model", desc: "View embedding model specification and status" },
  { cmd: ">add folder", desc: "Configure a new folder path to index" },
  { cmd: ">settings", desc: "Open the application settings dialog" },
];

function App() {
  const [isSetupComplete, setIsSetupComplete] = useState<boolean | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [activeFilters, setActiveFilters] = useState<ActiveFilter[]>([]);
  const [hints, setHints] = useState<string[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [indexingStatus, setIndexingStatus] = useState<IndexingStatus | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showActions, setShowActions] = useState(false);
  const [commandOutput, setCommandOutput] = useState<string | null>(null);
  const [searchTimeMs, setSearchTimeMs] = useState<number | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);
  const currentSeqRef = useRef(0);

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

  // 2. Tauri event listener for indexing status (replaces polling)
  useEffect(() => {
    if (!isSetupComplete) return;

    // Initial fetch once
    invoke<IndexingStatus>("get_indexing_status")
      .then(setIndexingStatus)
      .catch((err) => console.error("Initial status error:", err));

    // Listen to live events emitted by backend
    let unlistenFn: (() => void) | undefined;
    listen<IndexingStatus>("indexing-status", (event) => {
      setIndexingStatus(event.payload);
    })
      .then((unlisten) => {
        unlistenFn = unlisten;
      })
      .catch((err) => console.error("Event listen error:", err));

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [isSetupComplete]);

  // 3. Search query execution with sequence guard & debounce
  useEffect(() => {
    if (!isSetupComplete) return;

    const trimmed = query.trim();
    if (trimmed === "") {
      setResults([]);
      setActiveFilters([]);
      setHints([]);
      setSelectedIndex(0);
      setSearchTimeMs(null);
      setIsSearching(false);
      return;
    }

    if (query.startsWith(">")) {
      setResults([]);
      setActiveFilters([]);
      setHints([]);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(async () => {
      const seq = ++currentSeqRef.current;
      try {
        const res = await invoke<SearchResponse>("search", { query, seq });
        if (res.seq >= currentSeqRef.current) {
          setResults(res.results);
          setActiveFilters(res.active_filters);
          setHints(res.hints);
          setSelectedIndex(0);
          setSearchTimeMs(res.search_time_ms);
        }
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

  const handleRunCommand = async (cmdStr: string) => {
    try {
      const out = await invoke<CommandOutput>("execute_command", { command: cmdStr });
      if (out.message === "SETTINGS") {
        setShowSettings(true);
      } else {
        setCommandOutput(out.message);
      }
    } catch (err) {
      setCommandOutput(String(err));
    }
  };

  const handleRemoveFilter = (filter: ActiveFilter) => {
    const newQuery = query
      .replace(filter.raw, "")
      .replace(/\s+/g, " ")
      .trim();
    setQuery(newQuery);
    inputRef.current?.focus();
  };

  const handleApplyHint = (hint: string) => {
    const match = hint.match(/📁\s*([^\?]+)/);
    if (match && match[1]) {
      const folder = match[1].trim();
      setQuery((prev) => `${prev.trim()} in:"${folder}" `);
    } else {
      setQuery((prev) => `${prev.trim()} ${hint} `);
    }
    inputRef.current?.focus();
  };

  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (showSettings) {
      if (e.key === "Escape") {
        setShowSettings(false);
      }
      return;
    }

    if (showActions) {
      return; // Handled in ActionPalette
    }

    // Ctrl+K / Cmd+K triggers Action Palette on selected result
    if (e.key.toLowerCase() === "k" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (results[selectedIndex]) {
        setShowActions(true);
      }
      return;
    }

    // Command mode execution on Enter
    if (query.startsWith(">")) {
      if (e.key === "Enter") {
        e.preventDefault();
        handleRunCommand(query);
      } else if (e.key === "Escape") {
        e.preventDefault();
        setQuery("");
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
        if (e.shiftKey) {
          handleOpenFolder(results[selectedIndex].path);
        } else {
          handleOpenFile(results[selectedIndex].path);
        }
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (commandOutput) {
        setCommandOutput(null);
      } else if (query) {
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

  const isCommandMode = query.startsWith(">");
  const filteredCommands = COMMAND_SUGGESTIONS.filter((s) =>
    s.cmd.toLowerCase().includes(query.toLowerCase())
  );

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
            {isCommandMode ? (
              <Terminal className="w-5 h-5 text-amber-400 mr-3 flex-shrink-0" />
            ) : (
              <Search className="w-5 h-5 text-blue-400 mr-3 flex-shrink-0" />
            )}

            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search files, contents, filters (type:pdf in:~/Projects) or > for commands..."
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
                    setCommandOutput(null);
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

          {/* Active Filter Chips & Hints */}
          {!isCommandMode && (
            <FilterChips
              filters={activeFilters}
              hints={hints}
              onRemoveFilter={handleRemoveFilter}
              onApplyHint={handleApplyHint}
            />
          )}

          {/* Command Output Banner if any */}
          {commandOutput && (
            <div className="flex items-start justify-between gap-3 px-4 py-2.5 bg-blue-600/15 border-b border-blue-500/20 text-xs text-blue-200">
              <div className="flex items-start gap-2 min-w-0">
                <Info className="w-4 h-4 text-blue-400 flex-shrink-0 mt-0.5" />
                <pre className="font-mono text-[11px] whitespace-pre-wrap break-all">
                  {commandOutput}
                </pre>
              </div>
              <button
                type="button"
                onClick={() => setCommandOutput(null)}
                className="text-slate-400 hover:text-white flex-shrink-0"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          )}

          {/* Split Content Area: Results List + Right-Side Preview Pane */}
          <div className="flex-1 flex min-h-0 overflow-hidden">
            {/* Left: Result List or Command Suggestions */}
            <div
              ref={resultsContainerRef}
              className="flex-1 overflow-y-auto p-2 space-y-1"
            >
              {isCommandMode ? (
                <div className="p-2 space-y-1.5">
                  <div className="px-2 py-1 text-[11px] font-medium text-amber-400/90 uppercase tracking-wider">
                    Command Mode (Press Enter to execute)
                  </div>
                  {filteredCommands.map((c) => (
                    <div
                      key={c.cmd}
                      onClick={() => {
                        setQuery(c.cmd);
                        handleRunCommand(c.cmd);
                      }}
                      className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] hover:bg-white/[0.08] border border-white/5 cursor-pointer transition-colors"
                    >
                      <div className="flex items-center gap-2.5">
                        <Terminal className="w-4 h-4 text-amber-400" />
                        <span className="font-mono text-xs text-white font-medium">
                          {c.cmd}
                        </span>
                        <span className="text-xs text-slate-400">{c.desc}</span>
                      </div>
                      <ArrowRight className="w-3.5 h-3.5 text-slate-500" />
                    </div>
                  ))}
                </div>
              ) : results.length > 0 ? (
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
                  <p className="text-xs text-slate-400">No matching files found</p>
                  <p className="text-[11px] text-slate-600 mt-1">
                    Try another keyword, remove filters, or check excluded folders
                  </p>
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-center py-10 text-slate-500">
                  <div className="w-12 h-12 rounded-2xl bg-white/5 border border-white/5 flex items-center justify-center text-blue-400 mb-3 shadow-inner">
                    <Sparkles className="w-6 h-6" />
                  </div>
                  <h3 className="text-sm font-medium text-slate-300">Kensaku Ready</h3>
                  <p className="text-xs text-slate-500 mt-1 max-w-sm">
                    Search files, text inside files, or use filters like <code className="text-slate-300">type:pdf</code>, <code className="text-slate-300">in:~/Projects</code>, <code className="text-slate-300">size:&gt;2mb</code>.
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

            {/* Right: Preview Pane */}
            {!isCommandMode && (
              <PreviewPane
                result={results[selectedIndex] || null}
                searchQuery={query}
                onOpen={() => handleOpenFile(results[selectedIndex].path)}
                onOpenFolder={() => handleOpenFolder(results[selectedIndex].path)}
                onOpenActions={() => setShowActions(true)}
              />
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
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">⌘K</kbd>
                <span>Actions</span>
              </span>
              <span className="flex items-center gap-1">
                <kbd className="px-1.5 py-0.5 rounded bg-white/10 text-slate-300 font-mono text-[10px]">&gt;</kbd>
                <span>Commands</span>
              </span>
            </div>

            <div className="flex items-center gap-2.5 text-slate-400">
              {/* Semantic Status Indicator */}
              {indexingStatus?.is_embedding ? (
                <span
                  className="flex items-center gap-1 text-cyan-400 text-[11px] font-medium"
                  title="Computing static embeddings in background..."
                >
                  <Loader2 className="w-3 h-3 animate-spin" />
                  <span>semantic (embedding...)</span>
                </span>
              ) : indexingStatus?.semantic_ready ? (
                <span
                  className="flex items-center gap-1 text-emerald-400/90 text-[11px] font-medium"
                  title={`${indexingStatus.units_indexed ?? 0} semantic units active in RRF fusion`}
                >
                  <span>semantic ✓</span>
                </span>
              ) : null}

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

          {/* Action Palette Modal (⌘K / Ctrl+K) */}
          {showActions && results[selectedIndex] && (
            <ActionPalette
              result={results[selectedIndex]}
              onClose={() => setShowActions(false)}
              onOpen={() => handleOpenFile(results[selectedIndex].path)}
              onOpenFolder={() => handleOpenFolder(results[selectedIndex].path)}
              onCopyPath={() => {
                navigator.clipboard.writeText(results[selectedIndex].path);
              }}
              onCopyContent={async () => {
                try {
                  const content = await invoke<string>("read_file_content", {
                    path: results[selectedIndex].path,
                  });
                  navigator.clipboard.writeText(content);
                } catch (e) {
                  console.error(e);
                }
              }}
              onFindSimilar={(simQuery) => {
                setQuery(simQuery);
                inputRef.current?.focus();
              }}
              onAsk={() => {
                setCommandOutput(`Ask about "${results[selectedIndex].filename}": Ollama integration configured for Phase 6.`);
              }}
            />
          )}

          {/* Settings Drawer/Modal */}
          {showSettings && (
            <SettingsModal
              onClose={() => setShowSettings(false)}
              onConfigSaved={() => {
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
