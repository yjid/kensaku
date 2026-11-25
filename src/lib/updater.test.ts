// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UPDATE_ATTEMPT_KEY } from "./updateGuard";

const invoke = vi.fn();
const getVersion = vi.fn();
const checkPlugin = vi.fn();
const relaunch = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: () => getVersion(),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: () => relaunch(),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => checkPlugin(),
}));
vi.mock("../i18n", () => ({ default: { t: (key: string) => key } }));

let indexing = false;
let statusCallAt = -1;

function updateStub() {
  return {
    currentVersion: "1.0.19",
    version: "1.0.20",
    body: "notes",
    date: "2026-11-25",
    download: vi.fn(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 10 } });
      onEvent({ event: "Progress", data: { chunkLength: 4 } });
      onEvent({ event: "Progress", data: { chunkLength: 6 } });
      onEvent({ event: "Finished" });
    }),
    install: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
  };
}

/** Records the activity stream the status bar renders. */
function recordActivity(
  subscribe: (cb: (next: { phase: string; secondsLeft?: number; percent?: number } | null) => void) => () => void,
) {
  const seen: string[] = [];
  const unsubscribe = subscribe((next) => {
    if (!next) return;
    if (next.phase === "restarting") seen.push(`restarting:${next.secondsLeft}`);
    else if (next.phase === "downloading") seen.push(`downloading:${next.percent}`);
    else seen.push(next.phase);
  });
  return { seen, unsubscribe };
}

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  vi.useRealTimers();
  localStorage.clear();
  indexing = false;
  statusCallAt = -1;
  getVersion.mockResolvedValue("1.0.19");
  relaunch.mockResolvedValue(undefined);
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === "updater_supported") return true;
    if (cmd === "get_indexing_status") {
      statusCallAt = Date.now();
      return { is_indexing: indexing };
    }
    throw new Error(`unexpected command: ${cmd}`);
  });
});

describe("autoInstallUpdate", () => {
  it("re-checks the live crawl state immediately before installing", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate } = await import("./updater");
    const onReady = vi.fn();
    const onDeferred = vi.fn();
    const result = autoInstallUpdate(onReady, onDeferred);

    await vi.advanceTimersByTimeAsync(5000);
    await expect(result).resolves.toBe(true);

    expect(onReady).toHaveBeenCalledOnce();
    expect(relaunch).toHaveBeenCalledOnce();
    expect(update.install).toHaveBeenCalledOnce();
    // The check ran after the five-second notice, not at startup.
    expect(statusCallAt).toBeGreaterThanOrEqual(5000);
    expect(onDeferred).not.toHaveBeenCalled();
  });

  it("publishes the download and restart countdown to the status bar", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate, subscribeUpdateActivity } = await import("./updater");
    const { seen, unsubscribe } = recordActivity(subscribeUpdateActivity);
    const result = autoInstallUpdate();

    await vi.advanceTimersByTimeAsync(5000);
    await expect(result).resolves.toBe(true);
    unsubscribe();

    expect(seen).toContain("downloading:0");
    expect(seen).toContain("downloading:40");
    expect(seen).toContain("ready");
    // Counts down in whole seconds, so the strip can say "Restarting in 4s".
    expect(seen.filter((s) => s.startsWith("restarting:"))).toEqual([
      "restarting:5",
      "restarting:4",
      "restarting:3",
      "restarting:2",
      "restarting:1",
    ]);
  });

  it("waits for the window to be hidden before installing on its own", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate, setPresenceProvider, getUpdateActivity } = await import("./updater");
    let present = true;
    setPresenceProvider(() => present);

    const result = autoInstallUpdate();
    await vi.advanceTimersByTimeAsync(5000);

    // Download finished, but the user is looking at the window: stay ready.
    expect(update.install).not.toHaveBeenCalled();
    expect(getUpdateActivity()?.phase).toBe("ready");

    // They switch to another app, and the retry installs without a click.
    present = false;
    await vi.advanceTimersByTimeAsync(20_000);
    await vi.advanceTimersByTimeAsync(5_000);
    await expect(result).resolves.toBe(true);
    expect(update.install).toHaveBeenCalledOnce();
    expect(relaunch).toHaveBeenCalledOnce();
  });

  it("installs on an explicit click even while the window is focused", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { checkForUpdates, downloadUpdate, installReadyUpdate, setPresenceProvider } =
      await import("./updater");
    setPresenceProvider(() => true);
    await checkForUpdates();
    await downloadUpdate();

    const result = installReadyUpdate();
    await vi.advanceTimersByTimeAsync(5000);
    await expect(result).resolves.toBe(true);
    expect(update.install).toHaveBeenCalledOnce();
  });

  it("defers an explicit click while a crawl is running", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { checkForUpdates, downloadUpdate, installReadyUpdate, setPresenceProvider } =
      await import("./updater");
    setPresenceProvider(() => true);
    await checkForUpdates();
    await downloadUpdate();

    indexing = true;
    const onDeferred = vi.fn();
    const result = installReadyUpdate(undefined, onDeferred);
    await vi.advanceTimersByTimeAsync(1);
    expect(onDeferred).toHaveBeenCalledOnce();
    expect(update.install).not.toHaveBeenCalled();

    indexing = false;
    await vi.advanceTimersByTimeAsync(20_000);
    await vi.advanceTimersByTimeAsync(5_000);
    await expect(result).resolves.toBe(true);
    expect(update.install).toHaveBeenCalledOnce();
  });

  it("records the target version before installing, so a failed install is detectable", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate } = await import("./updater");
    const result = autoInstallUpdate();
    await vi.advanceTimersByTimeAsync(5000);
    await expect(result).resolves.toBe(true);

    expect(localStorage.getItem(UPDATE_ATTEMPT_KEY)).toContain("1.0.20");
  });

  it("defers the install when a crawl starts during the countdown", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate } = await import("./updater");
    const onDeferred = vi.fn();
    const result = autoInstallUpdate(undefined, onDeferred);

    await vi.advanceTimersByTimeAsync(4999);
    indexing = true;
    await vi.advanceTimersByTimeAsync(1);
    await vi.advanceTimersByTimeAsync(1);

    // Deferred, not cancelled: still downloading-installed, just waiting.
    expect(onDeferred).toHaveBeenCalledOnce();
    expect(update.install).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();

    // Indexing finishes, the retry installs on its own.
    indexing = false;
    await vi.advanceTimersByTimeAsync(20_000);
    await vi.advanceTimersByTimeAsync(5_000);
    await expect(result).resolves.toBe(true);
    expect(update.install).toHaveBeenCalledOnce();
    expect(relaunch).toHaveBeenCalledOnce();
  });

  it("gives up after the retry window and leaves the update ready", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate, IDLE_RETRY_LIMIT_MS, getUpdateActivity } = await import("./updater");
    indexing = true;
    const result = autoInstallUpdate();

    await vi.advanceTimersByTimeAsync(IDLE_RETRY_LIMIT_MS + 1000);
    await expect(result).resolves.toBe(false);

    expect(update.install).not.toHaveBeenCalled();
    // The download survives, so the strip can offer "Restart to update".
    expect(getUpdateActivity()?.phase).toBe("ready");
  });

  it("defers when the crawl state cannot be read", async () => {
    const update = updateStub();
    checkPlugin.mockResolvedValue(update);
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "updater_supported") return true;
      if (cmd === "get_indexing_status") throw new Error("backend gone");
      throw new Error(`unexpected command: ${cmd}`);
    });
    vi.useFakeTimers();

    const { autoInstallUpdate, IDLE_RETRY_LIMIT_MS } = await import("./updater");
    const onDeferred = vi.fn();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const result = autoInstallUpdate(undefined, onDeferred);

    await vi.advanceTimersByTimeAsync(IDLE_RETRY_LIMIT_MS + 1000);
    await expect(result).resolves.toBe(false);
    expect(onDeferred).toHaveBeenCalled();
    expect(update.install).not.toHaveBeenCalled();
    warn.mockRestore();
  });

  it("marks a failed download instead of leaving a stale progress bar", async () => {
    const update = updateStub();
    update.download.mockRejectedValue(new Error("network down"));
    checkPlugin.mockResolvedValue(update);
    vi.useFakeTimers();

    const { autoInstallUpdate, getUpdateActivity } = await import("./updater");
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const result = autoInstallUpdate();

    await expect(result).rejects.toThrow("network down");
    expect(getUpdateActivity()?.phase).toBe("failed");
    expect(update.install).not.toHaveBeenCalled();
    warn.mockRestore();
  });
});
