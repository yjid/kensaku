import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronDown,
  Copy,
  RefreshCw,
  TriangleAlert,
} from "lucide-react";
import { KensakuConfig, ModelsStatus, OcrStatus } from "../../types";
import { formatBytes } from "../../lib/format";
import { Switch } from "./kit";

/* OCR row — state-aware. Calls `get_ocr_status` on mount; every outcome
   is honest: when OCR cannot work the switch is disabled next to the
   reason, never left looking functional. The hint line height is always
   reserved so resolving the status never shifts the row. */

const OCR_DESCRIPTION =
  "Read text from screenshots, scans and photos using Tesseract. Existing images are re-scanned after saving.";

const INSTALL_COMMANDS = [
  {
    id: "linux" as const,
    label: "Ubuntu / Debian",
    command: "sudo apt install tesseract-ocr tesseract-ocr-eng",
  },
  {
    id: "arch" as const,
    label: "Arch Linux",
    command: "sudo pacman -S tesseract tesseract-data-eng",
  },
  {
    id: "fedora" as const,
    label: "Fedora",
    command: "sudo dnf install tesseract tesseract-langpack-eng",
  },
  { id: "mac" as const, label: "macOS (Homebrew)", command: "brew install tesseract" },
  {
    id: "windows" as const,
    label: "Windows (winget)",
    command: "winget install UB-Mannheim.TesseractOCR",
  },
];

function pickPlatform(): (typeof INSTALL_COMMANDS)[number] {
  const ua = navigator.userAgent;
  if (/Windows/i.test(ua)) return INSTALL_COMMANDS[4];
  if (/Macintosh|Mac OS/i.test(ua)) return INSTALL_COMMANDS[3];
  /* Linux webviews and anything unmatched land here — the honest default. */
  return INSTALL_COMMANDS[0];
}

export function OcrField({
  config,
  setConfig,
}: {
  config: KensakuConfig;
  setConfig: React.Dispatch<React.SetStateAction<KensakuConfig | null>>;
}) {
  /* null = status unknown (missing command or re-check pending) → the
     switch stays enabled, optimistic, with no hint line. */
  const [status, setStatus] = useState<OcrStatus | null>(null);
  const [pack, setPack] = useState<ModelsStatus | null>(null);
  const [packBusy, setPackBusy] = useState(false);
  const [rechecking, setRechecking] = useState(false);
  const [allPlatforms, setAllPlatforms] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const platform = pickPlatform();

  const check = useCallback(async () => {
    try {
      setStatus(await invoke<OcrStatus>("get_ocr_status"));
    } catch (err) {
      console.warn("get_ocr_status not available:", err);
      setStatus(null);
    }
    try {
      setPack(await invoke<ModelsStatus>("get_ocr_pack_status"));
    } catch {
      setPack(null);
    }
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  const downloadPack = async () => {
    setPackBusy(true);
    try {
      setPack(await invoke<ModelsStatus>("download_ocr_pack"));
    } catch (err) {
      console.warn("download_ocr_pack failed:", err);
    } finally {
      setPackBusy(false);
    }
  };

  const recheck = async () => {
    setRechecking(true);
    await check();
    setRechecking(false);
  };

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(text);
      window.setTimeout(() => setCopied((c) => (c === text ? null : c)), 1500);
    } catch {
      /* clipboard unavailable — the text stays selectable */
    }
  };

  const unavailable = status !== null && !status.available;
  const engineMissing = status !== null && !status.engine_present;
  const noLanguages = !engineMissing && status !== null && !status.available;

  const hint: React.ReactNode | null = engineMissing ? (
    <>
      <TriangleAlert className="h-3 w-3 shrink-0 text-warning" />
      <span className="text-warning">tesseract not found or not runnable.</span>
    </>
  ) : noLanguages ? (
    <>
      <TriangleAlert className="h-3 w-3 shrink-0 text-warning" />
      <span className="text-warning">OCR engine found, but no language data.</span>
    </>
  ) : status?.available ? (
    <span className="font-mono text-muted">
      Engine ready · languages: {status.languages.join(", ")}
    </span>
  ) : null; /* unknown — slot reserved but empty */

  return (
    <div className={unavailable ? "border-b border-line px-4 py-3" : "px-4 py-3"}>
      <div className="flex items-center justify-between gap-6">
        <div className="min-w-0 space-y-0.5">
          <p className="text-[12px] font-medium text-fg">OCR (images)</p>
          <p className="text-[11px] leading-relaxed text-muted">{OCR_DESCRIPTION}</p>
          {/* Reserved line: same height whether or not a hint resolves into it. */}
          <p className="flex min-h-[18px] items-center gap-1.5 text-[11px] leading-[18px]">
            {hint}
          </p>
        </div>
        <Switch
          checked={config.ocr_enabled ?? true}
          disabled={unavailable}
          onChange={(v) => setConfig((c) => (c ? { ...c, ocr_enabled: v } : c))}
        />
      </div>

      {status?.accurate_compiled ? (
        <div className="mt-3 border-t border-line pt-3">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <p className="text-[11px] font-medium text-fg">Accurate OCR pack</p>
              <p className="text-[10px] leading-relaxed text-muted">
                PP-OCRv6 tiny · about 6 MB · higher recall on UI text · uses PP-OCRv6.
              </p>
              <p className="mt-0.5 font-mono text-[10px] text-subtle">
                {pack?.downloading
                  ? `Downloading ${Math.round(
                      ((pack.progress?.downloaded ?? 0) / Math.max(1, pack.progress?.total ?? 1)) * 100,
                    )}%`
                  : pack?.ready
                    ? "Installed"
                    : pack?.error
                      ? pack.error
                      : "Not installed"}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <select
                aria-label="OCR engine"
                value={config.ocr_engine ?? "tesseract"}
                onChange={(e) =>
                  setConfig((c) =>
                    c ? { ...c, ocr_engine: e.target.value as "tesseract" | "accurate" } : c,
                  )
                }
                className="h-8 rounded-[10px] border border-line bg-elevated px-2 text-[11px] text-fg"
              >
                <option value="tesseract">Tesseract</option>
                <option value="accurate" disabled={!pack?.ready}>
                  Accurate
                </option>
              </select>
              {!pack?.ready && (
                <button
                  type="button"
                  disabled={packBusy || pack?.downloading}
                  onClick={downloadPack}
                  className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
                >
                  <RefreshCw
                    className={`h-3 w-3 ${packBusy || pack?.downloading ? "animate-spin" : ""}`}
                  />
                  Download
                </button>
              )}
            </div>
          </div>
          {pack && (
            <div className="mt-3 space-y-1 border-t border-line pt-2">
              <p className="truncate font-mono text-[10px] text-subtle">{pack.dir}</p>
              {pack.artifacts.map((a) => (
                <div key={a.name + a.url} className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <span
                      className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                        a.present ? "bg-success" : "bg-subtle"
                      }`}
                    />
                    <code className="font-mono text-[10px] text-fg">{a.name}</code>
                  </div>
                  <span className="font-mono text-[10px] text-subtle">
                    {formatBytes(a.present ? a.bytes_on_disk || a.bytes_expected : a.bytes_expected)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : (
        <div className="mt-3 border-t border-line pt-3">
          <p className="text-[11px] leading-relaxed text-muted">
            Accurate OCR pack (PP-OCRv6) is not included in this build.
          </p>
        </div>
      )}

      {!unavailable ? null : engineMissing ? (
        /* tesseract absent — show the install command for this platform,
           keep the rest one chevron away, let the user re-check. */
        <div className="space-y-2 bg-sunken px-4 py-3">
          {status?.error && (
            <p className="break-words font-mono text-[10px] leading-4 text-muted">
              {status.error}
            </p>
          )}
          <div className="flex items-center justify-between gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-2">
              <span className="shrink-0 font-mono text-[10px] text-subtle">
                {platform.label}
              </span>
              <code className="no-scrollbar min-w-0 flex-1 select-text overflow-x-auto whitespace-nowrap font-mono text-[11px] leading-5 text-fg">
                {platform.command}
              </code>
            </div>
            <button
              type="button"
              title={`Copy install command — ${platform.command}`}
              onClick={() => copy(platform.command)}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] text-muted transition-colors hover:bg-elevated hover:text-fg"
            >
              {copied === platform.command ? (
                <Check className="h-3.5 w-3.5 text-accent" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
            </button>
          </div>

          {allPlatforms && (
            <div className="space-y-0.5 rounded-[10px] border border-line bg-elevated p-1.5">
              {INSTALL_COMMANDS.map((p) => (
                <div key={p.id} className="flex min-w-0 items-center gap-2 px-1.5 py-1">
                  <span className="w-32 shrink-0 truncate text-[10px] text-subtle">
                    {p.label}
                  </span>
                  <code className="no-scrollbar min-w-0 flex-1 select-text overflow-x-auto whitespace-nowrap font-mono text-[11px] leading-4 text-fg">
                    {p.command}
                  </code>
                  <button
                    type="button"
                    title={`Copy install command — ${p.command}`}
                    onClick={() => copy(p.command)}
                    className="flex h-7 w-7 shrink-0 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-fg"
                  >
                    {copied === p.command ? (
                      <Check className="h-3 w-3 text-accent" />
                    ) : (
                      <Copy className="h-3 w-3" />
                    )}
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="flex items-center justify-between gap-2 pt-0.5">
            <button
              type="button"
              aria-expanded={allPlatforms}
              onClick={() => setAllPlatforms((v) => !v)}
              className="flex h-7 items-center gap-1 pl-1 pr-2 text-[10px] text-muted transition-colors hover:text-fg"
            >
              <ChevronDown
                className={`h-3 w-3 transition-transform duration-150 ${
                  allPlatforms ? "rotate-180" : ""
                }`}
              />
              Other platforms
            </button>
            <button
              type="button"
              disabled={rechecking}
              onClick={recheck}
              className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
            >
              <RefreshCw
                className={`h-3 w-3 ${rechecking ? "animate-spin" : ""}`}
                aria-hidden={rechecking}
              />
              Re-check
            </button>
          </div>
        </div>
      ) : (
        /* Engine present, no language data — setup owns the download. */
        <div className="space-y-2.5 bg-sunken px-4 py-3">
          {status?.error && (
            <p className="break-words font-mono text-[10px] leading-4 text-muted">
              {status.error}
            </p>
          )}
          <p className="text-[11px] leading-relaxed text-muted">
            Language data is downloaded during setup. Use <b>About → Run setup again</b> to
            acquire it, then re-check here.
          </p>
          <button
            type="button"
            disabled={rechecking}
            onClick={recheck}
            className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
          >
            <RefreshCw className={`h-3 w-3 ${rechecking ? "animate-spin" : ""}`} aria-hidden={rechecking} />
            Re-check
          </button>
        </div>
      )}
    </div>
  );
}
