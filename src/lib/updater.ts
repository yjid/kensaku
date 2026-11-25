import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n";
import { recordAttempt } from "./updateGuard";
import type { IndexingStatus, UpdateInfo, UpdateStatus } from "../types";

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

/** The check currently in flight, shared by startup and manual checks so two
 *  callers cannot close each other's pending `Update` resource. */
let checking: Promise<UpdateInfo> | null = null;

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

/**
 * What the status bar shows for the update flow. The updater is the only
 * writer, so a startup auto-install, a manual check, and a manual download all
 * feed the same state and the indicator cannot disagree with what is running.
 */
export interface UpdateActivity {
  phase: "available" | "downloading" | "ready" | "restarting" | "deferred" | "failed";
  version: string;
  percent?: number;
  secondsLeft?: number;
  message?: string;
}

let activity: UpdateActivity | null = null;
const activityListeners = new Set<(next: UpdateActivity | null) => void>();

function setActivity(next: UpdateActivity | null): void {
  activity = next;
  for (const listener of activityListeners) {
    try {
      listener(next);
    } catch (err) {
      console.warn("Update activity listener failed:", err);
    }
  }
}

/** Last known activity, so a late subscriber renders immediately. */
export function getUpdateActivity(): UpdateActivity | null {
  return activity;
}

/** Subscribes to update activity. Fires once with the current value. */
export function subscribeUpdateActivity(
  listener: (next: UpdateActivity | null) => void,
): () => void {
  activityListeners.add(listener);
  listener(activity);
  return () => {
    activityListeners.delete(listener);
  };
}

/** How long the restart notice stays up before installing. */
export const RESTART_COUNTDOWN_SECONDS = 5;
/** How often a deferred install re-checks whether indexing finished. */
export const IDLE_RETRY_INTERVAL_MS = 20_000;
/** After this long, a deferred install waits for the next launch instead. */
export const IDLE_RETRY_LIMIT_MS = 2 * 60 * 60 * 1000;

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Whether the user can see the window and is working in it. Restarting a
 * focused window is the one thing every mature updater avoids, so the
 * automatic path waits for the window to be hidden or unfocused. An explicit
 * click is consent and skips this.
 */
type PresenceProvider = () => boolean;
let presenceProvider: PresenceProvider | null = null;

export function setPresenceProvider(provider: PresenceProvider | null): void {
  presenceProvider = provider;
}

export function isUserPresent(): boolean {
  // No provider (tests, headless) means nobody is watching. A provider that
  // throws is unknown, which counts as watching: never restart on a guess.
  if (!presenceProvider) return false;
  try {
    return presenceProvider();
  } catch {
    return true;
  }
}

function snapshot(): UpdateStatus {
  return {
    ...status,
    progress: status.progress ? { ...status.progress } : null,
  };
}

/**
 * Checks the configured updater endpoint for a signed release.
 */
export function checkForUpdates(): Promise<UpdateInfo> {
  if (!checking) {
    checking = runCheck().finally(() => {
      checking = null;
    });
  }
  return checking;
}

async function runCheck(): Promise<UpdateInfo> {
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
  const version = lastInfo?.latest_version ?? "";
  setActivity({ phase: "downloading", version, percent: 0 });

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
      setActivity({
        phase: status.progress.phase === "ready" ? "ready" : "downloading",
        version,
        percent: status.progress.percent,
      });
    });

    status.busy = false;
    status.ready_to_install = true;
    return snapshot();
  } catch (err) {
    status.busy = false;
    status.error = String(err);
    setActivity({ phase: "failed", version, message: String(err) });
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
  // Record the target before the install so a launch that comes back on the old
  // version can tell the install failed instead of retrying it forever.
  if (typeof localStorage !== "undefined") {
    recordAttempt(localStorage, lastInfo?.latest_version ?? "");
  }
  setActivity(null);
  await pending.install();
  await relaunch();
}

/**
 * Reads the live crawl state from the backend. The status used to decide
 * whether to *start* an automatic install is stale by the time the download
 * and the restart countdown finish; a crawl can begin in that window. An
 * unreadable status counts as active, so we never restart on top of an
 * unknown state.
 */
async function crawlIsActive(): Promise<boolean> {
  try {
    const status = await invoke<Pick<IndexingStatus, "is_indexing">>(
      "get_indexing_status",
    );
    return status.is_indexing === true;
  } catch (err) {
    console.warn("Could not read indexing status before update install:", err);
    return true;
  }
}

/**
 * Polls a condition on the retry interval until the deadline. Returns false when
 * the window expires, leaving the downloaded update `ready` for a click or the
 * next launch.
 */
async function waitUntil(
  done: () => Promise<boolean> | boolean,
  deadline: number,
): Promise<boolean> {
  while (Date.now() < deadline) {
    await wait(IDLE_RETRY_INTERVAL_MS);
    if (await done()) return true;
  }
  return false;
}

/**
 * Waits for indexing to end and, on the automatic path, for the window to be
 * hidden or unfocused, then counts down, re-checks, and installs.
 *
 * A crawl that starts before the install defers it rather than cancelling it:
 * `onDeferred` lets the caller correct the restart notice and the install
 * retries until the machine is safe to restart. If the window expires the
 * update stays downloaded and ready, so the indicator or Settings can install
 * it later.
 */
async function finishInstall(
  version: string,
  opts: {
    requirePresence: boolean;
    onReady?: () => void;
    onDeferred?: () => void;
  },
): Promise<boolean> {
  const deadline = Date.now() + IDLE_RETRY_LIMIT_MS;
  for (;;) {
    // Never install on top of a crawl; wait it out instead of cancelling.
    if (await crawlIsActive()) {
      setActivity({ phase: "deferred", version });
      opts.onDeferred?.();
      if (!(await waitUntil(async () => !(await crawlIsActive()), deadline))) {
        setActivity({ phase: "ready", version });
        return false;
      }
      continue;
    }

    // A focused window means the user is here. Installing later, invisibly, is
    // better than restarting under their cursor.
    if (opts.requirePresence && isUserPresent()) {
      setActivity({ phase: "ready", version });
      if (!(await waitUntil(() => !isUserPresent(), deadline))) return false;
      continue;
    }

    opts.onReady?.();
    for (let secondsLeft = RESTART_COUNTDOWN_SECONDS; secondsLeft > 0; secondsLeft--) {
      setActivity({ phase: "restarting", version, secondsLeft });
      await wait(1000);
    }

    // Re-check both after the countdown: a crawl can start, or the user can
    // come back, while it runs.
    if (await crawlIsActive()) continue;
    if (opts.requirePresence && isUserPresent()) continue;

    await installUpdateAndRestart();
    return true;
  }
}

/**
 * Automatic path for "install updates automatically": check, download, then
 * install once indexing is idle and the window cannot be seen.
 *
 * Returns false when there is nothing to install, this copy updates through its
 * own package (MSI, portable, .deb/.rpm), or the update never became safe to
 * install within the retry window.
 */
export async function autoInstallUpdate(
  onReadyToRestart?: () => void,
  onDeferred?: () => void,
): Promise<boolean> {
  const info = await checkForUpdates();
  if (!info.has_update || !info.in_app_supported) {
    setActivity(null);
    return false;
  }
  const version = info.latest_version;

  const downloaded = await downloadUpdate();
  if (!downloaded.ready_to_install) return false;

  return finishInstall(version, {
    requirePresence: true,
    onReady: onReadyToRestart,
    onDeferred,
  });
}

/**
 * Installs a downloaded update on the user's explicit request. Consent replaces
 * the "window is invisible" gate, but a running crawl still defers: killing an
 * index is not what "restart now" means.
 */
export async function installReadyUpdate(
  onReadyToRestart?: () => void,
  onDeferred?: () => void,
): Promise<boolean> {
  if (!pending) throw new Error("No downloaded update to install.");
  const version = lastInfo?.latest_version ?? activity?.version ?? "";
  return finishInstall(version, {
    requirePresence: false,
    onReady: onReadyToRestart,
    onDeferred,
  });
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
