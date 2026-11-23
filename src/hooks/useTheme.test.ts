import { describe, expect, it } from "vitest";
import themeBoot from "../../public/theme-boot.js?raw";
import { normalizeTheme } from "./useTheme";

const themeCases = [
  ["light", "light"],
  ["dark", "dark"],
  ["paper", "light"],
  ["void", "dark"],
  ["frost", "frost"],
  ["graphite", "liquid-glass"],
  ["liquid-glass", "liquid-glass"],
] as const;

function readThemeMigrationMap(): Record<string, string> {
  const mapSource = themeBoot.match(/var map = \{([^}]+)\};/)?.[1];
  if (!mapSource) throw new Error("Could not find the theme boot migration map");

  const entries = [
    ...mapSource.matchAll(/(?:"([^"]+)"|([\w-]+)):\s*"([^"]+)"/g),
  ].map(([, quotedKey, bareKey, value]) => [quotedKey ?? bareKey, value]);
  return Object.fromEntries(entries);
}

describe("normalizeTheme", () => {
  it.each(themeCases)("keeps %s as %s", (input, expected) => {
    expect(normalizeTheme(input)).toBe(expected);
  });

  it("falls back to dark for an unknown value", () => {
    expect(normalizeTheme("neon")).toBe("dark");
  });

  it("keeps the pre-paint migration map in sync", () => {
    expect(readThemeMigrationMap()).toEqual(
      Object.fromEntries(themeCases),
    );
    for (const [stored, expected] of themeCases) {
      expect(normalizeTheme(stored)).toBe(expected);
    }
  });
});
