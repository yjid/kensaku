import { describe, expect, it } from "vitest";
import { updateLabel, updateTitle, type Translate } from "./update-labels";
import type { UpdateActivity } from "./updater";

/** Renders the key and its parameters so the assertions read like the UI. */
const t: Translate = (key, params) =>
  params
    ? `${key}(${Object.entries(params)
        .map(([name, value]) => `${name}=${value}`)
        .join(",")})`
    : key;

const update = (phase: UpdateActivity["phase"], extra: Partial<UpdateActivity> = {}) =>
  ({ phase, version: "1.0.21", ...extra }) as UpdateActivity;

describe("updateLabel", () => {
  it("maps every phase to a compact strip label", () => {
    expect(updateLabel(update("available"), t)).toBe("update.availableShort(version=1.0.21)");
    expect(updateLabel(update("ready"), t)).toBe("update.ready");
    expect(updateLabel(update("deferred"), t)).toBe("update.deferred");
    expect(updateLabel(update("failed"), t)).toBe("update.failed");
  });

  it("shows the download percentage, and falls back before it is known", () => {
    expect(updateLabel(update("downloading", { percent: 42 }), t)).toBe("update.downloading(percent=42)");
    expect(updateLabel(update("downloading", { percent: 0 }), t)).toBe("update.downloadingUnknown");
    expect(updateLabel(update("downloading"), t)).toBe("update.downloadingUnknown");
  });

  it("counts the restart down in whole seconds", () => {
    expect(updateLabel(update("restarting", { secondsLeft: 4 }), t)).toBe("update.restarting(seconds=4)");
    expect(updateLabel(update("restarting"), t)).toBe("update.restarting(seconds=0)");
  });
});

describe("updateTitle", () => {
  it("carries the version or the countdown into the tooltip", () => {
    expect(updateTitle(update("available"), t)).toBe("update.available(version=1.0.21)");
    expect(updateTitle(update("downloading"), t)).toBe("update.downloadingTitle(version=1.0.21)");
    expect(updateTitle(update("ready"), t)).toBe("update.readyTitle");
    expect(updateTitle(update("deferred"), t)).toBe("update.deferredTitle");
    expect(updateTitle(update("restarting", { secondsLeft: 3 }), t)).toBe(
      "update.restartingTitle(seconds=3)",
    );
  });

  it("appends the backend error when there is one", () => {
    expect(updateTitle(update("failed", { message: "network down" }), t)).toBe(
      "update.failedTitle: network down",
    );
    expect(updateTitle(update("failed"), t)).toBe("update.failedTitle");
  });
});
