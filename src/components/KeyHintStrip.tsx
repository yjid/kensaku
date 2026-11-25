import React from "react";
import type { IndexingStatus, DoctorReport } from "../types";
import type { UpdateActivity } from "../lib/updater";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { formatComma } from "../lib/format";
import { updateLabel, updateTitle, type Translate } from "../lib/update-labels";
import { AlertTriangle, ArrowUpCircle, Clock3, Download, RefreshCw, Stethoscope } from "lucide-react";

/* Status line. Index health on the left, the few keys worth knowing on the
   right. Quiet by default: it only takes the ink while indexing. */

const HINTS: {
  keys: string;
  labelKey: "actions.open" | "actions.preview" | "actions.reveal" | "actions.details";
}[] = [
  { keys: "↵", labelKey: "actions.open" },
  { keys: "Space", labelKey: "actions.preview" },
  { keys: "⇧↵", labelKey: "actions.reveal" },
  { keys: "⌘I", labelKey: "actions.details" },
];

function statusText(
  s: IndexingStatus | null | undefined,
  t: TFunction<"common">,
): { text: string; busy: boolean } {
  if (!s) return { text: "", busy: false };
  if (s.paused) return { text: t("status.paused"), busy: false };
  if (s.is_indexing || (s.phase && s.phase !== "idle")) {
    if (s.phase === "rebuilding") return { text: t("status.rebuilding"), busy: true };
    /* Background semantic embedding runs after the crawl. Showing it as
       "Indexing N of N" (the finished crawl's counts) looked stuck. */
    if (s.phase === "embedding" && !s.is_indexing)
      return { text: t("status.embedding", { count: formatComma(s.files_indexed) }), busy: true };
    const done = s.extract_done ?? s.files_indexed;
    const total = s.extract_total ?? s.total_files;
    return {
      text: total
        ? t("status.indexingProgress", { done: formatComma(done), total: formatComma(total) })
        : t("status.indexingFiles", { count: formatComma(done) }),
      busy: true,
    };
  }
  return { text: t("status.filesCount", { count: formatComma(s.files_indexed) }), busy: false };
}

/* Compact labels: the strip is 11px tall and shares the row with the index
   status, so every update state has to fit in a few words. The mapping lives in
   lib/update-labels.ts so it can be tested without mounting React. */
function updateIcon(update: UpdateActivity) {
  switch (update.phase) {
    case "downloading":
      return <Download className="h-3.5 w-3.5" />;
    case "restarting":
      return <RefreshCw className="h-3.5 w-3.5 animate-spin" />;
    case "deferred":
      return <Clock3 className="h-3.5 w-3.5" />;
    case "failed":
      return <AlertTriangle className="h-3.5 w-3.5" />;
    default:
      return <ArrowUpCircle className="h-3.5 w-3.5" />;
  }
}

function updateTone(update: UpdateActivity): string {
  if (update.phase === "failed") return "text-warning";
  if (update.phase === "deferred") return "text-subtle";
  return "text-accent";
}

export const KeyHintStrip: React.FC<{
  status?: IndexingStatus | null;
  doctor?: DoctorReport | null;
  update?: UpdateActivity | null;
  onDoctor?: () => void;
  onUpdate?: () => void;
}> = ({ status, doctor, update, onDoctor, onUpdate }) => {
  const { t } = useTranslation("common");
  const tUpdate = t as unknown as Translate;
  const issues = doctor ? doctor.summary.warn + doctor.summary.fail : 0;
  const { text, busy } = statusText(status, t);
  const percent = update?.percent ?? 0;
  return (
    <div className="relative z-10 flex h-7 shrink-0 items-center justify-between gap-4 px-3.5 text-[11px] text-subtle">
      <span className="flex min-w-0 items-center gap-2">
        {text && (
          <>
            <span className={`h-1.5 w-1.5 rounded-full ${busy ? "dot-pulse bg-ink" : "bg-success/70"}`} />
            <span className="t-meta">{text}</span>
          </>
        )}
        {doctor && (
          <button
            type="button"
            onClick={onDoctor}
            title={issues ? t("health.issue", { count: issues }) : t("health.ok")}
            className={`flex items-center gap-1 rounded-full px-1.5 py-0.5 transition-colors hover:bg-surface-2 ${issues ? "text-warning" : "text-subtle"}`}
          >
            <Stethoscope className="h-3.5 w-3.5" />
            {issues > 0 && <span className="t-meta">{t("health.issuesShort", { count: issues })}</span>}
          </button>
        )}
        {update && (
          <button
            type="button"
            onClick={onUpdate}
            disabled={update.phase === "restarting"}
            title={updateTitle(update, tUpdate)}
            aria-label={updateTitle(update, tUpdate)}
            className={`flex shrink-0 items-center gap-1 rounded-full px-1.5 py-0.5 transition-colors enabled:hover:bg-surface-2 disabled:cursor-default ${updateTone(update)}`}
          >
            {updateIcon(update)}
            <span
              className="t-meta whitespace-nowrap"
              aria-live={update.phase === "restarting" ? "off" : "polite"}
            >
              {updateLabel(update, tUpdate)}
            </span>
            {update.phase === "downloading" &&
              (percent > 0 ? (
                <span className="h-[3px] w-10 overflow-hidden rounded-full bg-surface-2" aria-hidden="true">
                  <span
                    className="block h-full rounded-full bg-accent transition-[width] duration-300 ease-out"
                    style={{ width: `${percent}%` }}
                  />
                </span>
              ) : (
                <span className="h-1.5 w-1.5 rounded-full bg-accent dot-pulse" aria-hidden="true" />
              ))}
          </button>
        )}
      </span>
      <span className="hidden items-center gap-4 sm:flex">
        {HINTS.map((h) => (
          <span key={h.labelKey} className="flex items-center gap-1.5">
            <span className="kbd">{h.keys}</span>
            <span>{t(h.labelKey)}</span>
          </span>
        ))}
      </span>
    </div>
  );
};
