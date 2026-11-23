import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n";
import type { UpdateInfo, UpdateStatus } from "../types";

/**
 * The update returned by the last check. The Tauri updater keeps the update
 * resource (and its signature) alive until it is installed, so do not keep
 * the `Update` object in React state.
 */
let pending: Update | null = null;

/** The last check result, so reopening Settings keeps the update card. */
let lastInfo: UpdateInfo | null = null;

/**
 * Cached answer to "can this install update itself?". MSI, portable, and Linux
 * package installs must update through their own package, never through the
 * in-app installer.
 */
let supported: boolean | null = null;

async function inAppUpdatesSupported(): Promise<boolean> {
  if (supported === null) {
    try {
      supported = await invoke<boolean>("updater_supported");
    } catch {
      // Older backends do not expose the command; they also predate the
      // updater, so let the plugin report the real error.
      supported = true;
    }
  }
  return supported;
}

function emptyStatus(): UpdateStatus {
  return {
    busy: false,
    progress: null,
    error: null,
    downloaded_path: null,
    ready_to_install: false,
  };
}

let status: UpdateStatus = emptyStatus();

function snapshot(): UpdateStatus {
  return {
    ...status,
    progress: status.progress ? { ...status.progress } : null,
  };
}

/**
 * Checks the configured updater endpoint for a signed release.
 */
export async function checkForUpdates(): Promise<UpdateInfo> {
  const current = await getVersion();
  const canInstall = await inAppUpdatesSupported();
  if (pending) {
    await pending.close().catch(() => {});
    pending = null;
  }
  const update = await check();
  pending = update;
  status = emptyStatus();

  if (!update) {
    lastInfo = {
      current_version: current,
      latest_version: current,
      has_update: false,
      release_name: `Kensaku ${current}`,
      release_notes: "",
      release_url: "https://github.com/yjid/kensaku/releases/latest",
      published_at: "",
      asset_name: null,
      asset_url: null,
      asset_size: null,
      in_app_supported: canInstall,
    };
    return lastInfo;
  }

  lastInfo = {
    current_version: update.currentVersion,
    latest_version: update.version,
    has_update: true,
    release_name: `Kensaku ${update.version}`,
    release_notes: update.body ?? "",
    release_url: "https://github.com/yjid/kensaku/releases/latest",
    published_at: update.date ?? "",
    asset_name: null,
    asset_url: null,
    asset_size: null,
    in_app_supported: canInstall,
  };
  return lastInfo;
}

/** The last check result, if Settings was closed and reopened. */
export function getCachedUpdateInfo(): UpdateInfo | null {
  return lastInfo;
}

/**
 * Downloads the checked update in the background. The Tauri updater verifies
 * the minisign signature before the downloaded bytes can be installed.
 */
export async function downloadUpdate(): Promise<UpdateStatus> {
  if (!pending) throw new Error("Check for updates before downloading one.");
  if (status.busy) return snapshot();
  if (!(await inAppUpdatesSupported())) {
    throw new Error(i18n.t("settings:about.manualUpdateRequired"));
  }

  status = {
    ...emptyStatus(),
    busy: true,
    progress: { phase: "downloading", downloaded: 0, total: 0, percent: 0 },
  };

  try {
    let downloaded = 0;
    let total = 0;

    await pending.download((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      } else if (event.event === "Finished") {
        status.ready_to_install = true;
      }
      status.progress = {
        phase: event.event === "Finished" ? "ready" : "downloading",
        downloaded,
        total,
        percent: total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0,
      };
    });

    status.busy = false;
    status.ready_to_install = true;
    return snapshot();
  } catch (err) {
    status.busy = false;
    status.error = String(err);
    throw err;
  }
}

/** Current download state, for the settings panel poller. */
export async function getUpdaterStatus(): Promise<UpdateStatus> {
  return snapshot();
}

/**
 * Installs the downloaded update. On Windows the installer relaunches the
 * app; macOS and Linux need an explicit relaunch.
 */
export async function installUpdateAndRestart(): Promise<void> {
  if (!pending) throw new Error("No downloaded update to install.");
  if (!(await inAppUpdatesSupported())) {
    throw new Error(i18n.t("settings:about.manualUpdateRequired"));
  }
  await pending.install();
  await relaunch();
}

/**
 * Formats byte count into human-readable MB / KB string.
 */
export function formatBytes(bytes?: number | null): string {
  if (!bytes || bytes <= 0) return "";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1) {
    return `${mb.toFixed(1)} MB`;
  }
  const kb = bytes / 1024;
  return `${Math.round(kb)} KB`;
}

/**
 * Formats an ISO date into localized short date.
 */
export function formatReleaseDate(iso?: string | null, lang?: string): string {
  if (!iso) return "";
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(lang, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso;
  }
}
