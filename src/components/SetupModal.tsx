import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import { Check, Plus, ArrowRight, ArrowLeft, LoaderCircle, Sun, Moon, Download, Sparkles, X } from "lucide-react";
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
  const { theme, toggle } = useTheme();

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

  const themeButton = (
    <button type="button" title="Theme" onClick={toggle} className="btn btn-ghost btn-icon h-8 !rounded-full">
      {theme === "light" ? <Moon className="h-4 w-4" strokeWidth={1.75} /> : theme === "dark" ? <Sparkles className="h-4 w-4" strokeWidth={1.75} /> : <Sun className="h-4 w-4" strokeWidth={1.75} />}
    </button>
  );

  if (done) {
    const files = scanStatus?.files_indexed ?? 0;
    const content = scanStatus?.content_indexed ?? 0;
    return (
      <Shell mark="Ready" trailing={themeButton} brand={<BrandMoment word="見つけた" gloss="found it." />}>
        <div className="flex flex-1 flex-col justify-center gap-10">
          <div className="space-y-3">
            <h1 className="t-title rise-in text-fg">Your files are ready.</h1>
            <p className="t-lead rise-in max-w-sm text-muted" style={{ ["--stagger-delay" as string]: "60ms" }}>
              Press <span className="kbd align-middle">{shortcut}</span> from anywhere to search.
            </p>
          </div>
          <dl className="rise-in grid grid-cols-2 gap-8" style={{ ["--stagger-delay" as string]: "120ms" }}>
            <Stat value={files} label="Files" />
            <Stat value={content} label="Read inside" />
          </dl>
        </div>
        <Footer>
          <span />
          <button type="button" onClick={onSetupCompleted} className="btn btn-primary h-10 px-5">
            Start searching <ArrowRight className="h-4 w-4" strokeWidth={1.75} />
          </button>
        </Footer>
      </Shell>
    );
  }

  if (scanning) {
    const st = scanStatus;
    const files = st?.files_indexed ?? 0;
    const phase = st?.phase ?? "walking";
    const extractTotal = st?.extract_total ?? 0;
    const extracting = phase === "extracting" && extractTotal > 0;
    const doneCount = extracting ? (st?.extract_done ?? 0) : files;
    const total = extracting ? extractTotal : (st?.total_files ?? 0);
    let eta = "";
    let pct = 0;
    if (total > 0) {
      pct = Math.min(100, Math.round((doneCount / total) * 100));
      if (etaRef.current.phase !== phase || etaRef.current.total !== total) {
        etaRef.current = { phase, done: 0, at: Date.now(), total };
      }
      const elapsed = Math.max(0, (Date.now() - etaRef.current.at) / 1000);
      if (doneCount > 0 && total > doneCount && elapsed >= 1) {
        eta = formatEta((elapsed * (total - doneCount)) / doneCount);
      }
    }
    const phaseLabel: Record<string, string> = {
      walking: "Finding files",
      extracting: "Reading inside documents",
      embedding: "Learning what things mean",
      committing: "Saving the index",
      paused: "Paused",
      idle: "Finishing",
    };

    return (
      <Shell mark="Indexing" trailing={themeButton} brand={<BrandMoment word="検索" gloss="to search." busy />}>
        <div className="flex flex-1 flex-col justify-center gap-8">
          <div className="space-y-2">
            <p className="t-meta text-subtle">{phaseLabel[phase] ?? st?.current_status ?? "Starting"}</p>
            <p className="font-medium tabular-nums text-fg" style={{ fontSize: 64, lineHeight: 1, letterSpacing: "-0.05em" }}>
              {doneCount.toLocaleString("en-US")}
            </p>
            <p className="t-lead text-muted">
              {total > 0 ? `of ${total.toLocaleString("en-US")} files` : "files so far"}
            </p>
          </div>
          <div className="max-w-sm space-y-2">
            <div className="h-px w-full overflow-hidden bg-line">
              <div className="h-full bg-ink transition-[width] duration-500 [transition-timing-function:var(--ease)]" style={{ width: `${Math.max(2, pct)}%` }} />
            </div>
            <div className="t-meta flex justify-between text-subtle">
              <span>{pct}%</span>
              <span>{eta || (pct >= 100 ? "Finishing" : "Estimating")}</span>
            </div>
          </div>
        </div>
        <Footer>
          <p className="text-[12px] text-subtle">Only the first time. After this, every launch is instant.</p>
          <span />
        </Footer>
      </Shell>
    );
  }

  const ocrReady = Boolean(ocrStatus?.available || ocrPackStatus?.ready);

  return (
    <Shell
      mark={`0${step + 1} / 02`}
      trailing={themeButton}
      brand={<BrandMoment word="検索" gloss="to search." />}
    >
      {step === 0 && (
        <div key="s0" className="section-enter flex min-h-0 flex-1 flex-col gap-8 overflow-y-auto pt-2">
          <div className="space-y-2">
            <h1 className="t-title text-fg">Where do your files live?</h1>
            <p className="t-lead text-muted">Pick the folders to search. You can change this later.</p>
          </div>
          <div className="flex flex-col gap-2">
            {[...suggestedDirs, ...selectedDirs.filter((d) => !suggestedDirs.includes(d))].map((dir, i) => {
              const selected = selectedDirs.includes(dir);
              const name = dir.split("/").filter(Boolean).pop() || dir;
              return (
                <button
                  key={dir}
                  type="button"
                  onClick={() => toggleDir(dir)}
                  aria-pressed={selected}
                  style={{ ["--stagger-delay" as string]: `${i * 40}ms` }}
                  className={`surface rise-in group flex h-14 items-center gap-4 px-4 text-left transition-[box-shadow,transform] duration-200 active:scale-[0.99] ${
                    selected ? "![box-shadow:inset_0_0_0_1.5px_var(--ink)]" : "hover:[box-shadow:var(--panel-edge),var(--shadow-float)]"
                  }`}
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[14px] font-medium text-fg">{name}</span>
                    <span className="block truncate text-[12px] text-subtle">{dir.replace(/^\/(Users|home)\/[^/]+/, "~")}</span>
                  </span>
                  <span
                    className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full transition-colors duration-200 ${
                      selected ? "bg-ink text-white" : "shadow-[inset_0_0_0_1.5px_var(--border-strong)]"
                    }`}
                  >
                    {selected && <Check className="h-3 w-3" strokeWidth={2.5} />}
                  </span>
                </button>
              );
            })}
            <div className="flex gap-2 pt-1">
              <input
                type="text"
                value={customDirInput}
                placeholder="Add another folder, e.g. ~/Work"
                onChange={(e) => setCustomDirInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addCustomDir()}
                spellCheck={false}
                className="field h-10 min-w-0 flex-1"
              />
              <button type="button" onClick={addCustomDir} disabled={!customDirInput.trim()} className="btn btn-secondary btn-icon h-10 w-10" aria-label="Add folder">
                <Plus className="h-4 w-4" strokeWidth={1.75} />
              </button>
            </div>
          </div>
        </div>
      )}

      {step === 1 && (
        <div key="s1" className="section-enter flex min-h-0 flex-1 flex-col gap-10 overflow-y-auto pt-2">
          <div className="space-y-2">
            <h1 className="t-title text-fg">Read the text in your images</h1>
            <p className="t-lead text-muted">Screenshots, scans and photos become searchable. Pick an engine.</p>
          </div>

          <div className="flex flex-col gap-2">
            <EngineOption
              selected={selectedOcrEngine === "tesseract"}
              onSelect={() => setSelectedOcrEngine("tesseract")}
              name="Tesseract"
              note="Fast and small. Good for most people."
              ready={Boolean(ocrStatus?.available)}
              busy={ocrBusy}
              disabled={ocrBusy || !ocrStatus?.engine_present}
              onDownload={async () => {
                setPackError(null);
                setOcrBusy(true);
                try {
                  setOcrStatus(await invoke<OcrStatus>("download_ocr_language"));
                } catch (err) {
                  setPackError(`Download failed: ${String(err)}`);
                } finally {
                  setOcrBusy(false);
                }
              }}
            />
            <EngineOption
              selected={selectedOcrEngine === "accurate"}
              onSelect={() => setSelectedOcrEngine("accurate")}
              name="PP-OCRv6"
              note="Slower. Reads small interface text better."
              ready={Boolean(ocrPackStatus?.ready)}
              busy={Boolean(ocrPackStatus?.downloading || ocrPackBusy)}
              disabled={Boolean(ocrPackStatus?.downloading) || !ocrStatus?.accurate_compiled}
              onDownload={async () => {
                setPackError(null);
                setOcrPackBusy(true);
                try {
                  setOcrPackStatus(await invoke<ModelsStatus>("download_ocr_pack"));
                } catch (err) {
                  setPackError(`Download failed: ${String(err)}`);
                } finally {
                  setOcrPackBusy(false);
                }
              }}
            />
            {packError && (
              <p className="flex items-start gap-2 break-words rounded-[10px] bg-danger/10 px-3 py-2 text-[12px] text-danger">
                <X className="mt-0.5 h-3.5 w-3.5 shrink-0" /> {packError}
              </p>
            )}
            {ocrStatus && !ocrStatus.engine_present && !ocrStatus.accurate_compiled && (
              <p className="text-[12px] text-subtle">Tesseract isn't installed and PP-OCRv6 isn't in this build.</p>
            )}
          </div>

          <div className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-[15px] font-semibold tracking-[-0.015em] text-fg">Summon it from anywhere</h2>
              <p className="text-[13px] text-muted">Esc hides it again. Kensaku keeps running quietly.</p>
            </div>
            <button
              type="button"
              onClick={() => setCapturing(true)}
              className={`surface flex h-14 w-full items-center justify-between px-4 text-left transition-shadow ${capturing ? "![box-shadow:inset_0_0_0_1.5px_var(--ink)]" : ""}`}
            >
              <span className="flex items-center gap-1.5">
                {capturing ? (
                  <span className="text-[13px] text-ink">Press a new shortcut…</span>
                ) : (
                  shortcut.split("+").map((k, i) => (
                    <span key={i} className="kbd !h-7 !min-w-7 !rounded-[7px] !px-2 !text-[12px] text-fg">{k}</span>
                  ))
                )}
              </span>
              <span className="text-[12px] text-subtle">{capturing ? "Esc to cancel" : "Change"}</span>
            </button>
            <p className="text-[12px] text-subtle">
              On Linux with Wayland, bind a system shortcut to <code className="font-mono text-[11px]">kensaku --toggle</code> if the key doesn't register.
            </p>
          </div>
        </div>
      )}

      <Footer>
        <p className="text-[12px] text-subtle">
          {step === 0
            ? `${selectedDirs.length} ${selectedDirs.length === 1 ? "folder" : "folders"}`
            : ocrReady
              ? "Everything stays on this computer."
              : "Download an engine to continue."}
        </p>
        <div className="flex items-center gap-2">
          {step > 0 && (
            <button type="button" onClick={() => setStep(0)} className="btn btn-ghost h-10">
              <ArrowLeft className="h-4 w-4" strokeWidth={1.75} /> Back
            </button>
          )}
          {step === 0 ? (
            <button type="button" onClick={() => setStep(1)} disabled={selectedDirs.length === 0} className="btn btn-primary h-10 px-5">
              Continue <ArrowRight className="h-4 w-4" strokeWidth={1.75} />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              disabled={isSubmitting || selectedDirs.length === 0 || !ocrReady}
              className="btn btn-primary h-10 px-5"
            >
              {isSubmitting && <LoaderCircle className="h-4 w-4 animate-spin" />}
              Start indexing
              {!isSubmitting && <ArrowRight className="h-4 w-4" strokeWidth={1.75} />}
            </button>
          )}
        </div>
      </Footer>
    </Shell>
  );
}

/* ---------------- layout pieces ---------------- */

function Shell({
  brand,
  mark,
  trailing,
  children,
}: {
  brand: React.ReactNode;
  mark: string;
  trailing?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="grain absolute inset-0 z-50 flex bg-bg">
      <div className="ambient" aria-hidden="true" />
      <aside data-tauri-drag-region className="relative z-10 hidden w-[42%] min-w-[320px] flex-col justify-between p-10 md:flex">
        <span className="text-[14px] font-semibold tracking-[-0.02em] text-fg">Kensaku</span>
        {brand}
        <span className="t-meta text-subtle">{mark}</span>
      </aside>
      <section className="relative z-10 flex min-w-0 flex-1 flex-col">
        <div className="m-3 flex min-h-0 flex-1 flex-col rounded-[24px] bg-[var(--panel)] px-10 pt-6 shadow-[var(--panel-edge)] backdrop-blur-[var(--panel-blur)] md:ml-0">
          <div data-tauri-drag-region className="flex h-10 shrink-0 items-center justify-end">{trailing}</div>
          <div className="mx-auto flex min-h-0 w-full max-w-[460px] flex-1 flex-col">{children}</div>
        </div>
      </section>
    </div>
  );
}

function BrandMoment({ word, gloss, busy = false }: { word: string; gloss: string; busy?: boolean }) {
  return (
    <div className="space-y-4">
      <p
        className={`rise-in font-medium text-fg ${busy ? "dot-pulse" : ""}`}
        style={{ fontSize: "clamp(72px, 11vw, 132px)", lineHeight: 0.9, letterSpacing: "-0.04em", fontFamily: '"Hiragino Mincho ProN", "Yu Mincho", "Noto Serif CJK JP", serif' }}
      >
        {word}
      </p>
      <p className="t-moment rise-in text-muted" style={{ ["--stagger-delay" as string]: "90ms" }}>
        {gloss}
      </p>
    </div>
  );
}

function Footer({ children }: { children: React.ReactNode }) {
  return <div className="mx-auto flex h-20 w-full max-w-[460px] shrink-0 items-center justify-between gap-4">{children}</div>;
}

function Stat({ value, label }: { value: number; label: string }) {
  return (
    <div className="space-y-1">
      <dd className="font-medium tabular-nums text-fg" style={{ fontSize: 34, lineHeight: 1, letterSpacing: "-0.04em" }}>
        {value.toLocaleString("en-US")}
      </dd>
      <dt className="t-meta text-subtle">{label}</dt>
    </div>
  );
}

function EngineOption({
  selected,
  onSelect,
  name,
  note,
  ready,
  busy,
  disabled,
  onDownload,
}: {
  selected: boolean;
  onSelect: () => void;
  name: string;
  note: string;
  ready: boolean;
  busy: boolean;
  disabled: boolean;
  onDownload: () => void;
}) {
  return (
    <div
      role="radio"
      aria-checked={selected}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect()}
      className={`surface flex min-h-16 cursor-default items-center gap-4 px-4 py-3 transition-shadow duration-200 ${
        selected ? "![box-shadow:inset_0_0_0_1.5px_var(--ink)]" : "hover:[box-shadow:var(--panel-edge),var(--shadow-float)]"
      }`}
    >
      <span className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full transition-colors ${selected ? "bg-ink" : "shadow-[inset_0_0_0_1.5px_var(--border-strong)]"}`}>
        {selected && <span className="h-1.5 w-1.5 rounded-full bg-white" />}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-[14px] font-medium text-fg">{name}</span>
        <span className="block text-[12px] text-muted">{note}</span>
      </span>
      {ready ? (
        <span className="t-meta flex items-center gap-1.5 text-success">
          <span className="h-1.5 w-1.5 rounded-full bg-success" /> Ready
        </span>
      ) : (
        <button
          type="button"
          disabled={disabled}
          onClick={(e) => {
            e.stopPropagation();
            onDownload();
          }}
          className="btn btn-secondary h-8 px-3 text-[12px]"
        >
          {busy ? <LoaderCircle className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" strokeWidth={1.75} />}
          {busy ? "Downloading" : "Download"}
        </button>
      )}
    </div>
  );
}
