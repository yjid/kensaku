import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, Download, Trash2 } from "lucide-react";
import { ModelsStatus } from "../../types";
import { formatBytes } from "../../lib/format";
import { Card, Field } from "./kit";

/* Model files — real on-disk artifacts behind the optional neural tier.
   Loads `get_models_status` on mount and polls every 600 ms only while a
   download is running. Every command is defensive: the backend may not
   ship it yet, so the card degrades to a calm unavailable state. */

function mb(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ModelFilesCard() {
  const [status, setStatus] = useState<ModelsStatus | null>(null);
  const [live, setLive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setStatus(await invoke<ModelsStatus>("get_models_status"));
      setLive(true);
    } catch (err) {
      console.warn("get_models_status not available:", err);
      setLive(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // 600 ms heartbeat strictly while a download is in flight.
  useEffect(() => {
    if (!live || !status?.downloading) return;
    const id = window.setInterval(load, 600);
    return () => window.clearInterval(id);
  }, [live, status?.downloading, load]);

  const act = useCallback(
    async (cmd: string) => {
      setBusy(true);
      setActionError(null);
      try {
        await invoke(cmd);
        await load();
      } catch (err) {
        console.warn(`${cmd} failed:`, err);
        setActionError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [load],
  );

  const copyDir = useCallback(async () => {
    if (!status?.dir) return;
    try {
      await navigator.clipboard.writeText(status.dir);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("Copy failed:", err);
    }
  }, [status?.dir]);

  // Defensive totals: backend fields win, artifact sums fill the gaps.
  const artifacts = status?.artifacts ?? [];
  const expected =
    status?.total_bytes || artifacts.reduce((n, a) => n + (a.bytes_expected || 0), 0);
  const present =
    status?.total_present_bytes ||
    artifacts.reduce((n, a) => n + (a.present ? a.bytes_on_disk || a.bytes_expected || 0 : 0), 0);

  const prog = status?.progress;
  const pct =
    prog && prog.total > 0 ? Math.min(100, Math.round((prog.downloaded / prog.total) * 100)) : 0;

  return (
    <Card title="Model files">
      {/* Directory */}
      {live && status?.dir && (
        <Field label="Directory" description="Where the model files live on disk.">
          <div className="flex min-w-0 items-center gap-1">
            <code className="max-w-56 truncate font-mono text-[11px] text-muted">{status.dir}</code>
            <button
              type="button"
              title="Copy path"
              onClick={copyDir}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded text-subtle transition-colors hover:bg-sunken hover:text-fg"
            >
              {copied ? <Check className="h-3 w-3 text-accent" /> : <Copy className="h-3 w-3" />}
            </button>
          </div>
        </Field>
      )}

      {/* Totals */}
      {live && (
        <Field label="On disk" description="Bytes of the neural tier present / expected.">
          <span className="font-mono text-[11px] text-fg">
            {formatBytes(present)} / {formatBytes(expected)}
          </span>
        </Field>
      )}

      {/* Artifacts */}
      {live &&
        artifacts.map((a) => (
          <div
            key={a.name + a.url}
            className="flex items-center justify-between gap-3 border-b border-line px-4 py-2.5 last:border-b-0"
          >
            <div className="flex min-w-0 items-center gap-2">
              <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${a.present ? "bg-success" : "bg-subtle"}`} />
              <code className="truncate font-mono text-[11px] text-fg">{a.name}</code>
            </div>
            <span className="shrink-0 font-mono text-[10px] text-subtle">
              {formatBytes(a.present ? a.bytes_on_disk || a.bytes_expected : a.bytes_expected)}
            </span>
          </div>
        ))}

      {/* State + actions */}
      {!live ? (
        <p className="px-4 py-3.5 text-[11px] leading-relaxed text-muted">
          Model status unavailable — the backend command is not connected yet.
        </p>
      ) : status?.downloading ? (
        <div className="space-y-2 border-t border-line px-4 py-3">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken">
            <div
              className="h-full rounded-full bg-accent transition-[width] duration-300"
              style={{ width: `${Math.max(4, pct)}%` }}
            />
          </div>
          <div className="flex items-center justify-between gap-3 text-[10px]">
            <code className="min-w-0 truncate font-mono text-muted">{prog?.file ?? "downloading…"}</code>
            <span className="shrink-0 font-mono text-fg">
              {mb(prog?.downloaded ?? 0)} / {mb(prog?.total || expected)} · {pct}%
            </span>
          </div>
          <button
            type="button"
            disabled
            className="flex h-7 items-center gap-1.5 rounded-[10px] border border-line px-2.5 text-[11px] text-subtle"
          >
            <Download className="h-3 w-3" />
            Downloading…
          </button>
        </div>
      ) : status?.error ? (
        <div className="border-t border-line px-4 py-3">
          <p className="break-words text-[11px] text-danger">{status.error}</p>
          <div className="flex items-center gap-1.5 pt-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => act("download_models")}
              className="flex h-7 items-center gap-1.5 rounded-[10px] border border-line px-2.5 text-[11px] text-fg transition-colors hover:border-line-strong disabled:opacity-50"
            >
              <Download className="h-3 w-3" />
              Retry download
            </button>
          </div>
        </div>
      ) : status?.ready ? (
        <div className="flex items-center justify-between gap-4 border-t border-line px-4 py-3">
          <div className="flex items-center gap-1.5">
            <span className="h-1.5 w-1.5 rounded-full bg-success" />
            <p className="text-[12px] font-medium text-fg">Model files ready</p>
          </div>
          <button
            type="button"
            disabled={busy}
            title="Delete the downloaded model files"
            onClick={() => act("delete_models")}
            className="flex h-7 items-center gap-1.5 rounded-[10px] border border-line px-2.5 text-[11px] text-muted transition-colors hover:border-danger/40 hover:text-danger disabled:opacity-50"
          >
            <Trash2 className="h-3 w-3" />
            Remove
          </button>
        </div>
      ) : (
        <div className="border-t border-line px-4 py-3">
          <button
            type="button"
            disabled={busy}
            onClick={() => act("download_models")}
            className="flex h-7 items-center gap-1.5 rounded-[10px] bg-accent px-3 text-[11px] font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            <Download className="h-3 w-3" />
            Download · {formatBytes(expected)}
          </button>
        </div>
      )}

      {busy && <p className="px-4 pb-3 font-mono text-[10px] text-subtle">working…</p>}
      {actionError && (
        <p className="break-words border-t border-line px-4 py-2 font-mono text-[10px] text-danger">
          {actionError}
        </p>
      )}

      {/* Honest note. */}
      <p className="border-t border-line bg-sunken px-4 py-2.5 text-[11px] leading-relaxed text-muted">
        The default semantic tier is built in. These files pre-fetch the optional neural tier for
        offline use.
      </p>
    </Card>
  );
}
