import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Plus, X, ArrowRight, ArrowLeft, LoaderCircle, Sun, Moon, Download } from "lucide-react";
import { EmptyState } from "./EmptyState";
import { useTheme } from "../hooks/useTheme";
import type { IndexingStatus, ModelsStatus, OcrStatus } from "../types";

/* First run: three screens, one decision each.
   1 Welcome → 2 Pick folders → 3 Exclusions + start. */

const DEFAULT_EXCLUSIONS = [
  "node_modules", ".git", "target", "dist", "build", "out",
  ".next", ".nuxt", ".output", ".cache", ".venv", "venv",
  "__pycache__", "vendor", ".gradle", ".idea", ".vscode",
  ".Trash", "lost+found",
];

const PRESETS: Record<string, string[]> = {
  Developer: DEFAULT_EXCLUSIONS.slice(0, 17),
  Creative: ["node_modules", ".git", ".Trash", ".cache", "Backups", "Library", "lost+found"],
  Minimal: [".Trash", "lost+found"],
};

function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  if (seconds < 60) return `~${Math.max(1, Math.round(seconds))}s left`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (mins < 60) return `~${mins}m ${secs}s left`;
  const hours = Math.floor(mins / 60);
  return `~${hours}h ${mins % 60}m left`;
}

export function SetupModal({ onSetupCompleted }: { onSetupCompleted: () => void }) {
  const [step, setStep] = useState<0 | 1 | 2 | 3 | 4>(0);
  const [suggestedDirs, setSuggestedDirs] = useState<string[]>([]);
  const [selectedDirs, setSelectedDirs] = useState<string[]>([]);
  const [customDirInput, setCustomDirInput] = useState("");
  const [excludedDirs, setExcludedDirs] = useState<string[]>(DEFAULT_EXCLUSIONS);
  const [customExcludeInput, setCustomExcludeInput] = useState("");
  const [shortcut, setShortcut] = useState("Alt+Space");
  const [capturing, setCapturing] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanStatus, setScanStatus] = useState<IndexingStatus | null>(null);
  const [done, setDone] = useState(false);
  const [modelsStatus, setModelsStatus] = useState<ModelsStatus | null>(null);
  const [ocrStatus, setOcrStatus] = useState<OcrStatus | null>(null);
  const [ocrBusy, setOcrBusy] = useState(false);
  const [packError, setPackError] = useState<string | null>(null);
  const etaRef = useRef<{ phase: string; done: number; at: number; total: number }>({
    phase: "",
    done: 0,
    at: 0,
    total: 0,
  });
  const { theme, setTheme } = useTheme();

  useEffect(() => {
    invoke<string[]>("get_suggested_directories")
      .then((dirs) => {
        setSuggestedDirs(dirs);
        if (dirs.length > 0) setSelectedDirs(dirs.slice(0, 3));
      })
      .catch((err) => console.error("Failed to load suggested dirs:", err));

    // Resume an interrupted first scan: if setup is already configured but the
    // first scan never finished, skip straight to the scanning state.
    invoke<boolean>("is_setup_configured")
      .then((configured) => {
        if (configured) setScanning(true);
      })
      .catch(() => {});
  }, []);

  // Capture a global shortcut by pressing the key combination.
  useEffect(() => {
    if (!capturing) return;
    // Modifiers accumulate across keydowns, so tapping Ctrl then K works as
    // well as holding Ctrl+K.
    const pressed = new Set<string>();
    const normalizeModifier = (key: string) =>
      key === "Meta" ? "Super" : key;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      if (e.key === "Escape") {
        setCapturing(false);
        return;
      }
      if (["Control", "Super", "Meta", "Alt", "Shift"].includes(e.key)) {
        pressed.add(normalizeModifier(e.key));
        return;
      }
      if (e.ctrlKey) pressed.add("Control");
      if (e.metaKey) pressed.add("Super");
      if (e.altKey) pressed.add("Alt");
      if (e.shiftKey) pressed.add("Shift");
      const order = ["Control", "Super", "Alt", "Shift"];
      const parts = order.filter((m) => pressed.has(m));
      const key = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
      parts.push(key);
      setShortcut(parts.join("+"));
      setCapturing(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [capturing]);

  // Optional packs: load state when the step opens and poll while downloading.
  useEffect(() => {
    if (step !== 4) return;
    invoke<ModelsStatus>("get_models_status").then(setModelsStatus).catch(() => {});
    invoke<OcrStatus>("get_ocr_status").then(setOcrStatus).catch(() => {});
  }, [step]);

  useEffect(() => {
    if (step !== 4 || !modelsStatus?.downloading) return;
    const id = window.setInterval(() => {
      invoke<ModelsStatus>("get_models_status").then(setModelsStatus).catch(() => {});
    }, 600);
    return () => window.clearInterval(id);
  }, [step, modelsStatus?.downloading]);

  // Poll indexing status while scanning; finish when the crawl completes.
  useEffect(() => {
    if (!scanning || done) return;
    let cancelled = false;
    const tick = async () => {
      try {
        const st = await invoke<IndexingStatus>("get_indexing_status");
        if (cancelled) return;
        setScanStatus(st);
        const isDone = await invoke<boolean>("is_setup_complete");
        if (isDone) {
          if (!cancelled) {
            setScanStatus(st);
            setDone(true);
          }
        }
      } catch (err) {
        console.error("Scan status error:", err);
      }
    };
    tick();
    const interval = setInterval(tick, 800);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [scanning, done, onSetupCompleted]);

  const toggleDir = (dir: string) =>
    setSelectedDirs((prev) =>
      prev.includes(dir) ? prev.filter((d) => d !== dir) : [...prev, dir],
    );

  const addCustomDir = () => {
    const t = customDirInput.trim();
    if (t && !selectedDirs.includes(t)) {
      setSelectedDirs((prev) => [...prev, t]);
      setCustomDirInput("");
    }
  };

  const handleFinish = async () => {
    if (selectedDirs.length === 0) return;
    setIsSubmitting(true);
    try {
      await invoke("complete_setup", {
        watchedDirs: selectedDirs,
        excludedDirs: excludedDirs,
        shortcut: shortcut,
      });
      setScanning(true);
    } catch (err) {
      console.error("Setup failed:", err);
      alert("Setup failed: " + err);
    } finally {
      setIsSubmitting(false);
    }
  };

  if (done) {
    const st = scanStatus;
    const files = st?.files_indexed ?? 0;
    const content = st?.content_indexed ?? 0;
    const units = st?.units_indexed ?? 0;
    return (
      <div className="absolute inset-0 z-50 flex flex-col items-center justify-center gap-6 bg-bg px-8 text-center">
        <div className="flex flex-col items-center gap-2">
          <span className="text-[15px] font-semibold tracking-tight text-fg">Kensaku</span>
          <span className="font-serif text-[13px] italic text-subtle">検索 — to search</span>
        </div>
        <div className="flex h-10 w-10 items-center justify-center rounded-full bg-accent text-accent-fg">
          <Check className="h-5 w-5" />
        </div>
        <div className="space-y-1">
          <h1 className="text-[18px] font-semibold tracking-tight text-fg">Done.</h1>
          <p className="text-[12px] text-muted">Your index is ready. Start searching.</p>
        </div>
        <div className="grid grid-cols-3 gap-6 text-center">
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{files.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">files</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{content.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">documents</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{units.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">embedded</div>
          </div>
        </div>
        <button
          type="button"
          onClick={onSetupCompleted}
          className="flex h-9 items-center gap-1.5 rounded-[10px] bg-accent px-5 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90"
        >
          Start searching <ArrowRight className="h-3.5 w-3.5" />
        </button>
      </div>
    );
  }

  if (scanning) {
    const st = scanStatus;
    const files = st?.files_indexed ?? 0;
    const content = st?.content_indexed ?? 0;
    const units = st?.units_indexed ?? 0;
    const phase = st?.phase ?? "walking";

    // ETA from the observed throughput of the current phase. Fall back to the
    // metadata counters when an older backend reports `extracting` before the
    // extraction queue is populated (extract_total == 0).
    const extractTotal = st?.extract_total ?? 0;
    const extracting = phase === "extracting" && extractTotal > 0;
    const done = extracting ? (st?.extract_done ?? 0) : files;
    const total = extracting ? extractTotal : (st?.total_files ?? 0);
    let eta = "";
    let pct = 0;
    if (total > 0) {
      pct = Math.min(100, Math.round((done / total) * 100));
      if (etaRef.current.phase !== phase || etaRef.current.total !== total) {
        etaRef.current = { phase, done: 0, at: Date.now(), total };
      }
      const elapsed = Math.max(0, (Date.now() - etaRef.current.at) / 1000);
      if (done > 0 && total > done && elapsed >= 1) {
        // Average throughput for the phase: stable, no per-tick flicker.
        eta = formatEta((elapsed * (total - done)) / done);
      }
    }

    return (
      <div className="absolute inset-0 z-50 flex flex-col items-center justify-center gap-6 bg-bg px-8 text-center">
        <div className="flex flex-col items-center gap-2">
          <span className="text-[15px] font-semibold tracking-tight text-fg">Kensaku</span>
          <span className="font-serif text-[13px] italic text-subtle">検索 — to search</span>
        </div>
        <LoaderCircle className="h-8 w-8 animate-spin text-accent" />
        <div className="space-y-1">
          <h1 className="text-[18px] font-semibold tracking-tight text-fg">
            Indexing your files…
          </h1>
          <p className="text-[12px] text-muted">
            {st?.current_status ?? "Starting scan…"}
          </p>
        </div>

        {total > 0 && (
          <div className="w-full max-w-sm space-y-1.5">
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken">
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-300"
                style={{ width: `${Math.max(3, pct)}%` }}
              />
            </div>
            <div className="flex items-center justify-between text-[10px] text-subtle">
              <span className="font-mono">
                {done.toLocaleString()} / {total.toLocaleString()}
              </span>
              <span className="font-mono">{eta || (pct >= 100 ? "finishing…" : "estimating…")}</span>
            </div>
          </div>
        )}

        <div className="grid grid-cols-3 gap-6 text-center">
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{files.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">files</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{content.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">documents</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{units.toLocaleString()}</div>
            <div className="text-[10px] uppercase tracking-wider text-subtle">embedded</div>
          </div>
        </div>
        <p className="max-w-sm text-[11px] text-subtle">
          Phase: {phase}. This only happens on the first run. Later launches are instant.
        </p>
      </div>
    );
  }

  return (
    <div className="absolute inset-0 z-50 flex flex-col bg-bg">
      {/* Header: wordmark + step indicator + theme toggle */}
      <header className="flex h-14 shrink-0 items-center justify-between px-5">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-semibold tracking-tight text-fg">Kensaku</span>
          <span className="font-serif text-[13px] italic text-subtle">検索 — to search</span>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            {[0, 1, 2, 3, 4].map((s) => (
              <span
                key={s}
                className={`h-1 w-6 rounded-full transition-colors duration-300 ${
                  s <= step ? "bg-accent" : "bg-line"
                }`}
              />
            ))}
          </div>
          <button
            type="button"
            title="Toggle theme"
            onClick={() => setTheme(theme === "paper" ? "void" : "paper")}
            className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors hover:bg-sunken hover:text-fg"
          >
            {theme === "paper" ? <Moon className="h-3.5 w-3.5" /> : <Sun className="h-3.5 w-3.5" />}
          </button>
        </div>
      </header>

      {/* Body — one decision per screen */}
      <main className="flex min-h-0 flex-1 flex-col items-center overflow-y-auto px-8 pb-6 pt-2">
        {step === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
            <EmptyState variant="empty" caption="Everything, already in its place" />
            <h1 className="pt-2 text-[20px] font-semibold tracking-tight text-fg">
              A precision index of your machine.
            </h1>
            <p className="max-w-sm text-[13px] leading-relaxed text-muted">
              Pick a few folders. Kensaku walks them once — every file, every line inside,
              searchable in milliseconds. Nothing ever leaves this device.
            </p>
          </div>
        )}

        {step === 1 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                What should we index?
              </h2>
              <p className="text-[12px] text-muted">
                Suggested starting points — toggle freely, add a custom path below.
              </p>
            </div>
            <div className="grid grid-cols-2 gap-2">
              {suggestedDirs.map((dir) => {
                const selected = selectedDirs.includes(dir);
                const name = dir.split("/").filter(Boolean).pop() || dir;
                return (
                  <button
                    key={dir}
                    type="button"
                    onClick={() => toggleDir(dir)}
                    className={`flex h-14 items-center gap-2.5 rounded-[14px] border px-3 text-left transition-colors duration-150 ${
                      selected
                        ? "border-accent bg-accent-soft"
                        : "bg-elevated hover:border-line-strong"
                    }`}
                  >
                    <span
                      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                        selected ? "border-accent bg-accent text-accent-fg" : "border-line-strong"
                      }`}
                    >
                      {selected && <Check className="h-2.5 w-2.5" />}
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate text-[12px] font-medium text-fg">{name}</span>
                      <span className="block truncate font-mono text-[10px] text-subtle">{dir}</span>
                    </span>
                  </button>
                );
              })}
            </div>
            <div className="flex gap-1.5">
              <input
                type="text"
                value={customDirInput}
                placeholder="Or a custom path, e.g. /workspace"
                onChange={(e) => setCustomDirInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addCustomDir()}
                spellCheck={false}
                className="h-9 min-w-0 flex-1 rounded-[10px] border border-line bg-elevated px-3 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
              />
              <button
                type="button"
                onClick={addCustomDir}
                className="flex h-9 items-center gap-1 rounded-[10px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong"
              >
                <Plus className="h-3.5 w-3.5" /> Add
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                What should we skip?
              </h2>
              <p className="text-[12px] text-muted">
                Ignored names keep the index lean and every query instant. A preset is a good default.
              </p>
            </div>
            <div className="flex gap-1.5">
              {Object.keys(PRESETS).map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => setExcludedDirs(PRESETS[p])}
                  className="flex h-7 items-center rounded-full border border-line px-3 text-[11px] text-muted transition-colors hover:border-line-strong hover:text-fg"
                >
                  {p} preset
                </button>
              ))}
            </div>
            <div className="flex max-h-28 flex-wrap gap-1.5 overflow-y-auto rounded-[14px] border border-line bg-elevated p-3">
              {excludedDirs.map((d) => (
                <button
                  key={d}
                  type="button"
                  title={`Remove ${d}`}
                  onClick={() => setExcludedDirs((prev) => prev.filter((x) => x !== d))}
                  className="group flex h-6 items-center gap-1.5 rounded-full bg-sunken px-2.5 font-mono text-[10px] text-muted transition-colors hover:text-danger"
                >
                  {d}
                  <X className="h-3 w-3 opacity-50 group-hover:opacity-100" />
                </button>
              ))}
            </div>
            <input
              type="text"
              value={customExcludeInput}
              placeholder="Add an ignored name — Enter"
              onChange={(e) => setCustomExcludeInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && customExcludeInput.trim()) {
                  const t = customExcludeInput.trim();
                  setExcludedDirs((prev) => (prev.includes(t) ? prev : [...prev, t]));
                  setCustomExcludeInput("");
                }
              }}
              spellCheck={false}
              className="h-9 w-full rounded-[10px] border border-line bg-elevated px-3 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
            />
          </div>
        )}
        {step === 3 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                Preferences
              </h2>
              <p className="text-[12px] text-muted">
                Theme and global shortcut. Everything else stays in Settings.
              </p>
            </div>
            <div className="flex items-center justify-between rounded-[14px] border border-line bg-elevated px-4 py-3">
              <div>
                <span className="block text-[12px] font-medium text-fg">Theme</span>
                <span className="block text-[11px] text-muted">paper or void</span>
              </div>
              <button
                type="button"
                onClick={() => setTheme(theme === "paper" ? "void" : "paper")}
                className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line px-3 text-[12px] text-fg transition-colors hover:border-line-strong"
              >
                {theme === "paper" ? <Moon className="h-3.5 w-3.5" /> : <Sun className="h-3.5 w-3.5" />}
                {theme}
              </button>
            </div>
            <div className="rounded-[14px] border border-line bg-elevated px-4 py-3">
              <span className="block text-[12px] font-medium text-fg">Global shortcut</span>
              <span className="block text-[11px] text-muted">Toggles the window anywhere.</span>
              <button
                type="button"
                onClick={() => setCapturing(true)}
                className={`mt-2 flex h-9 min-w-40 items-center justify-center rounded-[10px] border px-3 font-mono text-[11px] transition-colors ${
                  capturing
                    ? "border-accent text-accent"
                    : "border-line bg-bg text-fg hover:border-line-strong"
                }`}
              >
                {capturing ? "press keys… (Esc to cancel)" : shortcut}
              </button>
            </div>
          </div>
        )}

        {step === 4 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                Optional packs
              </h2>
              <p className="text-[12px] text-muted">
                Downloads are optional. Skip any you don't need — keyword search
                works without them.
              </p>
            </div>
            <div className="space-y-2">
              <div className="rounded-[14px] border border-line bg-elevated px-4 py-3">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <span className="block text-[12px] font-medium text-fg">OCR language data</span>
                    <span className="block text-[11px] text-muted">English tessdata, ~4 MB</span>
                  </div>
                  {ocrStatus?.available ? (
                    <span className="flex h-8 items-center gap-1.5 rounded-[10px] px-2 text-[11px] text-success">
                      <Check className="h-3.5 w-3.5" /> Ready
                    </span>
                  ) : (
                    <button
                      type="button"
                      disabled={ocrBusy}
                      onClick={async () => {
                        setPackError(null);
                        setOcrBusy(true);
                        try {
                          const status = await invoke<OcrStatus>("download_ocr_language");
                          setOcrStatus(status);
                        } catch (e) {
                          setPackError(`OCR download failed: ${String(e)}`);
                        } finally {
                          setOcrBusy(false);
                        }
                      }}
                      className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line px-3 text-[12px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                    >
                      {ocrBusy ? (
                        <>
                          <LoaderCircle className="h-3.5 w-3.5 animate-spin" /> Downloading…
                        </>
                      ) : (
                        <>
                          <Download className="h-3.5 w-3.5" /> Download
                        </>
                      )}
                    </button>
                  )}
                </div>
                {ocrStatus && !ocrStatus.available && !ocrBusy && (
                  <p className="pt-1.5 text-[11px] text-muted">
                    {ocrStatus.engine_present
                      ? "Tesseract found; language data is still missing."
                      : "Tesseract is not installed; language data alone will not enable OCR."}
                  </p>
                )}
              </div>

              <div className="rounded-[14px] border border-line bg-elevated px-4 py-3">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <span className="block text-[12px] font-medium text-fg">Semantic model</span>
                    <span className="block text-[11px] text-muted">
                      Optional neural tier for better recall
                    </span>
                  </div>
                  {modelsStatus?.ready ? (
                    <span className="flex h-8 items-center gap-1.5 rounded-[10px] px-2 text-[11px] text-success">
                      <Check className="h-3.5 w-3.5" /> Ready
                    </span>
                  ) : (
                    <button
                      type="button"
                      disabled={modelsStatus?.downloading}
                      onClick={async () => {
                        setPackError(null);
                        try {
                          await invoke("download_models");
                          const status = await invoke<ModelsStatus>("get_models_status");
                          setModelsStatus(status);
                        } catch (e) {
                          setPackError(`Model download failed: ${String(e)}`);
                        }
                      }}
                      className="flex h-8 shrink-0 items-center gap-1.5 rounded-[10px] border border-line px-3 text-[12px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                    >
                      {modelsStatus?.downloading ? (
                        <>
                          <LoaderCircle className="h-3.5 w-3.5 animate-spin" /> Downloading…
                        </>
                      ) : (
                        <>
                          <Download className="h-3.5 w-3.5" /> Download
                        </>
                      )}
                    </button>
                  )}
                </div>
                {modelsStatus?.downloading && modelsStatus.progress && (
                  <div className="space-y-1 pt-2">
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken">
                      <div
                        className="h-full rounded-full bg-accent transition-[width] duration-300"
                        style={{
                          width: `${Math.max(
                            4,
                            modelsStatus.progress.total > 0
                              ? Math.round(
                                  (modelsStatus.progress.downloaded / modelsStatus.progress.total) * 100,
                                )
                              : 0,
                          )}%`,
                        }}
                      />
                    </div>
                    <div className="flex items-center justify-between gap-2 text-[10px] text-subtle">
                      <code className="min-w-0 truncate font-mono">
                        {modelsStatus.progress.file || "downloading…"}
                      </code>
                      <span className="shrink-0 font-mono">
                        {(modelsStatus.progress.downloaded / 1048576).toFixed(1)} /{" "}
                        {(modelsStatus.progress.total / 1048576).toFixed(1)} MB
                      </span>
                    </div>
                  </div>
                )}
                {modelsStatus?.error && !modelsStatus.downloading && (
                  <p className="pt-1.5 text-[11px] text-danger">{modelsStatus.error}</p>
                )}
              </div>
            </div>

            {packError && (
              <p className="break-words rounded-[10px] border border-danger/30 bg-danger/5 px-3 py-2 font-mono text-[10px] text-danger">
                {packError}
              </p>
            )}
          </div>
        )}

      </main>

      {/* Footer: step nav */}
      <footer className="flex h-16 shrink-0 items-center justify-between border-t border-line px-5">
        <div className="min-w-0 text-[11px] text-subtle">
          {step === 1 && `${selectedDirs.length} folder${selectedDirs.length === 1 ? "" : "s"} selected`}
          {step === 2 && `${excludedDirs.length} ignored names`}
          {step === 3 && (
            <span className="font-mono">
              shortcut <kbd className="kbd">{shortcut}</kbd> toggles the window anywhere
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {step > 0 && (
            <button
              type="button"
              onClick={() => setStep((s) => (s - 1) as 0 | 1 | 2 | 3 | 4)}
              className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line px-3 text-[12px] text-muted transition-colors hover:border-line-strong hover:text-fg"
            >
              <ArrowLeft className="h-3.5 w-3.5" /> Back
            </button>
          )}
          {step < 4 ? (
            <button
              type="button"
              onClick={() => setStep((s) => (s + 1) as 0 | 1 | 2 | 3 | 4)}
              disabled={step === 1 && selectedDirs.length === 0}
              className="flex h-8 items-center gap-1.5 rounded-[10px] bg-accent px-4 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              Continue <ArrowRight className="h-3.5 w-3.5" />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              disabled={isSubmitting || selectedDirs.length === 0}
              className="flex h-8 items-center gap-2 rounded-[10px] bg-accent px-4 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              {isSubmitting && <LoaderCircle className="h-3.5 w-3.5 animate-spin" />}
              Start indexing
              {!isSubmitting && <ArrowRight className="h-3.5 w-3.5" />}
            </button>
          )}
        </div>
      </footer>
    </div>
  );
}
