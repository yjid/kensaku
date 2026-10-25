import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Plus, RefreshCw, Save, Check, Copy, Sun, Moon } from "lucide-react";
import { KensakuConfig } from "../types";
import { useTheme } from "../hooks/useTheme";
import { useStats } from "../hooks/useStats";
import { Card, Field, Switch, Stepper } from "./settings/kit";
import { StatsForNerds } from "./settings/stats";
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
  | "about";

const SECTIONS: { id: SectionId; label: string }[] = [
  { id: "general", label: "General" },
  { id: "folders", label: "Folders" },
  { id: "exclusions", label: "Exclusions" },
  { id: "search", label: "Search" },
  { id: "ai", label: "AI & Semantic" },
  { id: "agents", label: "Agents" },
  { id: "stats", label: "Stats for Nerds" },
  { id: "about", label: "About" },
];

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
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<KensakuConfig>("get_config")
      .then(setConfig)
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
        <nav className="w-[200px] shrink-0 overflow-y-auto border-r border-line bg-sunken/50 p-2">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              type="button"
              onClick={() => setSection(s.id)}
              className={`flex h-8 w-full items-center rounded-[10px] px-2.5 text-left text-[12px] transition-colors duration-150 ${
                section === s.id
                  ? "bg-fg font-medium text-bg"
                  : "text-muted hover:bg-elevated hover:text-fg"
              }`}
            >
              {s.label}
            </button>
          ))}
        </nav>

        <main className="min-w-0 flex-1 overflow-y-auto bg-sunken/30 p-5">
          {config ? renderSection(section, config, setConfig) : <ConfigSkeleton />}

          <div className="mt-4 flex items-center gap-2">
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
              disabled={saving || !config}
              className="flex h-8 items-center gap-1.5 rounded-[10px] bg-accent px-3.5 text-[12px] font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90 disabled:opacity-50"
            >
              {saved ? <Check className="h-3.5 w-3.5" /> : <Save className="h-3.5 w-3.5" />}
              {saved ? "Saved" : saving ? "Saving…" : "Save"}
            </button>
          </div>
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

        <Field label="Start at login" description="Managed by the daemon in a later build.">
          <Switch checked={false} disabled onChange={() => {}} />
        </Field>
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
          className="m-2 flex h-16 items-center justify-center rounded-[10px] border border-dashed border-line text-[11px] text-subtle transition-colors duration-150"
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
        <div className="flex flex-wrap gap-1.5 p-3">
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
        <div className="flex flex-wrap gap-1.5 p-3">
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
        <div className="flex flex-wrap gap-1.5 p-3">
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
        <Field label="OCR (images)" description="Lands with backend phase 6.">
          <Switch checked={false} disabled onChange={() => {}} />
        </Field>
      </Card>

      <Card title="Query language">
        <div className="grid grid-cols-2 gap-x-8 p-3">
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
              style={{ width: `${Math.max(4, Math.min(100, 100 - Math.min(96, queueDepth)))}%` }}
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

      <Card title="Smart pack">
        <div className="flex items-center justify-between gap-4 p-4 opacity-50">
          <div>
            <p className="text-[12px] font-medium text-fg">Prebuilt smart pack</p>
            <p className="text-[11px] leading-relaxed text-muted">
              A curated model + lexicon bundle for faster first-run indexing.
            </p>
          </div>
          <button
            type="button"
            disabled
            className="flex h-7 shrink-0 items-center gap-1.5 rounded-[10px] border border-line px-2.5 text-[11px] text-subtle"
          >
            Download — soon
          </button>
        </div>
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* Agents                                                            */
/* ---------------------------------------------------------------- */

const MCP_SNIPPET = `# Claude Code / Cursor — Kensaku MCP (read-only)
claude mcp add kensaku --transport stdio -- kensaku mcp

# ...or point any MCP client at:
#   command: kensaku
#   args:    ["mcp"]`;

function AgentsSection() {
  const [copied, setCopied] = useState(false);
  return (
    <>
      <Card title="Agent API (MCP)">
        <div className="flex items-center justify-between gap-4 p-4">
          <div>
            <p className="flex items-center gap-2 text-[12px] font-medium text-fg">
              Coming soon
              <span className="rounded-full bg-sunken px-2 py-0.5 font-mono text-[9px] uppercase tracking-wide text-subtle">
                disabled
              </span>
            </p>
            <p className="pt-0.5 text-[11px] leading-relaxed text-muted">
              Local stdio MCP server. Read-only: search, read_snippet, find_related, index_state.
            </p>
          </div>
          <svg
            viewBox="0 0 24 24"
            className="h-5 w-5 shrink-0 text-subtle"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <circle cx="6" cy="6" r="2.5" />
            <circle cx="6" cy="18" r="2.5" />
            <circle cx="18" cy="7" r="2.5" />
            <path d="M6 8.5v7M13.5 8.5c0 3-7.5 2.5-7.5 7M18 9.5c0 3-4 2.5-4 5" />
          </svg>
        </div>
        <div className="relative border-t border-line bg-sunken p-3 pr-10">
          <pre className="select-text whitespace-pre-wrap break-all font-mono text-[10.5px] leading-relaxed text-muted">
            {MCP_SNIPPET}
          </pre>
          <button
            type="button"
            title="Copy config snippet"
            onClick={() => {
              navigator.clipboard.writeText(MCP_SNIPPET);
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1200);
            }}
            className="absolute right-2 top-2 flex h-6 w-6 items-center justify-center rounded text-subtle transition-colors hover:bg-elevated hover:text-fg"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-accent" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
        </div>
      </Card>

      <Card title="Write access">
        <p className="p-4 text-[11px] leading-relaxed text-muted">
          There will never be write operations through the agent API. The index is rebuilt
          exclusively by the daemon's own reconciler and watcher.
        </p>
      </Card>
    </>
  );
}

/* ---------------------------------------------------------------- */
/* About                                                             */
/* ---------------------------------------------------------------- */

function AboutSection() {
  return (
    <>
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
        <div className="p-4 pt-1 text-center">
          <p className="font-serif text-[15px] italic leading-relaxed text-muted">
            “A warm, local, read-only index of your machine that answers in under 15 ms —
            for you, and for every agent you use.”
          </p>
        </div>
      </Card>
    </>
  );
}
