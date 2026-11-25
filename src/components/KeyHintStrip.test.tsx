// @vitest-environment jsdom
import { renderToStaticMarkup } from "react-dom/server";
import { I18nextProvider } from "react-i18next";
import i18n from "i18next";
import { beforeAll, describe, expect, it } from "vitest";
import { KeyHintStrip } from "./KeyHintStrip";
import enCommon from "../locales/en/common.json";
import type { UpdateActivity } from "../lib/updater";

/**
 * Renders the component, not just its state machine. The strip is a switch over
 * the update phase, and a wrong translation key or a broken conditional would
 * pass the pure tests while showing nothing useful in the app.
 */
let instance: ReturnType<typeof i18n.createInstance>;

beforeAll(async () => {
  instance = i18n.createInstance();
  // The project's typed resources narrow `init` to the app's exact namespace
  // shape; a test instance only needs `common`, so the options are cast.
  await instance.init({
    lng: "en",
    fallbackLng: "en",
    ns: ["common"],
    defaultNS: "common",
    resources: { en: { common: enCommon } },
    initImmediate: false,
    interpolation: { escapeValue: false },
  } as never);
});

function render(update: UpdateActivity | null): string {
  return renderToStaticMarkup(
    <I18nextProvider i18n={instance}>
      <KeyHintStrip status={null} doctor={null} update={update} />
    </I18nextProvider>,
  );
}

const update = (phase: UpdateActivity["phase"], extra: Partial<UpdateActivity> = {}): UpdateActivity => ({
  phase,
  version: "1.0.21",
  ...extra,
});

describe("KeyHintStrip update indicator", () => {
  it("shows nothing when there is no update", () => {
    const html = render(null);
    expect(html).not.toContain("Update 1.0.21");
    expect(html).not.toContain("Restart");
  });

  it("names the available version", () => {
    const html = render(update("available"));
    expect(html).toContain("Update 1.0.21");
    expect(html).toContain('title="Update available: 1.0.21"');
  });

  it("renders the download percentage and a matching progress bar", () => {
    const html = render(update("downloading", { percent: 42 }));
    expect(html).toContain("Downloading 42%");
    expect(html).toContain("width:42%");
  });

  it("does not draw a zero-width bar before the size is known", () => {
    const html = render(update("downloading", { percent: 0 }));
    expect(html).toContain("Downloading…");
    expect(html).not.toContain("width:0%");
  });

  it("offers the restart when the download is ready", () => {
    expect(render(update("ready"))).toContain("Restart to update");
  });

  it("counts down without announcing every second", () => {
    const html = render(update("restarting", { secondsLeft: 4 }));
    expect(html).toContain("Restarting in 4s");
    expect(html).toContain('aria-live="off"');
  });

  it("says why the install is waiting", () => {
    const html = render(update("deferred"));
    expect(html).toContain("Update waits for index");
    expect(html).toContain('aria-live="polite"');
  });

  it("shows the failure and keeps the backend detail in the tooltip", () => {
    const html = render(update("failed", { message: "network down" }));
    expect(html).toContain("Update failed");
    expect(html).toContain("network down");
  });
});
