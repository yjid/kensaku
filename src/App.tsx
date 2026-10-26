import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  SearchResult,
  SearchResponse,
  SnippetDto,
  ActiveFilter,
  IndexingStatus,
  CommandOutput,
  InterpretationChip,
  Facets,
  HomeData,
  KensakuConfig,
} from "./types";
import { SetupModal } from "./components/SetupModal";
import { DuplicatesModal } from "./components/DuplicatesModal";
import { SlashMenu, SlashCommand } from "./components/SlashMenu";
import { ScopeMenu } from "./components/ScopeMenu";
import { HomeView } from "./components/HomeView";
import { ResultItem, ResultView } from "./components/ResultItem";
import { Settings } from "./components/SettingsModal";
import { FilterChips } from "./components/FilterChips";
import { PreviewPane } from "./components/PreviewPane";
import { SearchBar } from "./components/SearchBar";
import { CategoryChips } from "./components/CategoryChips";
import { EmptyState } from "./components/EmptyState";
import { useTheme } from "./hooks/useTheme";
import { classify } from "./lib/categories";
import {
  Settings as SettingsIcon,
  RefreshCw,
  Copy,
  FileText,
  Sun,
  Moon,
  Image as ImageIcon,
  FileCode,
  Film,
  Clock,
  Terminal,
  Stethoscope,
  TriangleAlert,
  Bookmark,
  X,
} from "lucide-react";
import "./App.css";

const VIEW_KEY = "kensaku.view";
const PREVIEW_KEY = "kensaku.preview";

function App() {
  const [isSetupComplete, setIsSetupComplete] = useState<boolean | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [total, setTotal] = useState(0);
  const [activeFilters, setActiveFilters] = useState<ActiveFilter[]>([]);
  const [hints, setHints] = useState<string[]>([]);
  const [interpretation, setInterpretation] = useState<InterpretationChip[]>([]);
  const [facets, setFacets] = useState<Facets | undefined>(undefined);
  const [home, setHome] = useState<HomeData | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [facetInclude, setFacetInclude] = useState<Set<string>>(new Set());
  const [facetExclude, setFacetExclude] = useState<Set<string>>(new Set());
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [indexingStatus, setIndexingStatus] = useState<IndexingStatus | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [settingsSection, setSettingsSection] = useState<string | undefined>(undefined);
  const [showDuplicates, setShowDuplicates] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const [scopeItems, setScopeItems] = useState<string[]>([]);
  const [scopeIndex, setScopeIndex] = useState(0);
  const [commandOutput, setCommandOutput] = useState<string | null>(null);
  const [sort, setSort] = useState("best");
  const [view] = useState<ResultView>(
    () => (localStorage.getItem(VIEW_KEY) === "grid" ? "grid" : "list") as ResultView,
  );
  const [previewOpen, setPreviewOpen] = useState<boolean>(
    () => localStorage.getItem(PREVIEW_KEY) !== "off",
  );

  const { theme, toggle: toggleTheme } = useTheme();

  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);

  /** Number of cards in the first grid row (1 in list view). Measured from
   *  the DOM so it stays correct across window resizes and auto-fill.
   *  Only `data-index` cells count — full-span labels ("Recent") sit in
   *  the grid too but span every column. */
  const gridColumnCount = useCallback(() => {
    const el = resultsContainerRef.current;
    if (!el) return 1;
    const children = Array.from(el.children).filter((child) =>
      child.hasAttribute("data-index"),
    ) as HTMLElement[];
    if (children.length === 0) return 1;
    const firstTop = children[0].offsetTop;
    let count = 0;
    for (const child of children) {
      if (child.offsetTop !== firstTop) break;
      count += 1;
    }
    return Math.max(1, count);
  }, []);
  const currentSeqRef = useRef(0);
  /** Query at the time the current effect run scheduled its requests —
   *  snippet merges are discarded if the user has typed since. */
  const queryRef = useRef(query);
  /** Snippets already fetched for a path, across queries ("path → snippet,
   *  null = fetched and none exists"). Kept so the same path never pays for
   *  extraction twice within a session; pruned when it grows past 200. */
  const fetchedSnippetsRef = useRef(new Map<string, string | null>()).current;

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

  /* ---------------- home data ---------------- */

  const loadHome = useCallback(async () => {
    try {
      setHome(await invoke<HomeData>("get_home_data"));
    } catch (err) {
      console.error("Home data error:", err);
    }
  }, []);

  useEffect(() => {
    if (isSetupComplete) loadHome();
  }, [isSetupComplete, loadHome, indexingStatus?.is_indexing]);

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

  /* ---------------- search (stale-while-revalidate) ---------------- */

  useEffect(() => {
    setFacetInclude(new Set());
    setFacetExclude(new Set());
    setSaveOpen(false);
  }, [query]);

  useEffect(() => {
    if (!isSetupComplete) return;

    const trimmed = query.trim();
    if (trimmed === "") {
      setResults([]);
      setActiveFilters([]);
      setHints([]);
      setInterpretation([]);
      setFacets(undefined);
      setSelectedIndex(0);
      setIsSearching(false);
      return;
    }

    if (query.startsWith("/")) {
      setResults([]);
      setActiveFilters([]);
      setHints([]);
      setInterpretation([]);
      setFacets(undefined);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(async () => {
      const seq = ++currentSeqRef.current;
      queryRef.current = query;
      try {
        const res = await invoke<SearchResponse>("search", { query, sort });
        if (seq >= currentSeqRef.current) {
          // Seed rows with snippets already fetched for these paths so a
          // repeated path never flashes snippet-less. Existing snippets are
          // never cleared mid-flight — old rows keep theirs until these
          // replace them.
          const seen = new Set<string>();
          const merged = res.results.map((r) => {
            seen.add(r.path);
            if (r.snippet == null && fetchedSnippetsRef.has(r.path)) {
              return { ...r, snippet: fetchedSnippetsRef.get(r.path) ?? null };
            }
            return r;
          });
          if (fetchedSnippetsRef.size > 200) {
            for (const p of fetchedSnippetsRef.keys()) {
              if (!seen.has(p)) fetchedSnippetsRef.delete(p);
            }
          }
          setResults(merged);
          setTotal(res.total ?? merged.length);
          setActiveFilters(res.active_filters);
          setHints(res.hints);
          setInterpretation(res.interpretation ?? []);
          setFacets(res.facets);
          setSelectedIndex(0);

          /* Phase 2 — the fast results are on screen; content snippets
             stream in afterwards and patch rows by path. Fails silently:
             rows simply keep their meta sub-lines. */
          const want: string[] = [];
          for (const r of merged) {
            if (r.snippet != null) continue;
            if (fetchedSnippetsRef.has(r.path)) continue;
            want.push(r.path);
            if (want.length >= 20) break;
          }
          if (want.length > 0) {
            invoke<SnippetDto[]>("get_snippets", { paths: want, query })
              .then((dtos) => {
                // Discard if a newer search superseded this one or the
                // query text changed while we were fetching.
                if (seq !== currentSeqRef.current || queryRef.current !== query) return;
                if (!Array.isArray(dtos)) return;
                let changed = false;
                const patch = new Set<string>();
                for (const dto of dtos) {
                  if (dto && typeof dto.path === "string") {
                    fetchedSnippetsRef.set(
                      dto.path,
                      typeof dto.snippet === "string" ? dto.snippet : null,
                    );
                    if (dto.snippet) patch.add(dto.path);
                  }
                }
                setResults((prev) => {
                  if (seq !== currentSeqRef.current || queryRef.current !== query) return prev;
                  const next = prev.map((r) => {
                    if (r.snippet != null || !patch.has(r.path)) return r;
                    changed = true;
                    return { ...r, snippet: fetchedSnippetsRef.get(r.path) ?? null };
                  });
                  // Avoid an identical state swap when nothing matched.
                  return changed ? next : prev;
                });
              })
              .catch(() => {
                /* snippets are optional decoration — never surface errors */
              });
          }
        }
      } catch (err) {
        console.error("Search error:", err);
      } finally {
        setIsSearching(false);
      }
    }, 40);

    return () => clearTimeout(timer);
  }, [query, sort, isSetupComplete]);

  const loadMore = useCallback(async () => {
    if (loadingMore || !query.trim()) return;
    setLoadingMore(true);
    try {
      const res = await invoke<SearchResponse>("search", {
        query,
        sort,
        offset: results.length,
      });
      setResults((prev) => {
        const seen = new Set(prev.map((r) => r.path));
        return [...prev, ...res.results.filter((r) => !seen.has(r.path))];
      });
      setTotal(res.total ?? 0);
    } catch (err) {
      console.error("Load more error:", err);
    } finally {
      setLoadingMore(false);
    }
  }, [query, sort, results.length, loadingMore]);

  // Results never reorder while typing — sort is backend-provided and stable;
  // nothing needed here beyond identity-stable render keys.

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
      } else if (out.message === "DUPLICATES") {
        setShowDuplicates(true);
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

  /* Category facet: click includes, Alt-click excludes, All clears. */
  const handleFacetToggle = useCallback((id: string, alt: boolean) => {
    if (id === "all") {
      setFacetInclude(new Set());
      setFacetExclude(new Set());
      return;
    }
    setFacetInclude((prev) => {
      const next = new Set(prev);
      if (alt) {
        next.delete(id);
        setFacetExclude((ex) => {
          const e = new Set(ex);
          e.has(id) ? e.delete(id) : e.add(id);
          return e;
        });
      } else {
        next.has(id) ? next.delete(id) : next.add(id);
        setFacetExclude((ex) => {
          const e = new Set(ex);
          e.delete(id);
          return e;
        });
      }
      return next;
    });
  }, []);

  const openSettings = useCallback((section?: string) => {
    setSettingsSection(section);
    setShowSettings(true);
  }, []);

  const handleCopyPath = useCallback((path: string) => {
    navigator.clipboard.writeText(path);
  }, []);

  const selected: SearchResult | null = results[selectedIndex] ?? null;

  /* ---------------- `/` commands ---------------- */

  const slashCommands = useMemo((): SlashCommand[] => {
    const focus = (q: string) => {
      setQuery(q);
      inputRef.current?.focus();
    };
    return [
      { id: "images", name: "images", label: "type:image", description: "Screenshots and photos", icon: ImageIcon, run: () => focus("type:image ") },
      { id: "pdf", name: "pdf", label: "ext:pdf", description: "PDF documents", icon: FileText, run: () => focus("ext:pdf ") },
      { id: "docs", name: "docs", label: "type:doc", description: "Documents and notes", icon: FileText, run: () => focus("type:doc ") },
      { id: "code", name: "code", label: "type:code", description: "Source and config files", icon: FileCode, run: () => focus("type:code ") },
      { id: "media", name: "media", label: "type:media", description: "Audio and video", icon: Film, run: () => focus("type:media ") },
      { id: "recent", name: "recent", label: "modified:<7d", description: "Changed in the last week", icon: Clock, run: () => focus("modified:<7d ") },
      { id: "errors", name: "errors", label: "error", description: "Search for error text", icon: TriangleAlert, run: () => focus("error ") },
      { id: "diagrams", name: "diagrams", label: "diagram", description: "Search for diagrams", icon: FileCode, run: () => focus("diagram ") },
      { id: "dupes", name: "dupes", label: "review", description: "Find duplicate files and images", icon: Copy, run: () => { setQuery(""); setShowDuplicates(true); } },
      { id: "reindex", name: "reindex", label: "action", description: "Re-scan watched folders", icon: RefreshCw, run: () => { setQuery(""); handleRunCommand("/reindex"); } },
      { id: "doctor", name: "doctor", label: "action", description: "Index health report", icon: Stethoscope, run: () => { setQuery(""); handleRunCommand("/doctor"); } },
      { id: "settings", name: "settings", label: "⌘,", description: "Open Settings", icon: SettingsIcon, run: () => { setQuery(""); openSettings(); } },
    ];
  }, [handleRunCommand, openSettings]);

  const slashFilter = query.startsWith("/") ? query.slice(1).trim().toLowerCase() : null;
  const slashItems = useMemo(() => {
    if (slashFilter === null) return [];
    return slashCommands.filter(
      (c) => slashFilter === "" || c.name.includes(slashFilter) || c.description.toLowerCase().includes(slashFilter),
    );
  }, [slashCommands, slashFilter]);

  useEffect(() => {
    setSlashIndex(0);
  }, [slashFilter]);

  /* ---------------- `@` scopes ---------------- */

  const scopePrefix = useMemo(() => {
    if (query.startsWith("/")) return null;
    const m = query.match(/(?:^|\s)@([^\s]*)$/);
    return m ? m[1] : null;
  }, [query]);

  useEffect(() => {
    if (scopePrefix === null) {
      setScopeItems([]);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      invoke<string[]>("get_scope_suggestions", { prefix: scopePrefix, limit: 12 })
        .then((items) => {
          if (!cancelled) {
            setScopeItems(items);
            setScopeIndex(0);
          }
        })
        .catch(() => {
          if (!cancelled) setScopeItems([]);
        });
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [scopePrefix]);

  const applyScope = useCallback((path: string) => {
    setQuery((prev) => prev.replace(/@[^\s]*$/, `in:"${path}" `));
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent) => {
      // Overlays capture their own Esc; everything else pauses the main list.
      if (showSettings) return;
      if (showDuplicates) return;

      if (e.key === "Tab") {
        // Tab toggles the preview pane, except when focus is on a real
        // control (theme/settings buttons) — then native focus cycling runs
        // so chrome stays reachable by keyboard. The / menu covers commands and filters.
        const t = e.target;
        if (t instanceof HTMLElement && t.closest("button")) return;
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

      if (e.key === "," && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        openSettings();
        return;
      }

      // `/` command menu
      if (query.startsWith("/")) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          setSlashIndex((i) => (slashItems.length ? (i + 1) % slashItems.length : 0));
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          setSlashIndex((i) =>
            slashItems.length ? (i - 1 + slashItems.length) % slashItems.length : 0,
          );
        } else if (e.key === "Enter") {
          e.preventDefault();
          const item = slashItems[slashIndex] ?? slashItems[0];
          if (item) item.run();
        } else if (e.key === "Escape") {
          e.preventDefault();
          setQuery("");
        }
        return;
      }

      // `@` scope menu
      if (scopePrefix !== null && scopeItems.length > 0) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          setScopeIndex((i) => (i + 1) % scopeItems.length);
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          setScopeIndex((i) => (i - 1 + scopeItems.length) % scopeItems.length);
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          applyScope(scopeItems[scopeIndex] ?? scopeItems[0]);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          setQuery((prev) => prev.replace(/@[^\s]*$/, "").trimEnd());
          return;
        }
      }

      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        if (navList.length === 0) {
          setSelectedIndex(0);
          return;
        }
        // Grid view navigates by rows first: Up/Down jump a whole row,
        // Left/Right step within the row. List view stays linear with wrap.
        const grid = view === "grid";
        if (grid) {
          const cols = gridColumnCount();
          if (e.key === "ArrowLeft") {
            setSelectedIndex((prev) => Math.max(0, prev - 1));
          } else if (e.key === "ArrowRight") {
            setSelectedIndex((prev) => Math.min(navList.length - 1, prev + 1));
          } else if (e.key === "ArrowDown") {
            setSelectedIndex((prev) => Math.min(navList.length - 1, prev + cols));
          } else {
            setSelectedIndex((prev) => Math.max(0, prev - cols));
          }
        } else {
          if (e.key === "ArrowDown") {
            setSelectedIndex((prev) => (prev + 1) % navList.length);
          } else if (e.key === "ArrowUp") {
            setSelectedIndex((prev) => (prev - 1 + navList.length) % navList.length);
          }
        }
      } else if (e.key === "Enter" && navList[selectedIndex]) {
        e.preventDefault();
        const target = navList[selectedIndex];
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
    [showSettings, showDuplicates, query, results, selectedIndex, commandOutput, handleRunCommand, handleOpenFile, handleOpenFolder, openSettings, view, gridColumnCount, slashItems, slashIndex, scopePrefix, scopeItems, scopeIndex, applyScope],
  );

  /* ---------------- render ---------------- */

  const displayResults = useMemo(
    () =>
      results.filter((r) => {
        const cat = classify(r);
        if (facetExclude.has(cat)) return false;
        if (facetInclude.size > 0 && !facetInclude.has(cat)) return false;
        return true;
      }),
    [results, facetInclude, facetExclude],
  );
  if (isSetupComplete === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg">
        <span className="h-4 w-4 animate-pulse rounded-full bg-accent" />
      </div>
    );
  }

  const isCommandMode = query.startsWith("/");

  const hasQuery = query.trim() !== "";
  const noResults = hasQuery && !isCommandMode && displayResults.length === 0;
  const isIndexing = indexingStatus?.is_indexing ?? false;
  // The main page shows files only: preview pane appears when a query has results.
  const showPreview = !isCommandMode && previewOpen && hasQuery;
  // The list keyboard navigation operates over — search results or recent files.
  const navList = displayResults;

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
          {showDuplicates && <DuplicatesModal onClose={() => setShowDuplicates(false)} />}

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
                onOpenCommands={() => {
                  setSlashIndex(0);
                  setQuery("/");
                  inputRef.current?.focus();
                }}
                onFocusInput={() => {}}
                resultCount={results.length}
                trailing={
                  <>
                    <button
                      type="button"
                      title={
                        theme === "paper" ? "Switch to void theme" : "Switch to paper theme"
                      }
                      onClick={toggleTheme}
                      className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
                    >
                      {theme === "paper" ? (
                        <Moon className="h-3.5 w-3.5" />
                      ) : (
                        <Sun className="h-3.5 w-3.5" />
                      )}
                    </button>
                    <button
                      type="button"
                      title="Settings (⌘,)"
                      onClick={() => openSettings()}
                      className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
                    >
                      <SettingsIcon className="h-3.5 w-3.5" />
                    </button>
                  </>
                }
              />

              {slashFilter !== null && (
                <SlashMenu items={slashItems} activeIndex={slashIndex} onPick={(c) => c.run()} />
              )}
              {scopePrefix !== null && (
                <ScopeMenu items={scopeItems} activeIndex={scopeIndex} onPick={applyScope} />
              )}

              {/* Global indexing progress: 2px accent line under the bar —
                  pure feedback, no numbers. Stale results stay visible. */}
              {isIndexing && (
                <div className="progress-track shrink-0 px-0">
                  <div className="progress-bar" />
                </div>
              )}

              {/* Category chips */}
              {!isCommandMode && (
                <CategoryChips
                  results={results}
                  facets={facets}
                  selected={facetInclude}
                  excluded={facetExclude}
                  onToggle={handleFacetToggle}
                />
              )}

              {!isCommandMode && interpretation.length > 0 && (
                <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-line px-4 py-1.5">
                  <span className="text-[10px] uppercase tracking-wider text-subtle">Interpreted</span>
                  {interpretation.map((c, i) => (
                    <button
                      key={`${c.raw}-${i}`}
                      type="button"
                      title={`Remove “${c.source}”`}
                      onClick={() =>
                        setQuery((prev) =>
                          prev.replace(c.source, "").replace(/\s+/g, " ").trim(),
                        )
                      }
                      className="group flex h-6 items-center gap-1.5 rounded-full bg-accent-soft px-2.5 text-[10px] text-fg transition-colors hover:text-danger"
                    >
                      {c.label}
                      <X className="h-2.5 w-2.5 opacity-50 group-hover:opacity-100" />
                    </button>
                  ))}
                </div>
              )}

              {!isCommandMode && displayResults.length > 0 && (
                <div className="flex shrink-0 items-center justify-end gap-2 border-b border-line px-4 py-1.5">
                  <label htmlFor="sort" className="text-[10px] uppercase tracking-wider text-subtle">
                    Sort
                  </label>
                  <select
                    id="sort"
                    value={sort}
                    onChange={(e) => setSort(e.target.value)}
                    className="rounded-[8px] border border-line bg-elevated px-2 py-1 text-[11px] text-fg outline-none focus:border-accent"
                  >
                    <option value="best">Best match</option>
                    <option value="newest">Newest</option>
                    <option value="oldest">Oldest</option>
                    <option value="largest">Largest</option>
                    <option value="name">Name</option>
                  </select>
                  <button
                    type="button"
                    title="Save this search"
                    onClick={() => setSaveOpen((v) => !v)}
                    className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-sunken hover:text-fg"
                  >
                    <Bookmark className="h-3.5 w-3.5" />
                  </button>
                </div>
              )}

              {saveOpen && (
                <div className="flex shrink-0 items-center gap-2 border-b border-line px-4 py-1.5">
                  <input
                    autoFocus
                    value={saveName}
                    placeholder="Name this search"
                    onChange={(e) => setSaveName(e.target.value)}
                    onKeyDown={async (e) => {
                      if (e.key !== "Enter") return;
                      const name = saveName.trim();
                      if (!name) return;
                      try {
                        const cfg = await invoke<KensakuConfig>("get_config");
                        cfg.saved_searches = [
                          ...(cfg.saved_searches ?? []),
                          { name, query, sort },
                        ];
                        await invoke("save_config", { config: cfg });
                        setSaveName("");
                        setSaveOpen(false);
                        loadHome();
                      } catch (err) {
                        console.error(err);
                      }
                    }}
                    className="h-7 min-w-0 flex-1 rounded-[8px] border border-line bg-elevated px-2 text-[11px] text-fg outline-none focus:border-accent"
                  />
                  <button
                    type="button"
                    onClick={async () => {
                      const name = saveName.trim();
                      if (!name) return;
                      try {
                        const cfg = await invoke<KensakuConfig>("get_config");
                        cfg.saved_searches = [
                          ...(cfg.saved_searches ?? []),
                          { name, query, sort },
                        ];
                        await invoke("save_config", { config: cfg });
                        setSaveName("");
                        setSaveOpen(false);
                        loadHome();
                      } catch (err) {
                        console.error(err);
                      }
                    }}
                    className="h-7 rounded-[8px] bg-accent px-3 text-[11px] font-medium text-accent-fg"
                  >
                    Save
                  </button>
                </div>
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
                      ? "grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] content-start gap-3 p-4"
                      : "px-2 py-1"
                  }`}
                >
                  {displayResults.length > 0 ? (
                    view === "list" ? (
                      displayResults.map((res, idx) => (
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
                      displayResults.map((res, idx) => (
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
                    <HomeView
                      data={home}
                      onOpen={(path) => handleOpenFile(path)}
                      onTogglePin={async (path) => {
                        try {
                          await invoke("toggle_pin", { path });
                          await loadHome();
                        } catch (e) {
                          console.error(e);
                        }
                      }}
                      onSearch={(q, s) => {
                        if (s) setSort(s);
                        setQuery(q);
                        inputRef.current?.focus();
                      }}
                    />
                  )}

                  {!isCommandMode && displayResults.length > 0 && results.length < total && (
                    <div className="flex justify-center p-3">
                      <button
                        type="button"
                        onClick={loadMore}
                        disabled={loadingMore}
                        className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line px-4 text-[12px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                      >
                        {loadingMore ? "Loading…" : `Load more (${results.length} of ${total})`}
                      </button>
                    </div>
                  )}
                </div>

                {/* Preview pane, hidden < 900px */}
                {showPreview && (
                  <PreviewPane
                    result={selected}
                    searchQuery={query}
                    onOpen={() => selected && handleOpenFile(selected.path)}
                    onOpenFolder={() => selected && handleOpenFolder(selected.path)}
                  />
                )}
              </div>

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

export default App;
