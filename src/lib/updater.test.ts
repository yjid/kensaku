import { beforeEach, describe, expect, it, vi } from "vitest";

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
      onEvent({ event: "Progress", data: { chunkLength: 10 } });
      onEvent({ event: "Finished" });
    }),
    install: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
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
    vi.setSystemTime(new Date("2026-11-25T00:00:00Z"));
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
    await expect(result).resolves.toBe(false);

    expect(onDeferred).toHaveBeenCalledOnce();
    expect(update.install).not.toHaveBeenCalled();
    expect(relaunch).not.toHaveBeenCalled();
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

    const { autoInstallUpdate } = await import("./updater");
    const onDeferred = vi.fn();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const result = autoInstallUpdate(undefined, onDeferred);
    await vi.advanceTimersByTimeAsync(5000);

    await expect(result).resolves.toBe(false);
    expect(onDeferred).toHaveBeenCalledOnce();
    expect(update.install).not.toHaveBeenCalled();
    warn.mockRestore();
  });
});
