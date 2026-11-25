// @vitest-environment jsdom
import { beforeAll, describe, expect, it } from "vitest";
import i18n from "../i18n";
import { whatsNewItems, whatsNewKey } from "./whatsNew";
import en from "../locales/en/whatsNew.json";
import ru from "../locales/ru/whatsNew.json";
import zh from "../locales/zh/whatsNew.json";

describe("whatsNewKey", () => {
  it("turns a version into an i18next-safe key", () => {
    // Dots would be read as nesting by i18next.
    expect(whatsNewKey("1.0.20")).toBe("1_0_20");
    expect(whatsNewKey(" 1.0.20 ")).toBe("1_0_20");
    expect(whatsNewKey("")).toBe("");
  });
});

describe("bundled release notes", () => {
  it("has notes for the version this branch ships", () => {
    for (const [lang, data] of Object.entries({ en, ru, zh })) {
      const items = (data as Record<string, string[]>)["1_0_20"];
      expect(Array.isArray(items), `${lang} is missing 1_0_20 notes`).toBe(true);
      expect(items.length).toBeGreaterThanOrEqual(3);
      expect(items.every((item) => typeof item === "string" && item.trim().length > 0)).toBe(true);
    }
  });

  it("keeps the same number of bullets in every language", () => {
    const counts = [en, ru, zh].map((data) => (data as Record<string, string[]>)["1_0_20"].length);
    expect(new Set(counts).size).toBe(1);
  });
});

describe("whatsNewItems", () => {
  beforeAll(async () => {
    if (!i18n.isInitialized) {
      await new Promise<void>((resolve) => {
        i18n.on("initialized", () => resolve());
      });
    }
  });

  it("resolves the notes for the running version through i18n", () => {
    // Proves the namespace is registered and the dotted version maps to a key
    // that exists, which is the part a JSON-only test cannot see.
    expect(whatsNewItems("1.0.20")).toHaveLength(5);
  });

  it("returns an empty list for a version without notes", () => {
    expect(whatsNewItems("0.9.9")).toEqual([]);
    expect(whatsNewItems("…")).toEqual([]);
  });
});
