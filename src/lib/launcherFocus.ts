/* Decision for the launcher's hide-on-blur behaviour, kept pure so it can be
   unit-tested (#95). The launcher hides itself when the window loses focus,
   like every launcher does — but a transient popup that legitimately takes
   focus must not be read as "the user clicked away". Two real cases hid the
   palette on Linux: WebKitGTK's native right-click menu (now suppressed at
   the app root) and the GTK popup behind a native <select> (now an in-app
   listbox). This guard is the backstop for anything else that briefly steals
   webview focus. */

export interface BlurDecision {
  /** true only after the setup wizard finished; null while unknown. */
  setupComplete: boolean | null;
  /** undefined while the config has not arrived yet. */
  appMode: "app" | "launcher" | undefined;
  /** any in-app modal that may legitimately own focus (settings, preview,
      details, save bar, context menu, duplicates, help). */
  modalOpen: boolean;
  /** milliseconds since the last pointerdown inside the window. A value
      below POINTER_FOCUS_GRACE_MS means a pointer interaction just happened,
      so a popup it opened may be taking focus. */
  msSincePointerDown: number;
}

/** Popups that briefly steal webview focus (GTK option lists, IME windows)
    settle within a few hundred ms. Hiding the palette during that window
    destroyed it mid-interaction; clicking genuinely away never passes
    through a pointerdown inside the window first, so click-to-dismiss
    still hides immediately. */
export const POINTER_FOCUS_GRACE_MS = 350;

export function shouldHideOnBlur(input: BlurDecision): boolean {
  if (input.setupComplete !== true) return false;
  if (!input.appMode || input.appMode === "app") return false;
  if (input.modalOpen) return false;
  if (input.msSincePointerDown < POINTER_FOCUS_GRACE_MS) return false;
  return true;
}
