import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  Bot,
  Check,
  Copy,
  Filter,
  FolderTree,
  Info,
  Plus,
  RefreshCw,
  Save,
  Search,
  Sparkles,
  SlidersHorizontal,
  Stethoscope,
  Sun,
  Moon,
  X,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { KensakuConfig } from "../types";
import { useTheme } from "../hooks/useTheme";
import { useStats } from "../hooks/useStats";
import { Card, Field, Switch, Stepper, SectionHeader, reasonFrom } from "./settings/kit";
import { OcrField } from "./settings/ocr";
import { StatsForNerds } from "./settings/stats";
import { DoctorSection } from "./settings/doctor";
import { ModelFilesCard } from "./settings/models";
import { formatBytes, formatComma } from "../lib/format";

export interface SettingsProps {
  onClose: () => void;
  initialSection?: string;
  onConfigSaved: () => void;
}

type SectionId =
  | "general"
  | "folders"
  | "exclusions"
  | "search"
  | "ai"
  | "agents"
  | "stats"
  | "doctor"
  | "about";

const SECTIONS: { id: SectionId; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "General", icon: SlidersHorizontal },
  { id: "folders", label: "Folders", icon: FolderTree },
  { id: "exclusions", label: "Exclusions", icon: Filter },
  { id: "search", label: "Search", icon: Search },
  { id: "ai", label: "AI & Semantic", icon: Sparkles },
  { id: "agents", label: "Agents", icon: Bot },
  { id: "stats", label: "Stats for Nerds", icon: Activity },
  { id: "doctor", label: "Doctor", icon: Stethoscope },
  { id: "about", label: "About", icon: Info },
];

const SECTION_META: Record<SectionId, { title: string; description: string }> = {
  general: { title: "General", description: "Appearance, invocation and startup behaviour." },
  folders: { title: "Folders", description: "Directories watched and indexed on this machine." },
  exclusions: { title: "Exclusions", description: "Names and extensions kept out of the index." },
  search: { title: "Search", description: "Ranking, OCR and the query language." },
  ai: { title: "AI & Semantic", description: "Embedding model, vector tier and model files." },
  agents: { title: "Agents", description: "Read-only access for MCP clients and the CLI." },
  stats: { title: "Stats for Nerds", description: "Live telemetry from the running index." },
  doctor: { title: "Doctor", description: "Health checks across index, watcher and models." },
  about: { title: "About", description: "Version, thesis and the reset control." },
};

const EXCLUSION_PRESETS: Record<string, string[]> = {
  Developer: [
    "node_modules", ".git", "target", "dist", "build", "out",
    ".next", ".nuxt", ".output", ".cache", ".venv", "venv",
    "__pycache__", "vendor", ".gradle", ".idea", ".vscode",
  ],
  Creative: [
    "node_modules", ".git", ".Trash", ".cache", "Backups",
    "Time Machine", "Library", "lost+found", ".dub", "_archive",
  ],
  "All-round": [
    "node_modules", ".git", "target", "dist", "build", "out",
    ".cache", ".venv", "__pycache__", ".Trash", "Backups",
    "lost+found", ".idea", ".vscode", "Library",
  ],
};

/* Full overlay settings page: 200px rail + bento sections.
   Config edits are local until Save; theme applies immediately. */

export function Settings({ onClose, initialSection, onConfigSaved }: SettingsProps) {
  const [section, setSection] = useState<SectionId>(
    (SECTIONS.find((s) => s.id === initialSection)?.id ?? "general") as SectionId,
  );
  const [config, setConfig] = useState<KensakuConfig | null>(null);
  /* Loaded snapshot — serialized once. A JSON compare marks the save bar. */
  const [snapshot, setSnapshot] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const dirty = config !== null && snapshot !== null && JSON.stringify(config) !== snapshot;

  useEffect(() => {
    invoke<KensakuConfig>("get_config")
      .then((cfg) => {
        setConfig(cfg);
        setSnapshot(JSON.stringify(cfg));
      })
      .catch((err) => console.error("Failed to load config:", err));
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleSave = async (reindex: boolean) => {
    if (!config) return;
    setSaving(true);
    try {
      await invoke("save_config", { config });
      setSnapshot(JSON.stringify(config));
      if (reindex) {
        await invoke("complete_setup", {
          watchedDirs: config.watched_dirs,
          excludedDirs: config.excluded_dirs,
          shortcut: config.shortcut,
        });
      }
      onConfigSaved();
      if (reindex) {
        onClose();
      } else {
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1600);
      }
    } catch (err) {
      console.error("Save failed:", err);
      alert("Failed to save settings: " + err);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fade-in absolute inset-0 z-40 flex flex-col bg-bg">
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-line px-4">
        <h1 className="text-[14px] font-semibold text-fg">Settings</h1>
        <button
          type="button"
          title="Close (Esc)"
          onClick={onClose}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors duration-150 hover:bg-sunken hover:text-fg"
        >
          <X className="h-4 w-4" />
        </button>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* Rail: 200px, 32px icon rows, accent-soft active with 2px left bar. */}
        <nav className="flex w-[200px] shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-line bg-sunken/50 p-3">
          {SECTIONS.map((s) => {
            const active = section === s.id;
            const Icon = s.icon;
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => setSection(s.id)}
                aria-current={active ? "page" : undefined}
                className={`relative flex h-8 w-full items-center gap-2 rounded-[10px] px-2.5 text-[13px] transition-colors duration-150 ${
                  active
                    ? "bg-accent-soft font-medium text-fg"
                    : "text-muted hover:bg-elevated hover:text-fg"
                }`}
              >
                {active && (
                  <span
                    aria-hidden="true"
                    className="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-accent"
                  />
                )}
                <Icon
                  className={`h-[15px] w-[15px] shrink-0 transition-colors duration-150 ${
                    active ? "text-fg" : "text-muted"
                  }`}
                />
                {s.label}
              </button>
            );
          })}
        </nav>

        {/* Content column: scroll area + sticky save bar that never
            overlaps the last row (scroll area gets bottom padding). */}
        <main className="relative min-w-0 flex-1">
          <div className="absolute inset-0 overflow-y-auto bg-sunken/30 p-5 pb-[72px]">
            {config ? (
              <SectionShell section={section}>
                {renderSection(section, config, setConfig)}
              </SectionShell>
            ) : (
              <ConfigSkeleton />
            )}
          </div>

          <footer className="absolute inset-x-0 bottom-0 z-10 border-t border-line bg-elevated/80 px-5 py-3 backdrop-blur">
            <div className="flex items-center justify-between gap-3">
              {dirty ? (
                <span className="flex items-center gap-1.5 text-[11px] text-muted">
                  <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-warning" />
                  Unsaved changes
                </span>
              ) : (
                <span />
              )}
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => handleSave(true)}
                  disabled={saving || !config}
                  className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line bg-elevated px-3 text-[12px] text-fg transition-colors duration-150 hover:border-line-strong disabled:opacity-50"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                  Save &amp; Reindex
                </button>
                <button
                  type="button"
                  onClick={() => handleSave(false)}
                  disabled={saving || !config || !dirty}
                  className="flex h-8 items-center gap-1.5 rounded-[10px] bg-accent px-3.5 text-[12px] font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90 disabled:opacity-50"
                >
                  {saved ? <Check className="h-3.5 w-3.5" /> : <Save className="h-3.5 w-3.5" />}
                  {saved ? "Saved" : saving ? "Saving…" : "Save"}
                </button>
              </div>
            </div>
          </footer>
        </main>
      </div>
    </div>
  );
}

function renderSection(
  section: SectionId,
  config: KensakuConfig,
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>,
): React.ReactNode {
  switch (section) {
    case "general":
      return <GeneralSection config={config} setConfig={setConfig} />;
    case "folders":
      return <FoldersSection config={config} setConfig={setConfig} />;
    case "exclusions":
      return <ExclusionsSection config={config} setConfig={setConfig} />;
    case "search":
      return <SearchSection config={config} setConfig={setConfig} />;
    case "ai":
      return <AiSection config={config} />;
    case "agents":
      return <AgentsSection />;
    case "stats":
      return <StatsForNerds />;
    case "doctor":
      return <DoctorSection />;
    case "about":
      return <AboutSection />;
  }
}

function ConfigSkeleton() {
  return (
    <div className="space-y-3">
      {[0, 1, 2].map((i) => (
        <div key={i} className="h-16 animate-pulse rounded-[14px] border border-line bg-elevated opacity-60" />
      ))}
    </div>
  );
}

/** Wraps every section: header block first, then the cards. */
function SectionShell({ section, children }: { section: SectionId; children: React.ReactNode }) {
  const meta = SECTION_META[section];
  return (
    <div>
      <SectionHeader title={meta.title} description={meta.description} />
      {children}
    </div>
  );
}

/* ---------------------------------------------------------------- */
/* General                                                           */
/* ---------------------------------------------------------------- */

function GeneralSection({
  config,
  setConfig,
}: {
  config: KensakuConfig;
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>;
}) {
  const { theme, setTheme } = useTheme();
  const [capturing, setCapturing] = useState(false);

  /* null = unknown (backend rejected get_autostart) → switch stays off. */
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [autostartError, setAutostartError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("get_autostart")
      .then((v) => {
        if (!cancelled) setAutostart(v);
      })
      .catch(() => {
        if (!cancelled) setAutostart(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** Optimistic toggle: flip the switch immediately, revert on failure. */
  const toggleAutostart = async (next: boolean) => {
    if (autostart === null || autostartBusy) return;
    const prev = autostart;
    setAutostart(next);
    setAutostartError(null);
    setAutostartBusy(true);
    try {
      await invoke("set_autostart", { enabled: next });
    } catch (err) {
      setAutostart(prev);
      setAutostartError(reasonFrom(err));
    } finally {
      setAutostartBusy(false);
    }
  };

  useEffect(() => {
    if (!capturing) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      const parts: string[] = [];
      if (e.ctrlKey) parts.push("Control");
      if (e.metaKey) parts.push("Super");
      if (e.altKey) parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");
      const key = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
      if (!["Control", "Super", "Alt", "Shift"].includes(e.key)) parts.push(key);
      if (parts.length > 1) setConfig((c) => (c ? { ...c, shortcut: parts.join("+") } : c));
      setCapturing(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [capturing, setConfig]);

  return (
    <>
      <Card title="Appearance">
        <Field label="Theme" description="Two finishes only: warm paper or deep void.">
          <div className="flex w-44 rounded-full border border-line bg-sunken p-0.5">
            <button
              type="button"
              onClick={() => setTheme("paper")}
              className={`flex h-6 flex-1 items-center justify-center gap-1.5 rounded-full text-[11px] transition-colors duration-150 ${
                theme === "paper" ? "bg-fg text-bg" : "text-muted hover:text-fg"
              }`}
            >
              <Sun className="h-3 w-3" />
              paper
            </button>
            <button
              type="button"
              onClick={() => setTheme("void")}
              className={`flex h-6 flex-1 items-center justify-center gap-1.5 rounded-full text-[11px] transition-colors duration-150 ${
                theme === "void" ? "bg-fg text-bg" : "text-muted hover:text-fg"
              }`}
            >
              <Moon className="h-3 w-3" />
              void
            </button>
          </div>
        </Field>
      </Card>

      <Card title="Invocation">
        <Field label="Global shortcut" description="Click, then press the combo to capture. Applied on Save.">
          <button
            type="button"
            onClick={() => setCapturing(true)}
            className={`flex h-8 min-w-40 items-center justify-center rounded-[10px] border px-3 font-mono text-[12px] transition-colors duration-150 ${
              capturing
                ? "border-accent text-accent"
                : "border-line bg-sunken text-fg hover:border-line-strong"
            }`}
          >
            {capturing ? "press keys…" : config.shortcut}
          </button>
        </Field>

        <Field label="Maximum results" description="Hard ceiling per query. Lower keeps rendering instant.">
          <Stepper
            value={config.max_results}
            min={25}
            max={500}
            step={25}
            onChange={(v) => setConfig((c) => (c ? { ...c, max_results: v } : c))}
          />
        </Field>

        <Field
          label="Start at login"
          description={
            autostart === null ? "Unavailable on this system" : "Launch Kensaku when you sign in."
          }
        >
          <Switch
            checked={autostart ?? false}
            disabled={autostart === null || autostartBusy}
            onChange={toggleAutostart}
          />
        </Field>
        {autostartError && (
          <p className="break-words border-t border-line px-4 py-2 font-mono text-[10px] text-danger">
            Could not update login item: {autostartError}
          </p>
        )}
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* Folders                                                           */
/* ---------------------------------------------------------------- */

function FoldersSection({
  config,
  setConfig,
}: {
  config: KensakuConfig;
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>;
}) {
  const [input, setInput] = useState("");
  const { stats } = useStats(null);

  const addDir = () => {
    const t = input.trim();
    if (t && !config.watched_dirs.includes(t)) {
      setConfig((c) => (c ? { ...c, watched_dirs: [...c.watched_dirs, t] } : c));
      setInput("");
    }
  };

  const addDropped = (paths: string[]) =>
    setConfig((c) => {
      if (!c) return c;
      const fresh = paths.filter((p) => !c.watched_dirs.includes(p));
      return fresh.length ? { ...c, watched_dirs: [...c.watched_dirs, ...fresh] } : c;
    });

  return (
    <>
      <Card title="Watched directories">
        {config.watched_dirs.length === 0 && (
          <p className="px-4 py-3 text-[11px] text-subtle">Nothing watched yet.</p>
        )}
        {config.watched_dirs.map((dir) => (
          <div
            key={dir}
            className="flex items-center justify-between gap-3 border-b border-line px-4 py-2.5 last:border-b-0"
          >
            <div className="min-w-0">
              <p className="truncate text-[12px] text-fg">
                {dir.split("/").filter(Boolean).pop() || dir}
              </p>
              <p className="truncate font-mono text-[10px] text-subtle">{dir}</p>
            </div>
            <button
              type="button"
              title={`Stop watching ${dir}`}
              onClick={() =>
                setConfig((c) =>
                  c ? { ...c, watched_dirs: c.watched_dirs.filter((d) => d !== dir) } : c,
                )
              }
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-danger"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}

        {/* Drop zone */}
        <div
          onDragOver={(e) => {
            e.preventDefault();
            e.currentTarget.classList.add("border-accent", "text-fg");
          }}
          onDragLeave={(e) => {
            e.currentTarget.classList.remove("border-accent", "text-fg");
          }}
          onDrop={async (e) => {
            e.preventDefault();
            e.currentTarget.classList.remove("border-accent", "text-fg");
            const paths: string[] = [];
            for (const item of Array.from(e.dataTransfer.files)) {
              const p = (item as File & { path?: string }).path;
              if (p) paths.push(p);
            }
            if (paths.length === 0 && input.trim()) paths.push(input.trim());
            addDropped(paths);
          }}
          className="mx-4 my-2.5 flex h-16 items-center justify-center rounded-[10px] border border-dashed border-line text-[11px] text-subtle transition-colors duration-150"
        >
          Drop folders here to add them for watching
        </div>

        <div className="flex gap-1.5 border-t border-line bg-sunken p-2">
          <input
            type="text"
            value={input}
            placeholder="/absolute/path/to/directory"
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addDir()}
            spellCheck={false}
            className="h-8 min-w-0 flex-1 rounded-[10px] border border-line bg-elevated px-2.5 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
          />
          <button
            type="button"
            onClick={addDir}
            className="flex h-8 items-center gap-1 rounded-[10px] border border-line bg-elevated px-2.5 text-[11px] text-fg transition-colors hover:border-line-strong"
          >
            <Plus className="h-3.5 w-3.5" />
            Add
          </button>
        </div>
        {stats && stats.index.files > 0 && (
          <p className="border-t border-line px-4 py-2 font-mono text-[10px] text-subtle">
            {formatComma(stats.index.files)} files indexed across {formatComma(stats.index.dirs)} folders
          </p>
        )}
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* Exclusions                                                        */
/* ---------------------------------------------------------------- */

function ExclusionsSection({
  config,
  setConfig,
}: {
  config: KensakuConfig;
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>;
}) {
  const [dirInput, setDirInput] = useState("");
  const [extInput, setExtInput] = useState("");

  const removeDir = (d: string) =>
    setConfig((c) => (c ? { ...c, excluded_dirs: c.excluded_dirs.filter((x) => x !== d) } : c));

  return (
    <>
      <Card title="Presets">
        <div className="flex flex-wrap gap-1.5 px-4 py-3">
          {Object.keys(EXCLUSION_PRESETS).map((p) => (
            <button
              key={p}
              type="button"
              onClick={() => setConfig((c) => (c ? { ...c, excluded_dirs: [...EXCLUSION_PRESETS[p]] } : c))}
              className="flex h-7 items-center rounded-full border border-line px-3 text-[11px] text-muted transition-colors duration-150 hover:border-line-strong hover:text-fg"
            >
              {p}
            </button>
          ))}
          <span className="self-center text-[10px] text-subtle">replaces the list below</span>
        </div>
      </Card>

      <Card title="Excluded directory names">
        <div className="flex flex-wrap gap-1.5 px-4 py-3">
          {config.excluded_dirs.map((d) => (
            <button
              key={d}
              type="button"
              title={`Remove ${d}`}
              onClick={() => removeDir(d)}
              className="group flex h-6 items-center gap-1.5 rounded-full bg-sunken px-2.5 font-mono text-[10px] text-muted transition-colors hover:text-danger"
            >
              {d}
              <X className="h-3 w-3 opacity-50 group-hover:opacity-100" />
            </button>
          ))}
        </div>
        <div className="border-t border-line bg-sunken p-2">
          <input
            value={dirInput}
            placeholder="directory name, e.g. bundle_cache — Enter to add"
            onChange={(e) => setDirInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && dirInput.trim()) {
                const t = dirInput.trim();
                setConfig((c) =>
                  c && !c.excluded_dirs.includes(t) ? { ...c, excluded_dirs: [...c.excluded_dirs, t] } : c,
                );
                setDirInput("");
              }
            }}
            spellCheck={false}
            className="h-8 w-full rounded-[10px] border border-line bg-elevated px-2.5 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
          />
        </div>
      </Card>

      <Card title="Excluded extensions">
        <div className="flex flex-wrap gap-1.5 px-4 py-3">
          {config.excluded_extensions.map((x) => (
            <button
              key={x}
              type="button"
              title={`Remove ${x}`}
              onClick={() =>
                setConfig((c) =>
                  c ? { ...c, excluded_extensions: c.excluded_extensions.filter((e) => e !== x) } : c,
                )
              }
              className="group flex h-6 items-center gap-1.5 rounded-full bg-sunken px-2.5 font-mono text-[10px] text-muted transition-colors hover:text-danger"
            >
              {x}
              <X className="h-3 w-3 opacity-50 group-hover:opacity-100" />
            </button>
          ))}
        </div>
        <div className="border-t border-line bg-sunken p-2">
          <input
            value={extInput}
            placeholder="extension, e.g. .bin — Enter to add"
            onChange={(e) => setExtInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && extInput.trim()) {
                const t = extInput.trim();
                setConfig((c) =>
                  c && !c.excluded_extensions.includes(t)
                    ? { ...c, excluded_extensions: [...c.excluded_extensions, t] }
                    : c,
                );
                setExtInput("");
              }
            }}
            spellCheck={false}
            className="h-8 w-full rounded-[10px] border border-line bg-elevated px-2.5 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
          />
        </div>
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* Search                                                            */
/* ---------------------------------------------------------------- */

const CHEATSHEET: [string, string][] = [
  ["type:image,dir,doc,code,media", "Class of file"],
  ["ext:png", "Restrict by extension"],
  ['in:"~/Projects"', "Within folder, recursively"],
  ["modified:<7d", "Changed recently"],
  ["size:>2mb", "Size threshold"],
  ["-ext:log", "Negate any filter"],
  [">reindex", "Rebuild the full index"],
  [">doctor", "Health diagnostics"],
  [">model", "Embedding model status"],
  [">add folder", "Register a watch path"],
];

function SearchSection({
  config,
  setConfig,
}: {
  config: KensakuConfig;
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>;
}) {
  return (
    <>
      <Card title="Ranking">
        <Field
          label="Semantic search"
          description="Static embeddings, ~0.1 ms per query, fused with BM25 via RRF."
        >
          <Switch
            checked={config.semantic_enabled ?? true}
            onChange={(v) => setConfig((c) => (c ? { ...c, semantic_enabled: v } : c))}
          />
        </Field>
        <OcrField config={config} setConfig={setConfig} />
      </Card>

      <Card title="Aliases">
        <div className="space-y-2 p-4">
          <p className="text-[11px] leading-relaxed text-muted">
            Personal vocabulary: typing the term also matches any of its expansions
            (case-insensitive, OR semantics).
          </p>
          {(config.aliases ?? []).map((alias, i) => (
            <div key={i} className="flex items-center gap-2">
              <input
                type="text"
                value={alias.term}
                placeholder="term"
                onChange={(e) =>
                  setConfig((c) => {
                    if (!c) return c;
                    const aliases = [...(c.aliases ?? [])];
                    aliases[i] = { ...aliases[i], term: e.target.value };
                    return { ...c, aliases };
                  })
                }
                className="h-8 w-28 shrink-0 rounded-[8px] border border-line bg-elevated px-2 font-mono text-[11px] text-fg outline-none focus:border-accent"
              />
              <span className="text-subtle">→</span>
              <input
                type="text"
                value={alias.expansions.join(", ")}
                placeholder="cv, curriculum"
                onChange={(e) =>
                  setConfig((c) => {
                    if (!c) return c;
                    const aliases = [...(c.aliases ?? [])];
                    aliases[i] = {
                      ...aliases[i],
                      expansions: e.target.value.split(",").map((s) => s.trim()).filter(Boolean),
                    };
                    return { ...c, aliases };
                  })
                }
                className="h-8 min-w-0 flex-1 rounded-[8px] border border-line bg-elevated px-2 font-mono text-[11px] text-fg outline-none focus:border-accent"
              />
              <button
                type="button"
                title="Remove alias"
                onClick={() =>
                  setConfig((c) =>
                    c ? { ...c, aliases: (c.aliases ?? []).filter((_, j) => j !== i) } : c,
                  )
                }
                className="flex h-7 w-7 shrink-0 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-danger"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              setConfig((c) =>
                c ? { ...c, aliases: [...(c.aliases ?? []), { term: "", expansions: [] }] } : c,
              )
            }
            className="flex h-7 items-center gap-1 rounded-[8px] border border-line px-2.5 text-[11px] text-muted transition-colors hover:border-line-strong hover:text-fg"
          >
            <Plus className="h-3 w-3" /> Add alias
          </button>
        </div>
      </Card>

      <Card title="Query language">
        <div className="grid grid-cols-2 gap-x-8 px-4 py-2.5">
          {CHEATSHEET.map(([token, desc], idx) => (
            <div
              key={token}
              className={`flex h-8 items-center justify-between gap-3 ${
                idx < CHEATSHEET.length - 2 ? "border-b border-line/70" : ""
              }`}
            >
              <code className="shrink-0 whitespace-nowrap text-fg">{token}</code>
              <span className="truncate text-right text-[11px] text-subtle">{desc}</span>
            </div>
          ))}
        </div>
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* AI & Semantic                                                     */
/* ---------------------------------------------------------------- */

function AiSection({ config }: { config: KensakuConfig }) {
  const { stats, live } = useStats(2000);
  const [reembedState, setReembedState] = useState<"idle" | "pending">("idle");

  const modelId =
    stats?.runtime?.model_id || config.semantic_model || "reported when the backend lands";
  const queueDepth = stats?.ingestion?.embed_queue_depth ?? 0;

  const reembed = async () => {
    setReembedState("pending");
    try {
      await invoke("reembed_all");
    } catch {
      console.warn("reembed_all not available yet");
    }
    window.setTimeout(() => setReembedState("idle"), 1500);
  };

  return (
    <>
      <Card title="Embedding model">
        <Field label="Model" description="Static embeddings — no GPU, no network.">
          <span className="max-w-48 truncate font-mono text-[11px] text-fg">{modelId}</span>
        </Field>
        <Field label="Dimensions" description="Reported by the model manifest.">
          <span className="font-mono text-[11px] text-muted">{live ? "—" : "—"}</span>
        </Field>
        <Field label="Vector tier on disk" description="What the embeddings cost you in bytes.">
          <span className="font-mono text-[11px] text-muted">{formatBytes(stats?.index?.vector_bytes ?? 0)}</span>
        </Field>
        <Field label="Readiness" description="Units searchable in RRF fusion right now.">
          <span
            className={`font-mono text-[11px] ${
              stats?.runtime?.semantic_ready ? "text-success" : "text-warning"
            }`}
          >
            {stats?.runtime?.semantic_ready ? "ready" : live ? "warming up" : "pending backend"}
          </span>
        </Field>
      </Card>

      <Card title="Embed queue">
        <div className="space-y-2 p-4">
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-muted">Queued units</span>
            <span className="font-mono text-fg">{formatComma(queueDepth)}</span>
          </div>
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken">
            <div
              className="h-full rounded-full bg-accent transition-[width] duration-500"
              style={{ width: `${queueDepth > 0 ? Math.max(4, Math.min(100, queueDepth)) : 0}%` }}
            />
          </div>
          <div className="flex items-center justify-between pt-1">
            <span className="font-mono text-[10px] text-subtle">
              {live ? "polled every 2 s" : "get_stats pending on the backend"}
            </span>
            <button
              type="button"
              onClick={reembed}
              className="flex h-7 items-center gap-1.5 rounded-[10px] border border-line px-2.5 text-[11px] text-fg transition-colors hover:border-line-strong"
            >
              <RefreshCw className="h-3 w-3" />
              {reembedState === "pending" ? "Queued…" : "Re-embed all"}
            </button>
          </div>
        </div>
      </Card>

      <ModelFilesCard />
    </>
  );
}

/* ---------------------------------------------------------------- */
/* Agents                                                            */
/* ---------------------------------------------------------------- */

const MCP_CONFIG = `{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}`;

const CLI_EXAMPLES = [
  'kensaku-cli search "docker error" --json --limit 10',
  'kensaku-cli search "type:pdf invoice" --path ~/Documents',
  'kensaku-cli state --json',
];

const MCP_TOOLS: [string, string][] = [
  ["search", "query, --json, --limit, --path, --ext, --since, --exhaustive"],
  ["read_snippet", "path, query?, start_line?, max_tokens <= 512"],
  ["index_state", "per-root counts, generation, queue depth, freshness"],
];

/** One copyable mono row — command text with a trailing copy button. */
function CopyRow({
  text,
  copied,
  onCopied,
}: {
  text: string;
  copied: boolean;
  onCopied: () => void;
}) {
  return (
    <div className="flex items-center gap-2 border-b border-line/60 px-3 py-2 last:border-b-0">
      <code className="no-scrollbar min-w-0 flex-1 select-text overflow-x-auto whitespace-nowrap font-mono text-[11px] leading-5 text-fg">
        {text}
      </code>
      <button
        type="button"
        title="Copy"
        onClick={() => {
          navigator.clipboard.writeText(text);
          onCopied();
        }}
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-subtle transition-colors hover:bg-elevated hover:text-fg"
      >
        {copied ? <Check className="h-3 w-3 text-accent" /> : <Copy className="h-3 w-3" />}
      </button>
    </div>
  );
}

function AgentsSection() {
  const [configCopied, setConfigCopied] = useState(false);
  const [copiedRow, setCopiedRow] = useState<string | null>(null);

  const flashRow = (text: string) => {
    setCopiedRow(text);
    window.setTimeout(() => setCopiedRow((r) => (r === text ? null : r)), 1500);
  };

  return (
    <>
      <p className="pb-3 text-[12px] leading-relaxed text-muted">
        Let your AI agents search this index. Read-only, local, no network.
      </p>

      {/* MCP */}
      <Card title="Model Context Protocol">
        <div className="px-4 py-3">
          <p className="text-[11px] leading-relaxed text-muted">
            Point Claude Code, Cursor, or any MCP client at the{" "}
            <code className="font-mono text-[10.5px] text-fg">kensaku-cli</code> binary. It shares
            this app's index — no daemon to start, nothing to configure twice.
          </p>
        </div>
        <div className="relative border-t border-line bg-sunken p-3 pr-10">
          <pre className="select-text whitespace-pre-wrap break-all font-mono text-[10.5px] leading-relaxed text-muted">
            {MCP_CONFIG}
          </pre>
          <button
            type="button"
            title="Copy config JSON"
            onClick={() => {
              navigator.clipboard.writeText(MCP_CONFIG);
              setConfigCopied(true);
              window.setTimeout(() => setConfigCopied(false), 1500);
            }}
            className="absolute right-2 top-2 flex h-6 w-6 items-center justify-center rounded text-subtle transition-colors hover:bg-elevated hover:text-fg"
          >
            {configCopied ? <Check className="h-3.5 w-3.5 text-accent" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
        </div>
        <p className="border-t border-line px-4 py-2.5 text-[11px] leading-relaxed text-muted">
          If <code className="font-mono text-[10.5px] text-fg">kensaku-cli</code> isn't on your
          PATH, use the absolute path to the binary —{" "}
          <code className="font-mono text-[10.5px] text-fg">~/.cargo/bin/kensaku-cli</code> or{" "}
          <code className="font-mono text-[10.5px] text-fg">target/release/kensaku-cli</code>.
        </p>
      </Card>

      {/* CLI */}
      <Card title="Command line">
        <div>
          {CLI_EXAMPLES.map((cmd) => (
            <CopyRow
              key={cmd}
              text={cmd}
              copied={copiedRow === cmd}
              onCopied={() => flashRow(cmd)}
            />
          ))}
        </div>
        <p className="border-t border-line px-4 py-2.5 text-[11px] leading-relaxed text-muted">
          The MCP server loads the index once per session; per-query latency is the same as the
          app.
        </p>
      </Card>

      {/* Tools */}
      <Card title="MCP tools">
        <div className="px-4 py-1.5">
          {MCP_TOOLS.map(([name, args]) => (
            <div
              key={name}
              className="flex items-baseline justify-between gap-4 border-b border-line/60 py-2.5 last:border-b-0"
            >
              <code className="shrink-0 whitespace-nowrap font-mono text-[11px] font-medium text-fg">
                {name}
              </code>
              <span className="min-w-0 truncate text-right font-mono text-[10px] text-subtle">
                {args}
              </span>
            </div>
          ))}
        </div>
      </Card>

      {/* Security */}
      <Card title="Security">
        <p className="p-4 text-[11px] leading-relaxed text-muted">
          Read-only. No writes, no shell, no network. Honors your watched folders and exclusions.
        </p>
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* About + Danger zone                                               */
/* ---------------------------------------------------------------- */

const CONFIRM_WORD = "RESET";

/** Best-effort: the backend may not ship get_reset_pending yet. */
function useResetPending(): boolean {
  const [pending, setPending] = useState(false);
  useEffect(() => {
    invoke<boolean>("get_reset_pending")
      .then(setPending)
      .catch(() => setPending(false));
  }, []);
  return pending;
}

/** Tracks keys the app persists in localStorage, so a reset starts clean. */
function clearClientState() {
  try {
    for (const key of ["kensaku.theme", "kensaku.view", "kensaku.preview"]) {
      localStorage.removeItem(key);
    }
  } catch {
    /* storage unavailable — the backend reset owns the rest */
  }
}

/**
 * Type-RESET confirmation. Traps focus, Esc cancels, Enter submits only
 * when the word matches exactly. On reject the error surfaces inline and
 * nothing is cleared; on resolve the app is expected to restart.
 */
function ResetDialog({ onClose }: { onClose: () => void }) {
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = React.useRef<HTMLDivElement>(null);
  const matches = text === CONFIRM_WORD;

  /* Window-level capture so Esc/Tab are handled before the settings
     overlay's own Escape-to-close listener sees them. */
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusables = () =>
      Array.from(dialog.querySelectorAll<HTMLElement>("input, button:not([disabled])"));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key === "Tab") {
        const list = focusables();
        if (list.length === 0) return;
        const first = list[0];
        const last = list[list.length - 1];
        const active = document.activeElement;
        if (e.shiftKey && active === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && (active === last || !dialog.contains(active))) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    const input = dialog.querySelector<HTMLInputElement>("input");
    input?.focus();
    input?.select();
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const submit = async () => {
    if (!matches || running) return;
    setRunning(true);
    setError(null);
    try {
      await invoke("reset_all", { confirm: CONFIRM_WORD });
      // Resolves → the app restarts. Clear client-only view state so a
      // relaunched window comes up fresh; show nothing.
      clearClientState();
      onClose();
    } catch (err) {
      // Rejects → keep everything, surface the reason inline.
      console.error("reset_all failed:", err);
      setError(String(err));
      setRunning(false);
    }
  };

  return (
    <div
      className="fade-in absolute inset-0 z-50 flex items-center justify-center p-6 backdrop-blur-[2px]"
      style={{ background: "var(--scrim)" }}
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Reset Kensaku"
        className="overlay-in w-full max-w-[400px] overflow-hidden rounded-[14px] border border-danger/30 bg-elevated shadow-lg"
      >
        <div className="flex items-center justify-between border-b border-line bg-sunken px-4 py-2.5">
          <p className="text-[10px] font-medium uppercase tracking-[0.08em] text-danger">Danger zone</p>
          <button
            type="button"
            title="Cancel (Esc)"
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded text-muted transition-colors hover:bg-elevated hover:text-fg"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <div className="p-4">
            <p className="text-[13px] font-semibold text-fg">Reset Kensaku</p>
            <p className="pt-0.5 text-[11px] leading-relaxed text-muted">
              This will delete your index and settings and restart Kensaku.
            </p>
          </div>

          <div className="px-4 pb-4">
            <label htmlFor="reset-confirm" className="eyebrow pb-1.5 block">
              Type <span className="font-mono normal-case text-danger">RESET</span> to confirm
            </label>
            <input
              id="reset-confirm"
              type="text"
              value={text}
              onChange={(e) => setText(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              className={`h-9 w-full rounded-[10px] border bg-sunken px-2.5 font-mono text-[12px] text-fg outline-none transition-colors focus:border-accent ${
                text && !matches ? "border-danger/50" : "border-line"
              }`}
            />
            {error && (
              <p className="break-words pt-2 font-mono text-[10px] text-danger">{error}</p>
            )}
          </div>

          <div className="flex items-center justify-end gap-1.5 border-t border-line bg-sunken px-3 py-2.5">
            <button
              type="button"
              onClick={onClose}
              className="flex h-8 items-center rounded-[10px] border border-line px-3 text-[12px] text-fg transition-colors hover:border-line-strong"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!matches || running}
              className="flex h-8 items-center rounded-[10px] bg-danger px-3 text-[12px] font-medium text-bg transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {running ? "Resetting…" : "Reset and restart"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function AboutSection() {
  const resetPending = useResetPending();
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <>
      {resetPending && (
        <div className="flex items-center gap-2 rounded-[14px] border border-warning/40 bg-elevated px-4 py-2.5">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-warning" />
          <p className="text-[11px] text-muted">A reset is pending and will finish on next launch.</p>
        </div>
      )}
      {resetPending && <div className="h-3" />}

      <Card title="Version">
        <div className="flex items-center justify-between p-4">
          <div>
            <p className="text-[15px] font-semibold text-fg">
              Kensaku <span className="font-normal text-muted">検索</span>
            </p>
            <p className="font-mono text-[11px] text-subtle">0.1.0</p>
          </div>
          <div className="flex items-center gap-1.5">
            {["Rust", "Tauri 2", "MIT"].map((b) => (
              <span
                key={b}
                className="rounded-full border border-line px-2.5 py-1 font-mono text-[10px] text-muted"
              >
                {b}
              </span>
            ))}
          </div>
        </div>
      </Card>

      <Card title="Design thesis">
        <div className="p-4 text-center">
          <p className="font-serif text-[15px] italic leading-relaxed text-muted">
            “A warm, local, read-only index of your machine that answers in under 15 ms —
            for you, and for every agent you use.”
          </p>
        </div>
      </Card>

      <Card title="Setup">
        <div className="flex items-center justify-between gap-4 p-4">
          <div className="min-w-0">
            <p className="text-[13px] font-medium text-fg">Run setup again</p>
            <p className="line-clamp-2 pt-0.5 text-[11px] leading-relaxed text-muted">
              Re-run the guided first-run flow (folders, preferences, optional packs, first scan).
              Your index and config are preserved; setup markers reset.
            </p>
          </div>
          <button
            type="button"
            onClick={async () => {
              try {
                await invoke("run_setup_again");
                window.location.reload();
              } catch (e) {
                console.error(e);
              }
            }}
            className="h-8 shrink-0 rounded-[10px] border border-line px-3 text-[11px] font-medium text-fg transition-colors hover:border-line-strong"
          >
            Run setup again…
          </button>
        </div>
      </Card>

      {/* Danger zone — visually distinct but calm: danger hairline, no fills. */}
      <div>
        <p className="pb-2 text-[10px] font-medium uppercase tracking-[0.08em] text-danger">
          Danger zone
        </p>
        <div className="overflow-hidden rounded-[14px] border border-danger/30 bg-elevated">
          <div className="flex items-center justify-between gap-4 p-4">
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-fg">Reset Kensaku</p>
              <p className="line-clamp-2 pt-0.5 text-[11px] leading-relaxed text-muted">
                Deletes the index, metadata database, downloaded models, and settings. This cannot
                be undone.
              </p>
            </div>
            <button
              type="button"
              onClick={() => setConfirmOpen(true)}
              className="h-8 shrink-0 rounded-[10px] border border-danger/40 px-3 text-[11px] font-medium text-danger transition-colors hover:bg-danger/10"
            >
              Reset everything…
            </button>
          </div>
        </div>
      </div>

      {confirmOpen && <ResetDialog onClose={() => setConfirmOpen(false)} />}
    </>
  );
}
