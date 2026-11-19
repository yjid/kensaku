import { describe, expect, it } from "vitest";
import { normalizeTheme } from "./useTheme";

describe("normalizeTheme", () => {
  it.each([
    ["light", "light"],
    ["dark", "dark"],
    ["paper", "light"],
    ["void", "dark"],
    ["frost", "frost"],
    ["liquid-glass", "liquid-glass"],
  ] as const)("keeps %s as %s", (input, expected) => {
    expect(normalizeTheme(input)).toBe(expected);
  });

  it("falls back to dark for an unknown value", () => {
    expect(normalizeTheme("neon")).toBe("dark");
  });
});
