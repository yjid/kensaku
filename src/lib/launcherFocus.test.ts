/// <reference types="bun" />
import { describe, expect, test } from "bun:test";
import { POINTER_FOCUS_GRACE_MS, shouldHideOnBlur } from "./launcherFocus";

const launcher = {
  setupComplete: true as const,
  appMode: "launcher" as const,
  modalOpen: false,
  msSincePointerDown: 10_000,
};

describe("shouldHideOnBlur", () => {
  test("hides a settled launcher whose focus really went away", () => {
    expect(shouldHideOnBlur(launcher)).toBe(true);
  });

  test("never hides during setup or before the config arrives", () => {
    expect(shouldHideOnBlur({ ...launcher, setupComplete: null })).toBe(false);
    expect(shouldHideOnBlur({ ...launcher, setupComplete: false })).toBe(false);
    expect(shouldHideOnBlur({ ...launcher, appMode: undefined })).toBe(false);
  });

  test("never hides an app-mode window", () => {
    expect(shouldHideOnBlur({ ...launcher, appMode: "app" })).toBe(false);
  });

  test("never hides while a modal owns focus", () => {
    expect(shouldHideOnBlur({ ...launcher, modalOpen: true })).toBe(false);
  });

  test("stays open when a just-opened popup took focus (#95)", () => {
    // Right-click or the sort list opens a popup: focus leaves the webview
    // within a pointerdown's grace window. The old code hid the palette here.
    expect(shouldHideOnBlur({ ...launcher, msSincePointerDown: 0 })).toBe(false);
    expect(shouldHideOnBlur({ ...launcher, msSincePointerDown: POINTER_FOCUS_GRACE_MS - 1 })).toBe(false);
  });

  test("hides again once the grace window has passed", () => {
    expect(shouldHideOnBlur({ ...launcher, msSincePointerDown: POINTER_FOCUS_GRACE_MS })).toBe(true);
  });
});
