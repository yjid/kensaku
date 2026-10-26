import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronDown,
  Copy,
  Download,
  RefreshCw,
  TriangleAlert,
} from "lucide-react";
import { KensakuConfig, OcrStatus } from "../../types";
import { Switch, reasonFrom } from "./kit";

/* OCR row — state-aware. Calls `get_ocr_status` on mount; every outcome
   is honest: when OCR cannot work the switch is disabled next to the
   reason, never left looking functional. The hint line height is always
   reserved so resolving the status never shifts the row. */

const OCR_DESCRIPTION =
  "Read text from screenshots, scans and photos. Existing images are re-scanned after saving.";

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

type DownloadState = "idle" | "running" | "failed";

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
  const [rechecking, setRechecking] = useState(false);
  const [download, setDownload] = useState<DownloadState>("idle");
  const [downloadError, setDownloadError] = useState<string | null>(null);
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
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  const recheck = async () => {
    setRechecking(true);
    await check();
    setRechecking(false);
  };

  const downloadEnglish = async () => {
    if (download === "running") return;
    setDownload("running");
    setDownloadError(null);
    try {
      /* The backend resolves with a refreshed status; fall back to an
         explicit re-check if it returns anything else. */
      const refreshed = await invoke<unknown>("download_ocr_language", { language: "eng" });
      if (
        refreshed &&
        typeof refreshed === "object" &&
        "engine_present" in refreshed &&
        "languages" in refreshed
      ) {
        setStatus(refreshed as OcrStatus);
      } else {
        await check();
      }
    } catch (err) {
      setDownloadError(reasonFrom(err));
      setDownload("failed");
    } finally {
      setDownload("idle");
    }
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
        /* Engine present, no language data — one small download unlocks OCR. */
        <div className="space-y-2.5 bg-sunken px-4 py-3">
          {status?.error && (
            <p className="break-words font-mono text-[10px] leading-4 text-muted">
              {status.error}
            </p>
          )}
          <div>
            <button
              type="button"
              disabled={download === "running"}
              onClick={downloadEnglish}
              className="flex h-8 items-center gap-1.5 rounded-[10px] bg-accent px-3.5 text-[11px] font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Download className="h-3.5 w-3.5" />
              {download === "running" ? "Downloading…" : "Download English · 4 MB"}
            </button>
          </div>
          {download === "running" && (
            <div className="progress-track w-[220px] max-w-full" aria-hidden="true">
              <div className="progress-bar" />
            </div>
          )}
          {downloadError && (
            <div className="flex items-center justify-between gap-3">
              <p className="min-w-0 break-words font-mono text-[10px] leading-4 text-danger">
                {downloadError}
              </p>
              <button
                type="button"
                onClick={downloadEnglish}
                className="h-8 shrink-0 rounded-[10px] border border-danger/40 px-3 text-[11px] font-medium text-danger transition-colors duration-150 hover:bg-danger/10"
              >
                Retry
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
