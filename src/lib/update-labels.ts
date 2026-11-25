import type { UpdateActivity } from "./updater";

/**
 * The status bar renders one short label per update phase and a longer title
 * for the tooltip. Both mappings live here, away from the component, so every
 * phase (including the percent-less download) can be unit tested without
 * mounting React.
 */
export type UpdateLabelKey =
  | "update.available"
  | "update.availableShort"
  | "update.downloading"
  | "update.downloadingUnknown"
  | "update.downloadingTitle"
  | "update.ready"
  | "update.readyTitle"
  | "update.restarting"
  | "update.restartingTitle"
  | "update.deferred"
  | "update.deferredTitle"
  | "update.failed"
  | "update.failedTitle";

export type Translate = (key: UpdateLabelKey, params?: Record<string, string | number>) => string;

export function updateLabel(update: UpdateActivity, t: Translate): string {
  switch (update.phase) {
    case "available":
      return t("update.availableShort", { version: update.version });
    case "downloading":
      return update.percent
        ? t("update.downloading", { percent: update.percent })
        : t("update.downloadingUnknown");
    case "ready":
      return t("update.ready");
    case "restarting":
      return t("update.restarting", { seconds: update.secondsLeft ?? 0 });
    case "deferred":
      return t("update.deferred");
    case "failed":
      return t("update.failed");
  }
}

export function updateTitle(update: UpdateActivity, t: Translate): string {
  switch (update.phase) {
    case "available":
      return t("update.available", { version: update.version });
    case "downloading":
      return t("update.downloadingTitle", { version: update.version });
    case "ready":
      return t("update.readyTitle");
    case "restarting":
      return t("update.restartingTitle", { seconds: update.secondsLeft ?? 0 });
    case "deferred":
      return t("update.deferredTitle");
    case "failed":
      // The backend error is appended when there is one; the label alone would
      // not tell the user what to do about it.
      return update.message
        ? `${t("update.failedTitle")}: ${update.message}`
        : t("update.failedTitle");
  }
}
