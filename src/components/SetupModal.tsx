import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Plus, ArrowRight, ArrowLeft, LoaderCircle, Sun, Moon, Download, ScanText, Keyboard, Sparkles, FileText, Filter, Eye, Copy, Bot, Shield, Pencil } from "lucide-react";
import { useTheme } from "../hooks/useTheme";
import type { IndexingStatus, OcrStatus, ModelsStatus } from "../types";

/* First run: three screens, one decision each.
   1 Welcome → 2 Pick folders → 3 Exclusions + start. */

const DEFAULT_EXCLUSIONS = [
  "node_modules", ".git", "target", "dist", "build", "out",
  ".next", ".nuxt", ".output", ".cache", ".venv", "venv",
  "__pycache__", "vendor", ".gradle", ".idea", ".vscode",
  ".Trash", "lost+found",
];


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
  const [step, setStep] = useState<0 | 1>(0);
  // Start at sign-in is on by default, so the app is one shortcut away after a restart.
  const startAtLogin = true;
  const [suggestedDirs, setSuggestedDirs] = useState<string[]>([]);
  const [selectedDirs, setSelectedDirs] = useState<string[]>([]);
  const [customDirInput, setCustomDirInput] = useState("");
  const excludedDirs = DEFAULT_EXCLUSIONS;
  const [ocrStatus, setOcrStatus] = useState<OcrStatus | null>(null);
  const [ocrPackStatus, setOcrPackStatus] = useState<ModelsStatus | null>(null);
  const [ocrBusy, setOcrBusy] = useState(false);
  const [ocrPackBusy, setOcrPackBusy] = useState(false);
  const [selectedOcrEngine, setSelectedOcrEngine] = useState<"tesseract" | "accurate">("tesseract");
  const [packError, setPackError] = useState<string | null>(null);
  const [shortcut, setShortcut] = useState("Alt+Space");
  const [capturing, setCapturing] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanStatus, setScanStatus] = useState<IndexingStatus | null>(null);
  const [done, setDone] = useState(false);
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

  // Load OCR state when the final screen opens.
  useEffect(() => {
    if (step !== 1) return;
    invoke<OcrStatus>("get_ocr_status").then(setOcrStatus).catch(() => {});
    invoke<ModelsStatus>("get_ocr_pack_status").then(setOcrPackStatus).catch(() => {});
  }, [step]);

  // Poll while an OCR pack download is running.
  useEffect(() => {
    if (step !== 1 || !ocrPackStatus?.downloading) return;
    const id = window.setInterval(() => {
      invoke<ModelsStatus>("get_ocr_pack_status").then(setOcrPackStatus).catch(() => {});
    }, 600);
    return () => window.clearInterval(id);
  }, [step, ocrPackStatus?.downloading]);

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
        ocrEngine: selectedOcrEngine,
      });
      // The default text model downloads without asking. Failure is not fatal:
      // keyword search works without it, and Settings can retry.
      invoke("download_models").catch((err) => console.warn("Model download did not start:", err));
      // Only touch autostart when the user asked for it; off is the default.
      if (startAtLogin) {
        await invoke("set_autostart", { enabled: true }).catch((err) =>
          console.warn("Could not enable start at sign-in:", err),
        );
      }
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
          <h1 className="text-[17px] font-semibold tracking-tight text-fg">Done.</h1>
          <p className="text-[12px] text-muted">Your index is ready. Start searching.</p>
        </div>
        <div className="grid grid-cols-3 gap-6 text-center">
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{files.toLocaleString()}</div>
            <div className="text-[11px] text-muted">files</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{content.toLocaleString()}</div>
            <div className="text-[11px] text-muted">documents</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{units.toLocaleString()}</div>
            <div className="text-[11px] text-muted">embedded</div>
          </div>
        </div>
        <button
          type="button"
          onClick={onSetupCompleted}
          className="flex h-9 items-center gap-1.5 rounded-[8px] bg-accent px-5 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90"
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
          <h1 className="text-[17px] font-semibold tracking-tight text-fg">
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
            <div className="flex items-center justify-between text-[11px] text-subtle">
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
            <div className="text-[11px] text-muted">files</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{content.toLocaleString()}</div>
            <div className="text-[11px] text-muted">documents</div>
          </div>
          <div>
            <div className="font-mono text-[20px] font-semibold text-fg">{units.toLocaleString()}</div>
            <div className="text-[11px] text-muted">embedded</div>
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
            {[0, 1].map((s) => (
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
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[17px] font-semibold tracking-tight text-fg">
                What should we index?
              </h2>
              <p className="text-[12px] text-muted">
                Start with these, or add your own folder.
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
                    className={`flex h-14 items-center gap-2.5 rounded-[12px] border px-3 text-left transition-colors duration-150 ${
                      selected
                        ? "border-fg bg-sunken"
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
                      <span className="block truncate font-mono text-[11px] text-subtle">{dir}</span>
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
                className="h-9 min-w-0 flex-1 rounded-[8px] border border-line bg-elevated px-3 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
              />
              <button
                type="button"
                onClick={addCustomDir}
                className="flex h-9 items-center gap-1 rounded-[8px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong"
              >
                <Plus className="h-3.5 w-3.5" /> Add
              </button>
            </div>
          </div>
        )}

        {step === 1 && (
          <div className="w-full max-w-2xl space-y-6">
            {/* Read Images (OCR) */}
            <section className="space-y-3">
              <div className="flex items-center gap-2">
                <ScanText className="h-4 w-4 text-muted" />
                <div>
                  <h2 className="text-[15px] font-semibold tracking-tight text-fg">Read images (OCR)</h2>
                  <p className="text-[11px] text-muted">Optional, but recommended. Start stays disabled until one engine is ready.</p>
                </div>
              </div>
              <div className="space-y-2">
                {/* Tesseract */}
                <button
                  type="button"
                  onClick={() => setSelectedOcrEngine("tesseract")}
                  className={`flex w-full items-center justify-between rounded-[12px] border px-4 py-3 text-left transition-colors ${
                    selectedOcrEngine === "tesseract" ? "border-accent bg-accent/5" : "border-line bg-elevated hover:border-line-strong"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <span className={`flex h-4 w-4 items-center justify-center rounded-full border ${selectedOcrEngine === "tesseract" ? "border-accent bg-accent text-accent-fg" : "border-line-strong"}`}>
                      {selectedOcrEngine === "tesseract" && <span className="h-1.5 w-1.5 rounded-full bg-accent-fg" />}
                    </span>
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-[13px] font-medium text-fg">Tesseract</span>
                      </div>
                      <span className="text-[11px] text-muted">Fast · smaller · default</span>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {ocrStatus?.available ? (
                      <>
                        <span className="h-2 w-2 rounded-full bg-success" />
                        <span className="text-[11px] text-success">Ready</span>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={ocrBusy || !ocrStatus?.engine_present}
                        onClick={async (e) => {
                          e.stopPropagation();
                          setPackError(null);
                          setOcrBusy(true);
                          try {
                            const status = await invoke<OcrStatus>("download_ocr_language");
                            setOcrStatus(status);
                          } catch (err) {
                            setPackError(`OCR download failed: ${String(err)}`);
                          } finally {
                            setOcrBusy(false);
                          }
                        }}
                        className="flex h-7 items-center gap-1.5 rounded-[8px] border border-line px-2.5 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                      >
                        {ocrBusy ? <LoaderCircle className="h-3 w-3 animate-spin" /> : <Download className="h-3 w-3" />}
                        {ocrBusy ? "Downloading…" : "Download"}
                      </button>
                    )}
                  </div>
                </button>

                {/* PP-OCRv6 */}
                <button
                  type="button"
                  onClick={() => setSelectedOcrEngine("accurate")}
                  className={`flex w-full items-center justify-between rounded-[12px] border px-4 py-3 text-left transition-colors ${
                    selectedOcrEngine === "accurate" ? "border-accent bg-accent/5" : "border-line bg-elevated hover:border-line-strong"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <span className={`flex h-4 w-4 items-center justify-center rounded-full border ${selectedOcrEngine === "accurate" ? "border-accent bg-accent text-accent-fg" : "border-line-strong"}`}>
                      {selectedOcrEngine === "accurate" && <span className="h-1.5 w-1.5 rounded-full bg-accent-fg" />}
                    </span>
                    <div>
                      <span className="text-[13px] font-medium text-fg">PP-OCRv6</span>
                      <div className="text-[11px] text-muted">Slower · may read small UI text better</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {ocrPackStatus?.ready ? (
                      <>
                        <span className="h-2 w-2 rounded-full bg-success" />
                        <span className="text-[11px] text-success">Ready</span>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={ocrPackStatus?.downloading || !ocrStatus?.accurate_compiled}
                        onClick={async (e) => {
                          e.stopPropagation();
                          setPackError(null);
                          setOcrPackBusy(true);
                          try {
                            const status = await invoke<ModelsStatus>("download_ocr_pack");
                            setOcrPackStatus(status);
                          } catch (err) {
                            setPackError(`PP-OCRv6 download failed: ${String(err)}`);
                          } finally {
                            setOcrPackBusy(false);
                          }
                        }}
                        className="flex h-7 items-center gap-1.5 rounded-[8px] border border-line px-2.5 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                      >
                        {ocrPackStatus?.downloading || ocrPackBusy ? <LoaderCircle className="h-3 w-3 animate-spin" /> : <Download className="h-3 w-3" />}
                        {ocrPackStatus?.downloading || ocrPackBusy ? "Downloading…" : "Download"}
                      </button>
                    )}
                  </div>
                </button>

                {packError && (
                  <p className="break-words rounded-[8px] border border-danger/30 bg-danger/5 px-3 py-2 font-mono text-[11px] text-danger">
                    {packError}
                  </p>
                )}
                {!ocrStatus?.engine_present && !ocrStatus?.accurate_compiled && (
                  <p className="text-[11px] text-subtle">
                    Tesseract is not installed and PP-OCRv6 is not compiled into this build.
                  </p>
                )}
              </div>
            </section>

            {/* Global Shortcut */}
            <section className="space-y-3">
              <div className="flex items-center gap-2">
                <Keyboard className="h-4 w-4 text-muted" />
                <div>
                  <h2 className="text-[15px] font-semibold tracking-tight text-fg">Global shortcut</h2>
                  <p className="text-[11px] text-muted">Open Kensaku from anywhere. Esc hides it; the app keeps running in the background.</p>
                  <p className="pt-1 text-[12px] font-semibold text-accent">Open: your shortcut · Close: Esc</p>
                </div>
              </div>
              <div className="flex items-center justify-between rounded-[12px] border border-line bg-elevated px-4 py-3">
                <div className="flex items-center gap-1.5">
                  {shortcut.split("+").map((k, i) => (
                    <kbd key={i} className="rounded-[6px] border border-line px-2 py-1 font-mono text-[11px] text-fg">{k}</kbd>
                  ))}
                </div>
                <button
                  type="button"
                  onClick={() => setCapturing(true)}
                  className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-sunken hover:text-fg"
                  aria-label="Edit shortcut"
                >
                  <Pencil className="h-3.5 w-3.5" />
                </button>
              </div>
              {capturing && (
                <p className="text-[11px] text-muted">Press your new shortcut… (Esc to cancel)</p>
              )}
            </section>

            {/* What Kensaku does */}
            <section className="space-y-3">
              <div className="flex items-center gap-2">
                <Sparkles className="h-4 w-4 text-muted" />
                <div>
                  <h2 className="text-[15px] font-semibold tracking-tight text-fg">What Kensaku does</h2>
                  <p className="text-[11px] text-muted">Search your files, contents and images — all locally.</p>
                </div>
              </div>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <FileText className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">Search everything</p>
                  <p className="text-[11px] text-muted">Find files by name, content, image text (OCR) and meaning.</p>
                </div>
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <Filter className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">Powerful filters</p>
                  <p className="text-[11px] text-muted">Type, extension, folder, date, size, regex. Use / for commands.</p>
                </div>
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <Eye className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">Quick preview</p>
                  <p className="text-[11px] text-muted">Press Space to preview. Enter to open. Shift+Enter to show in folder. Esc clears, then hides.</p>
                </div>
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <Copy className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">Drag and drop</p>
                  <p className="text-[11px] text-muted">Drag a result to copy or move the file to another app.</p>
                </div>
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <Bot className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">AI agent ready</p>
                  <p className="text-[11px] text-muted">Works with AI agents over MCP (read-only by default).</p>
                </div>
                <div className="rounded-[10px] border border-line bg-elevated p-3">
                  <Shield className="h-4 w-4 text-muted" />
                  <p className="pt-1.5 text-[12px] font-medium text-fg">Local and private</p>
                  <p className="text-[11px] text-muted">Nothing leaves your machine. Index stays local.</p>
                </div>
              </div>
            </section>
          </div>
        )}
      </main>

      {/* Footer: step nav */}
      <footer className="flex h-16 shrink-0 items-center justify-between border-t border-line px-5">
        <div className="min-w-0 text-[11px] text-subtle">
          {step === 1 && `${selectedDirs.length} folder${selectedDirs.length === 1 ? "" : "s"} selected`}
          {step === 1 && !(ocrStatus?.available || ocrPackStatus?.ready) && (
            <span className="ml-2 text-warning">· download an OCR engine to enable Start</span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {step > 0 && (
            <button
              type="button"
              onClick={() => setStep((s) => (s - 1) as 0 | 1)}
              className="flex h-9 items-center gap-1.5 rounded-[8px] px-3 text-[13px] text-muted transition-colors hover:text-fg"
            >
              <ArrowLeft className="h-3.5 w-3.5" /> Back
            </button>
          )}
          {step < 1 ? (
            <button
              type="button"
              onClick={() => setStep((s) => (s + 1) as 0 | 1)}
              disabled={step === 1 && selectedDirs.length === 0}
              className="flex h-9 items-center gap-1.5 rounded-[8px] bg-fg px-5 text-[13px] font-medium text-bg transition-opacity hover:opacity-85 disabled:opacity-40"
            >
              Continue <ArrowRight className="h-3.5 w-3.5" />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              disabled={
                isSubmitting ||
                selectedDirs.length === 0 ||
                !(ocrStatus?.available || ocrPackStatus?.ready)
              }
              className="flex h-9 items-center gap-2 rounded-[8px] bg-fg px-5 text-[13px] font-medium text-bg transition-opacity hover:opacity-85 disabled:opacity-40"
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
