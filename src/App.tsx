import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  SearchResult,
  SearchResponse,
  ActiveFilter,
  IndexingStatus,
  CommandOutput,
  SystemStats,
  emptyStats,
} from "./types";
import { SetupModal } from "./components/SetupModal";
import { ResultItem, ResultView } from "./components/ResultItem";
import { Settings } from "./components/SettingsModal";
import { FilterChips } from "./components/FilterChips";
import { PreviewPane } from "./components/PreviewPane";
import { SearchBar } from "./components/SearchBar";
import { CategoryChips } from "./components/CategoryChips";
import { StatusBar } from "./components/StatusBar";
import { HomeGrid } from "./components/HomeGrid";
import { Palette, PaletteGroup, PaletteItem } from "./components/Palette";
import { EmptyState } from "./components/EmptyState";
import { useTheme } from "./hooks/useTheme";
import { useStatsSlow } from "./hooks/useStats";
import { CATEGORY_CHIPS, CategoryChip } from "./lib/categories";
import {
  Eye,
  EyeOff,
  LayoutGrid,
  List,
  Settings as SettingsIcon,
  RefreshCw,
  ExternalLink,
  FolderOpen,
  Copy,
  FileText,
  Sun,
  Moon,
  Image as ImageIcon,
  Folder as FolderIcon,
  FileCode,
  File as FileIcon,
  Film,
  Clock,
  Terminal,
  ClipboardCopy,
  X,
} from "lucide-react";
import "./App.css";

const COMMAND_SUGGESTIONS = [
  { cmd: ">reindex", desc: "Rebuild and re-scan index for watched folders" },
  { cmd: ">doctor", desc: "Run health diagnostics and check database state" },
  { cmd: ">model", desc: "View embedding model specification and status" },
  { cmd: ">add folder", desc: "Configure a new folder path to index" },
  { cmd: ">settings", desc: "Open the application settings dialog" },
];

const VIEW_KEY = "kensaku.view";
const PREVIEW_KEY = "kensaku.preview";

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
  const [settingsSection, setSettingsSection] = useState<string | undefined>(undefined);
  const [showPalette, setShowPalette] = useState(false);
  const [commandOutput, setCommandOutput] = useState<string | null>(null);
  const [searchTimeMs, setSearchTimeMs] = useState<number | null>(null);
  const [view, setView] = useState<ResultView>(
    () => (localStorage.getItem(VIEW_KEY) === "grid" ? "grid" : "list") as ResultView,
  );
  const [previewOpen, setPreviewOpen] = useState<boolean>(
    () => localStorage.getItem(PREVIEW_KEY) !== "off",
  );

  const { theme, toggle: toggleTheme } = useTheme();
  const { stats: slowStats, live: slowStatsLive } = useStatsSlow();

  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);
  const currentSeqRef = useRef(0);

  /* ---------------- setup gating ---------------- */

  const checkSetup = useCallback(async () => {
    try {
      const done = await invoke<boolean>("is_setup_complete");
      setIsSetupComplete(done);
    } catch (err) {
      console.error("Failed to check setup:", err);
      setIsSetupComplete(false);
    }
  }, []);

  useEffect(() => {
    checkSetup();
  }, [checkSetup]);

  /* ---------------- indexing status (events, not polling) ---------------- */

  useEffect(() => {
    if (!isSetupComplete) return;
    invoke<IndexingStatus>("get_indexing_status")
      .then(setIndexingStatus)
      .catch((err) => console.error("Initial status error:", err));

    let unlisten: (() => void) | undefined;
    listen<IndexingStatus>("indexing-status", (event) => setIndexingStatus(event.payload))
      .then((off) => {
        unlisten = off;
      })
      .catch((err) => console.error("Event listen error:", err));

    return () => {
      if (unlisten) unlisten();
    };
  }, [isSetupComplete]);

  /* Search only. Ingestion uses the periodic slow stats poll above. */

  /* ---------------- search (stale-while-revalidate) ---------------- */

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
        const res = await invoke<SearchResponse>("search", { query });
        if (seq >= currentSeqRef.current) {
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

  // Results never reorder while typing — sort is backend-provided and stable;
  // nothing needed here beyond identity-stable render keys.

  /* ---------------- recent files for the home bento ---------------- */

  const [recent, setRecent] = useState<SearchResult[]>([]);
  useEffect(() => {
    if (!isSetupComplete) return;
    invoke<SearchResponse>("search", { query: "modified:<7d" })
      .then((res) => setRecent(res.results))
      .catch(() => setRecent([]));
  }, [isSetupComplete]);

  /* ---------------- focus + persistence ---------------- */

  useEffect(() => {
    if (isSetupComplete) inputRef.current?.focus();
  }, [isSetupComplete]);

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);
  useEffect(() => {
    localStorage.setItem(PREVIEW_KEY, previewOpen ? "on" : "off");
  }, [previewOpen]);

  useEffect(() => {
    if (!resultsContainerRef.current) return;
    const el = resultsContainerRef.current.querySelector<HTMLElement>(
      `[data-index="${selectedIndex}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex, view]);

  /* ---------------- actions ---------------- */

  const handleOpenFile = useCallback(async (path: string) => {
    try {
      await invoke("open_file", { path });
      await invoke("hide_window");
    } catch (err) {
      console.error("Open file error:", err);
    }
  }, []);

  const handleOpenFolder = useCallback(async (path: string) => {
    try {
      await invoke("open_folder", { path });
      await invoke("hide_window");
    } catch (err) {
      console.error("Open folder error:", err);
    }
  }, []);

  const handleRunCommand = useCallback(async (cmdStr: string) => {
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
  }, []);

  const handleRemoveFilter = useCallback((filter: ActiveFilter) => {
    setQuery((prev) =>
      prev
        .replace(filter.raw, "")
        .replace(/\s+/g, " ")
        .trim(),
    );
    inputRef.current?.focus();
  }, []);

  const handleApplyHint = useCallback((hint: string) => {
    const match = hint.match(/📁\s*([^\?]+)/);
    setQuery((prev) =>
      match && match[1]
        ? `${prev.trim()} in:"${match[1].trim()}"`
        : `${prev.trim()} ${hint}`.trim(),
    );
    inputRef.current?.focus();
  }, []);

  /* Category chip → query token. */
  const handleChip = useCallback((chip: CategoryChip) => {
    setQuery((prev) => {
      let next = prev
        .replace(/\btype:[^\s]+/gi, "")
        .replace(/\bmodified:<[^\s]+/gi, "")
        .replace(/\s+/g, " ")
        .trim();
      if (chip.id === "all") return next;
      next = next ? `${next} ${chip.token}` : chip.token!;
      return next;
    });
    inputRef.current?.focus();
  }, []);

  const openSettings = useCallback((section?: string) => {
    setSettingsSection(section);
    setShowPalette(false);
    setShowSettings(true);
  }, []);

  const handleCopyPath = useCallback((path: string) => {
    navigator.clipboard.writeText(path);
  }, []);

  const selected: SearchResult | null = results[selectedIndex] ?? null;

  /* ---------------- global palette ---------------- */

  const paletteGroups = useMemo((): PaletteGroup[] => {
    const groups: PaletteGroup[] = [];

    if (selected) {
      const fileActions: PaletteItem[] = [
        { id: "open", label: `Open ${selected.filename}`, shortcut: ["↵"], icon: ExternalLink, run: () => handleOpenFile(selected.path) },
        { id: "reveal", label: "Reveal in file manager", hint: "Shift+↵", icon: FolderOpen, run: () => handleOpenFolder(selected.path) },
        { id: "copypath", label: "Copy absolute path", icon: Copy, run: () => handleCopyPath(selected.path) },
      ];
      if (!selected.is_dir) {
        fileActions.push({
          id: "copycontent",
          label: "Copy file text content",
          icon: FileText,
          run: async () => {
            try {
              const content = await invoke<string>("read_file_content", { path: selected.path });
              navigator.clipboard.writeText(content);
            } catch (e) {
              console.error(e);
            }
          },
        });
        fileActions.push({
          id: "similar",
          label: "Find similar files",
          icon: ImageIcon,
          run: () => {
            const ext = selected.filename.split(".").pop();
            setQuery(ext && ext !== selected.filename ? `ext:${ext}` : selected.filename.slice(0, 4));
            inputRef.current?.focus();
          },
        });
      }
      groups.push({ label: selected.filename, items: fileActions });
    }

    groups.push({
      label: "Actions",
      items: [
        {
          id: "toggle-preview",
          label: previewOpen ? "Hide preview pane" : "Show preview pane",
          hint: "Tab",
          icon: previewOpen ? EyeOff : Eye,
          run: () => setPreviewOpen((p) => !p),
        },
        {
          id: "toggle-view",
          label: view === "list" ? "Switch to grid cards" : "Switch to list rows",
          icon: view === "list" ? LayoutGrid : List,
          run: () => setView((v) => (v === "list" ? "grid" : "list")),
        },
        {
          id: "reindex",
          label: "Reindex all folders",
          icon: RefreshCw,
          run: () => handleRunCommand(">reindex"),
        },
      ],
    });

    groups.push({
      label: "Filters",
      items: CATEGORY_CHIPS.filter((c) => c.token).map((c) => ({
        id: `chip-${c.id}`,
        label: `Filter: ${c.label}`,
        hint: c.token,
        icon: categoryIcon(c.id),
        run: () => handleChip(c),
      })),
    });

    groups.push({
      label: "Settings",
      items: [
        { id: "open-settings", label: "Open settings", hint: "⌘,", icon: SettingsIcon, run: () => openSettings() },
        {
          id: "theme",
          label: theme === "paper" ? "Switch to void theme" : "Switch to paper theme",
          icon: theme === "paper" ? Moon : Sun,
          run: toggleTheme,
        },
        {
          id: "copy-stats",
          label: "Copy system stats JSON",
          icon: ClipboardCopy,
          run: async () => {
            let s: SystemStats = emptyStats();
            try {
              s = await invoke<SystemStats>("get_stats");
            } catch {
              /* keeps zeroed fallback */
            }
            navigator.clipboard.writeText(JSON.stringify(s, null, 2));
          },
        },
      ],
    });

    return groups;
  }, [selected, previewOpen, view, theme, handleOpenFile, handleOpenFolder, handleCopyPath, handleRunCommand, handleChip, openSettings, toggleTheme]);

  /* ---------------- keyboard ---------------- */

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent) => {
      // Overlays capture their own Esc; everything else pauses the main list.
      if (showPalette) return;
      if (showSettings) return;

      if (e.key === "Tab") {
        e.preventDefault();
        setPreviewOpen((p) => !p);
        return;
      }

      // "/" focuses search when the focus is elsewhere.
      if (e.key === "/" && document.activeElement !== inputRef.current) {
        e.preventDefault();
        inputRef.current?.focus();
        return;
      }

      if (e.key.toLowerCase() === "k" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setShowPalette(true);
        return;
      }

      if (e.key === "," && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        openSettings();
        return;
      }

      // Command mode execution
      if (query.startsWith(">")) {
        if (e.key === "Enter") {
          e.preventDefault();
          handleRunCommand(query.trim());
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
          results.length > 0 ? (prev - 1 + results.length) % results.length : 0,
        );
      } else if (e.key === "Enter" && results[selectedIndex]) {
        e.preventDefault();
        const target = results[selectedIndex];
        if (e.metaKey || e.ctrlKey) {
          // "Open at line" — backend currently opens at default; reveal path is a safe stand-in.
          handleOpenFile(target.path);
        } else if (e.shiftKey) {
          handleOpenFolder(target.path);
        } else {
          handleOpenFile(target.path);
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        if (commandOutput) {
          setCommandOutput(null);
        } else if (query) {
          setQuery("");
          inputRef.current?.focus();
        } else {
          await invoke("hide_window");
        }
      }
    },
    [showPalette, showSettings, query, results, selectedIndex, commandOutput, handleRunCommand, handleOpenFile, handleOpenFolder, openSettings],
  );

  /* ---------------- render ---------------- */

  if (isSetupComplete === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg">
        <span className="h-4 w-4 animate-pulse rounded-full bg-accent" />
      </div>
    );
  }

  const isCommandMode = query.startsWith(">");
  const filteredCommands = COMMAND_SUGGESTIONS.filter((s) =>
    s.cmd.toLowerCase().includes(query.toLowerCase()),
  );

  const hasQuery = query.trim() !== "";
  const noResults = hasQuery && !isCommandMode && results.length === 0;
  const isIndexing = indexingStatus?.is_indexing ?? false;
  // Home bento owns the full width; the preview pane would just be empty chrome.
  const showPreview = !isCommandMode && previewOpen && hasQuery;

  return (
    <div
      onKeyDown={handleKeyDown}
      className="flex h-screen w-screen select-none flex-col overflow-hidden bg-bg text-fg"
    >
      {isSetupComplete === false && (
        <SetupModal
          onSetupCompleted={() => {
            setIsSetupComplete(true);
            window.setTimeout(() => inputRef.current?.focus(), 120);
          }}
        />
      )}

      {isSetupComplete === true && (
        <>
          {/* Search bar */}
          {showSettings && (
            <div className="absolute inset-0 z-40">
              <Settings
                onClose={() => setShowSettings(false)}
                initialSection={settingsSection}
                onConfigSaved={() => {
                  invoke<IndexingStatus>("get_indexing_status").then(setIndexingStatus);
                }}
              />
            </div>
          )}

          {!showSettings && (
            <>
              <SearchBar
                query={query}
                setQuery={setQuery}
                inputRef={inputRef}
                isSearching={isSearching}
                onOpenPalette={() => setShowPalette(true)}
                onFocusInput={() => {}}
                resultCount={results.length}
              />

              {/* Global indexing progress: 2px accent line under the bar.
                  Stale results stay visible — never a spinner over the list. */}
              {isIndexing && (
                <div className="progress-track shrink-0 px-0">
                  <div className="progress-bar" />
                </div>
              )}

              {/* Category chips */}
              {!isCommandMode && (
                <CategoryChips query={query} results={results} onPick={handleChip} />
              )}

              {/* Active backend filters + hints */}
              {!isCommandMode && (
                <FilterChips
                  filters={activeFilters}
                  hints={hints}
                  onRemoveFilter={handleRemoveFilter}
                  onApplyHint={handleApplyHint}
                />
              )}

              {/* Command output banner */}
              {commandOutput && (
                <div className="flex shrink-0 items-start justify-between gap-3 border-b border-line bg-elevated px-4 py-2">
                  <div className="flex min-w-0 items-start gap-2">
                    <Terminal className="mt-0.5 h-3.5 w-3.5 shrink-0 text-accent" />
                    <pre className="select-text whitespace-pre-wrap break-all font-mono text-[10.5px] leading-relaxed text-muted">
                      {commandOutput}
                    </pre>
                  </div>
                  <button
                    type="button"
                    onClick={() => setCommandOutput(null)}
                    className="shrink-0 text-subtle transition-colors hover:text-fg"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </div>
              )}

              {/* Content: list/grid + preview */}
              <div className="flex min-h-0 flex-1 overflow-hidden">
                <div
                  ref={resultsContainerRef}
                  className={`min-w-0 flex-1 overflow-y-auto ${
                    view === "grid"
                      ? "grid grid-cols-[repeat(auto-fill,160px)] justify-start gap-2 p-3"
                      : "px-2 py-1"
                  }`}
                >
                  {isCommandMode ? (
                    <div className="p-1.5">
                      <p className="eyebrow pb-2 pt-1">commands — Enter executes</p>
                      {filteredCommands.length === 0 ? (
                        <p className="px-2 py-4 text-[12px] text-subtle">Unknown command</p>
                      ) : (
                        filteredCommands.map((c) => (
                          <button
                            key={c.cmd}
                            type="button"
                            onClick={() => {
                              setQuery(c.cmd);
                              handleRunCommand(c.cmd);
                            }}
                            className="flex h-10 w-full items-center gap-2.5 rounded-[10px] px-2.5 text-left transition-colors duration-150 hover:bg-sunken"
                          >
                            <Terminal className="h-4 w-4 shrink-0 text-warning" />
                            <span className="font-mono text-[12px] font-medium text-fg">{c.cmd}</span>
                            <span className="min-w-0 flex-1 truncate text-[12px] text-muted">{c.desc}</span>
                            <kbd className="kbd">↵</kbd>
                          </button>
                        ))
                      )}
                    </div>
                  ) : results.length > 0 ? (
                    view === "list" ? (
                      results.map((res, idx) => (
                        <div key={res.path + idx} data-index={idx}>
                          <ResultItem
                            result={res}
                            query={query}
                            index={idx}
                            isSelected={idx === selectedIndex}
                            view="list"
                            onSelect={() => setSelectedIndex(idx)}
                            onOpen={() => handleOpenFile(res.path)}
                            onReveal={() => handleOpenFolder(res.path)}
                            onCopyPath={() => handleCopyPath(res.path)}
                          />
                        </div>
                      ))
                    ) : (
                      results.map((res, idx) => (
                        <div key={res.path + idx} data-index={idx}>
                        <ResultItem
                          result={res}
                          query={query}
                          index={idx}
                          isSelected={idx === selectedIndex}
                          view="grid"
                          onSelect={() => setSelectedIndex(idx)}
                          onOpen={() => handleOpenFile(res.path)}
                          onReveal={() => handleOpenFolder(res.path)}
                          onCopyPath={() => handleCopyPath(res.path)}
                        />
                        </div>
                      ))
                    )
                  ) : noResults ? (
                    <EmptyState variant="noresults" caption={`Nothing matches ${query.trim()}`}>
                      <div className="flex flex-wrap items-center justify-center gap-1.5">
                        {hasActiveFilters(query) && (
                          <SuggestChip label="Remove filters" onClick={() => setQuery("")} />
                        )}
                        <SuggestChip
                          label="Try type:pdf"
                          onClick={() => {
                            setQuery("type:pdf");
                            inputRef.current?.focus();
                          }}
                        />
                        <SuggestChip
                          label="Nothing but folders"
                          onClick={() => {
                            setQuery("type:dir");
                            inputRef.current?.focus();
                          }}
                        />
                      </div>
                    </EmptyState>
                  ) : (
                    /* Home — empty query bento */
                    <HomeGrid
                      recent={recent}
                      stats={slowStats}
                      statsLive={slowStatsLive}
                      onOpenFile={handleOpenFile}
                      onSelectResult={(r) => {
                        setQuery(r.filename);
                        inputRef.current?.focus();
                      }}
                      onAddFolder={() => openSettings("folders")}
                      onReindex={() => handleRunCommand(">reindex")}
                      onOpenSettings={() => openSettings()}
                      onToggleView={() => setView((v) => (v === "list" ? "grid" : "list"))}
                      view={view}
                      onTogglePreview={() => setPreviewOpen((p) => !p)}
                      previewOpen={previewOpen}
                      onRunQuery={(q) => {
                        setQuery(q);
                        inputRef.current?.focus();
                      }}
                    />
                  )}
                </div>

                {/* Preview pane, hidden < 900px */}
                {showPreview && (
                  <PreviewPane
                    result={selected}
                    searchQuery={query}
                    onOpen={() => selected && handleOpenFile(selected.path)}
                    onOpenFolder={() => selected && handleOpenFolder(selected.path)}
                    onOpenActions={() => setShowPalette(true)}
                  />
                )}
              </div>

              {/* Status bar */}
              <StatusBar
                searchTimeMs={searchTimeMs}
                resultCount={results.length}
                indexingStatus={indexingStatus}
                stats={
                  slowStats
                    ? {
                        files: slowStats.index.files,
                        lastReconcileSecsAgo: slowStats.index.last_reconcile_secs_ago,
                        watcherOk: slowStats.runtime.watcher_ok || !slowStatsLive,
                      }
                    : null
                }
                theme={theme}
                onToggleTheme={toggleTheme}
                onOpenSettings={openSettings}
              />

              {/* Palette */}
              {showPalette && (
                <Palette
                  title={selected ? "Kensaku · file actions" : "Kensaku · command palette"}
                  placeholder={selected ? "Type a command…" : "Type a command or filter…"}
                  groups={paletteGroups}
                  onClose={() => setShowPalette(false)}
                />
              )}
            </>
          )}
        </>
      )}
    </div>
  );
}

/* Chips rendered under a no-results illustration. */
function SuggestChip({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-6 items-center rounded-full border border-line px-2.5 font-mono text-[10.5px] text-muted transition-colors duration-150 hover:border-line-strong hover:text-fg"
    >
      {label}
    </button>
  );
}

function hasActiveFilters(q: string): boolean {
  return /\b(type|ext|in|modified|size):/i.test(q);
}

function categoryIcon(id: string): React.ComponentType<{ className?: string }> {
  switch (id) {
    case "image":
      return ImageIcon;
    case "folder":
      return FolderIcon;
    case "code":
      return FileCode;
    case "media":
      return Film;
    case "file":
      return FileIcon;
    default:
      return Clock;
  }
}

export default App;
