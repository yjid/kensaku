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
  LlmCompileOutcome,
} from "./types";
import { SetupModal } from "./components/SetupModal";
import { DuplicatesModal } from "./components/DuplicatesModal";
import { AskPanel } from "./components/AskPanel";
import { SlashMenu, SlashCommand } from "./components/SlashMenu";
import { ScopeMenu } from "./components/ScopeMenu";
import { HomeView } from "./components/HomeView";
import { ResultItem, ResultView } from "./components/ResultItem";
import { Settings } from "./components/SettingsModal";
import { FilterChips } from "./components/FilterChips";
import { ContextMenu, ContextMenuItem } from "./components/ContextMenu";
import { DetailsDialog } from "./components/DetailsDialog";
import { SearchBar } from "./components/SearchBar";
import { CategoryChips } from "./components/CategoryChips";
import { ResultControls } from "./components/ResultControls";
import { VirtualGrid } from "./components/VirtualGrid";
import { FileRow } from "./components/FileRow";
import { PreviewDialog } from "./components/PreviewDialog";
import { classify } from "./lib/categories";
import {
  Settings as SettingsIcon,
  RefreshCw,
  Copy,
  FileText,
  MessageSquare,
  Image as ImageIcon,
  FileCode,
  Film,
  Clock,
  Terminal,
  Stethoscope,
  TriangleAlert,
  X,
} from "lucide-react";
import "./App.css";

const SNIPPET_LRU_LIMIT = 200;

/** Small insertion-order LRU for snippets: recent hits move to the end. */
function putSnippet(
  cache: Map<string, string | null>,
  key: string,
  value: string | null,
) {
  cache.delete(key);
  cache.set(key, value);
  while (cache.size > SNIPPET_LRU_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest === undefined) break;
    cache.delete(oldest);
  }
}

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
  const [showAsk, setShowAsk] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const [scopeItems, setScopeItems] = useState<string[]>([]);
  const [scopeIndex, setScopeIndex] = useState(0);
  const [commandOutput, setCommandOutput] = useState<string | null>(null);
  const [sort, setSort] = useState("best");
  /* Right-click menu and the details dialog replace the old preview pane. */
  // Thumbnail setting, read from config. Reloaded whenever Settings closes.
  const [config, setConfig] = useState<KensakuConfig | null>(null);
  useEffect(() => {
    invoke<KensakuConfig>("get_config")
      .then(setConfig)
      .catch(() => undefined);
  }, [showSettings]);
  const showThumbs = config?.show_thumbnails !== false;
  const view: ResultView = config?.results_view === "list" ? "list" : "grid";
  const [menu, setMenu] = useState<{ x: number; y: number; result: SearchResult } | null>(null);
  const [details, setDetails] = useState<SearchResult | null>(null);
  const [preview, setPreview] = useState<SearchResult | null>(null);


  const inputRef = useRef<HTMLInputElement>(null);
  const resultsContainerRef = useRef<HTMLDivElement>(null);

  /** Number of cards in the first grid row (1 in list view). Measured from
   *  the DOM so it stays correct across window resizes and auto-fill.
   *  Only `data-index` cells count — full-span labels ("Recent") sit in
   *  the grid too but span every column. */
  const columnsRef = useRef(1);
  const gridColumnCount = useCallback(() => {
    if (columnsRef.current > 0) return columnsRef.current;
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
  /** Cache keyed by path + mtime + query so a changed file or query refetches. */
  const snippetCacheRef = useRef(new Map<string, string | null>()).current;

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

  /**
   * Optional LLM refinement (Settings → Agents → Local AI). Deterministic
   * results are already on screen; when the local model produces a *valid
   * compiled* refinement that differs from the raw text, its results replace
   * them and its chips are shown. Best-effort: any failure keeps the
   * deterministic results.
   */
  const refineWithLlm = useCallback(async (seq: number, raw: string, currentSort: string) => {
    const trimmed = raw.trim();
    if (
      !trimmed ||
      trimmed.startsWith("/") ||
      trimmed.split(/\s+/).some((t) => t.includes(":") && !t.startsWith("http"))
    ) {
      return;
    }
    try {
      const outcome = await invoke<LlmCompileOutcome>("llm_compile_query", { query: raw });
      if (seq < currentSeqRef.current || queryRef.current !== raw) return;
      if (outcome.status !== "compiled" || !outcome.compiled) return;
      const compiled = outcome.compiled;
      if (compiled.query.trim() === trimmed) {
        if (compiled.chips.length > 0) setInterpretation(compiled.chips);
        return;
      }
      const res = await invoke<SearchResponse>("search", {
        query: compiled.query,
        sort: compiled.sort ?? currentSort,
        seq,
      });
      if (seq >= currentSeqRef.current && queryRef.current === raw) {
        setResults(res.results);
        setTotal(res.total ?? res.results.length);
        setActiveFilters(res.active_filters);
        setHints(res.hints);
        setInterpretation(compiled.chips.length > 0 ? compiled.chips : (res.interpretation ?? []));
        setFacets(res.facets);
        setSelectedIndex(0);
      }
    } catch {
      // Refinement is best-effort; deterministic results stay.
    }
  }, []);

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
            const cacheKey = `${r.path}\u0000${r.modified}\u0000${query}`;
            if (r.snippet == null && snippetCacheRef.has(cacheKey)) {
              const cached = snippetCacheRef.get(cacheKey) ?? null;
              putSnippet(snippetCacheRef, cacheKey, cached);
              fetchedSnippetsRef.set(r.path, cached);
              return { ...r, snippet: cached };
            }
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

          // Snippets are fetched by the visibility observer below (or the
          // no-IntersectionObserver fallback) so off-screen rows never pay
          // for content reads.
        }
        // Best-effort LLM refinement runs after deterministic results land.
        void refineWithLlm(seq, query, sort);
      } catch (err) {
        console.error("Search error:", err);
      } finally {
        setIsSearching(false);
      }
    }, 40);

    return () => clearTimeout(timer);
  }, [query, sort, isSetupComplete, refineWithLlm]);

  // Fetch snippets only for rows that are actually visible (or the first 20
  // when IntersectionObserver is unavailable). Results are patched by path
  // and stale responses for superseded queries are discarded.
  useEffect(() => {
    if (!isSetupComplete || results.length === 0) return;
    const container = resultsContainerRef.current;
    if (!container) return;

    const seq = currentSeqRef.current;
    const q = query;
    const pending = new Set<string>();

    const flush = () => {
      const paths = Array.from(pending).slice(0, 20);
      pending.clear();
      if (paths.length === 0) return;
      invoke<SnippetDto[]>("get_snippets", { paths, query: q })
        .then((dtos) => {
          if (seq !== currentSeqRef.current || queryRef.current !== q) return;
          if (!Array.isArray(dtos)) return;
          const patch = new Set<string>();
          for (const dto of dtos) {
            if (dto && typeof dto.path === "string") {
              const result = results.find((r) => r.path === dto.path);
              const key = `${dto.path}\u0000${result?.modified ?? 0}\u0000${q}`;
              putSnippet(
                snippetCacheRef,
                key,
                typeof dto.snippet === "string" ? dto.snippet : null,
              );
              fetchedSnippetsRef.set(
                dto.path,
                typeof dto.snippet === "string" ? dto.snippet : null,
              );
              if (dto.snippet) patch.add(dto.path);
            }
          }
          setResults((prev) => {
            if (seq !== currentSeqRef.current || queryRef.current !== q) return prev;
            let changed = false;
            const next = prev.map((r) => {
              if (r.snippet != null || !patch.has(r.path)) return r;
              changed = true;
              return { ...r, snippet: fetchedSnippetsRef.get(r.path) ?? null };
            });
            return changed ? next : prev;
          });
        })
        .catch(() => {
          /* snippets are optional decoration */
        });
    };

    if (typeof IntersectionObserver === "undefined") {
      const want: string[] = [];
      for (const r of results) {
        if (r.snippet != null) continue;
        const key = `${r.path}\u0000${r.modified}\u0000${q}`;
        if (snippetCacheRef.has(key)) continue;
        want.push(r.path);
        if (want.length >= 20) break;
      }
      if (want.length > 0) {
        for (const path of want) pending.add(path);
        flush();
      }
      return;
    }

    let timer: ReturnType<typeof setTimeout> | null = null;
    const schedule = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(flush, 80);
    };

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const index = Number((entry.target as HTMLElement).getAttribute("data-index"));
          const result = results[index];
          if (!result || result.snippet != null) continue;
          const key = `${result.path}\u0000${result.modified}\u0000${q}`;
          if (snippetCacheRef.has(key)) continue;
          if (pending.has(result.path)) continue;
          pending.add(result.path);
        }
        if (pending.size > 0) schedule();
      },
      { root: container, threshold: 0.01 },
    );

    container.querySelectorAll("[data-index]").forEach((el) => observer.observe(el));

    return () => {
      observer.disconnect();
      if (timer) clearTimeout(timer);
    };
  }, [results, query, isSetupComplete]);

  /* Infinite scroll guard. A page that adds nothing new marks this query as
     exhausted, so the grid stops asking. In-flight requests are ref-guarded,
     so repeated near-end signals can't stack up requests. */
  const pageKey = `${query}\u0000${sort}`;
  const [exhaustedKey, setExhaustedKey] = useState<string | null>(null);
  const loadingRef = useRef(false);
  const resultsLenRef = useRef(0);
  resultsLenRef.current = results.length;

  const loadMore = useCallback(async () => {
    if (loadingRef.current || !query.trim() || exhaustedKey === pageKey) return;
    loadingRef.current = true;
    setLoadingMore(true);
    const requestKey = pageKey;
    try {
      const offset = resultsLenRef.current;
      const res = await invoke<SearchResponse>("search", { query, sort, offset });
      // Discard a page for a query the user has already left.
      if (queryRef.current !== query) return;
      let added = 0;
      setResults((prev) => {
        const seen = new Set(prev.map((r) => r.path));
        const fresh = res.results.filter((r) => !seen.has(r.path));
        added = fresh.length;
        return fresh.length ? [...prev, ...fresh] : prev;
      });
      setTotal(res.total ?? 0);
      if (added === 0 || res.results.length === 0) setExhaustedKey(requestKey);
    } catch (err) {
      console.error("Load more error:", err);
      setExhaustedKey(requestKey);
    } finally {
      loadingRef.current = false;
      setLoadingMore(false);
    }
  }, [query, sort, pageKey, exhaustedKey]);

  // Results never reorder while typing — sort is backend-provided and stable;
  // nothing needed here beyond identity-stable render keys.

  /* ---------------- focus + persistence ---------------- */

  useEffect(() => {
    if (isSetupComplete) inputRef.current?.focus();
  }, [isSetupComplete]);


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
      { id: "ask", name: "ask", label: "answers", description: "Ask your files, with sources", icon: MessageSquare, run: () => { setQuery(""); setShowAsk(true); } },
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
      } else if (e.key === " " && !preview && navList[selectedIndex] && document.activeElement !== inputRef.current) {
        e.preventDefault();
        setPreview(navList[selectedIndex]);
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
        if (showAsk) {
          setShowAsk(false);
        } else if (commandOutput) {
          setCommandOutput(null);
        } else if (query) {
          setQuery("");
          inputRef.current?.focus();
        } else {
          await invoke("hide_window");
        }
      }
    },
    [showSettings, showDuplicates, showAsk, query, results, selectedIndex, commandOutput, handleRunCommand, handleOpenFile, handleOpenFolder, openSettings, view, gridColumnCount, slashItems, slashIndex, scopePrefix, scopeItems, scopeIndex, applyScope, preview],
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
          {menu && (
            <ContextMenu
              x={menu.x}
              y={menu.y}
              onClose={() => setMenu(null)}
              items={[
                { label: "Open", onSelect: () => handleOpenFile(menu.result.path) },
                { label: "Preview", onSelect: () => setPreview(menu.result) },
                { label: "Show in folder", onSelect: () => handleOpenFolder(menu.result.path) },
                { label: "Copy path", onSelect: () => handleCopyPath(menu.result.path) },
                { label: "Details", onSelect: () => setDetails(menu.result), separatorBefore: true },
              ] as ContextMenuItem[]}
            />
          )}
          {details && <DetailsDialog result={details} onClose={() => setDetails(null)} />}
          {preview && (
            <PreviewDialog
              result={preview}
              onOpen={() => handleOpenFile(preview.path)}
              onClose={() => setPreview(null)}
            />
          )}
          <AskPanel
            open={showAsk}
            onClose={() => setShowAsk(false)}
            onOpenFile={(path) => void handleOpenFile(path)}
          />

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
                onFocusInput={() => {}}
                resultCount={results.length}
                trailing={
                  <button
                    type="button"
                    title="Settings"
                    onClick={() => openSettings()}
                    className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
                  >
                    <SettingsIcon className="h-4 w-4" />
                  </button>
                }
              />

              {slashFilter !== null && (
                <SlashMenu items={slashItems} activeIndex={slashIndex} onPick={(c) => c.run()} />
              )}
              {scopePrefix !== null && (
                <ScopeMenu items={scopeItems} activeIndex={scopeIndex} onPick={applyScope} />
              )}


              {!isCommandMode && interpretation.length > 0 && (
                <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-line px-4 py-1.5">
                  <span className="text-[11px] text-muted">Interpreted</span>
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
                      className="group flex h-6 items-center gap-1.5 rounded-full bg-accent-soft px-2.5 text-[11px] text-fg transition-colors hover:text-danger"
                    >
                      {c.label}
                      <X className="h-2.5 w-2.5 opacity-50 group-hover:opacity-100" />
                    </button>
                  ))}
                </div>
              )}

              {/* One control bar: category chips on the left, layout and save on the right. */}
              {!isCommandMode && (
                <div className="flex shrink-0 items-center gap-2 border-b border-line px-4 py-2">
                  <CategoryChips
                    results={results}
                    facets={facets}
                    selected={facetInclude}
                    excluded={facetExclude}
                    onToggle={handleFacetToggle}
                  />
                  <ResultControls
                    sort={sort}
                    onSort={setSort}
                    view={view}
                    onView={(v) => {
                      setConfig((c) => (c ? { ...c, results_view: v } : c));
                      invoke<KensakuConfig>("get_config")
                        .then((cfg) => invoke("save_config", { config: { ...cfg, results_view: v } }))
                        .catch(() => undefined);
                    }}
                    canSave={displayResults.length > 0}
                    onSave={() => setSaveOpen((v) => !v)}
                  />
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
                    <pre className="select-text whitespace-pre-wrap break-all font-mono text-[11px] leading-relaxed text-muted">
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
                  className={`flex min-h-0 min-w-0 flex-1 flex-col ${
                    !hasQuery || displayResults.length === 0 ? "overflow-y-auto" : ""
                  }`}
                >
                  {displayResults.length > 0 ? (
                    <VirtualGrid
                      items={displayResults}
                      getKey={(res) => res.path}
                      cardHeight={(w) => w * 0.75 + 56}
                      list={view === "list"}
                      selectedIndex={selectedIndex}
                      hasMore={results.length < total && exhaustedKey !== pageKey}
                      loading={loadingMore}
                      onNearEnd={loadMore}
                      onColumns={(cols) => {
                        columnsRef.current = cols;
                      }}
                      renderItem={(res, idx) =>
                        view === "list" ? (
                          <FileRow
                            item={{ path: res.path, filename: res.filename, file_size: res.file_size, modified: res.modified, is_dir: res.is_dir }}
                            query={query}
                            selected={idx === selectedIndex}
                            showThumbs={showThumbs}
                            onSelect={() => setSelectedIndex(idx)}
                            onOpen={() => handleOpenFile(res.path)}
                            onContextMenu={(x, y) => setMenu({ x, y, result: res })}
                          />
                        ) : (
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
                            onContextMenu={(x, y) => setMenu({ x, y, result: res })}
                            showThumbs={showThumbs}
                          />
                        )
                      }
                    />
                  ) : noResults ? (
                    <div className="flex h-full flex-col items-center justify-center gap-1 p-8 text-center select-none">
                      <p className="font-serif text-[17px] italic text-muted">
                        {`Nothing matches ${query.trim()}`}
                      </p>
                      <div className="flex flex-wrap items-center justify-center gap-1.5 mt-3">
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
                    </div>
                  ) : (
                    <HomeView
                      data={home}
                      onOpen={(path) => handleOpenFile(path)}
                      showThumbs={showThumbs}
                      onSearch={(q) => {
                        setQuery(q);
                        inputRef.current?.focus();
                      }}
                    />
                  )}

                </div>
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
      className="flex h-6 items-center rounded-full border border-line px-2.5 font-mono text-[11px] text-muted transition-colors duration-150 hover:border-line-strong hover:text-fg"
    >
      {label}
    </button>
  );
}

function hasActiveFilters(q: string): boolean {
  return /\b(type|ext|in|modified|size):/i.test(q);
}

export default App;
