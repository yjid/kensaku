import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { usePresence } from "../lib/usePresence";

const KEYS = [
  { combo: "Alt+Space", key: "showHide" },
  { combo: "↑ ↓", key: "moveResults" },
  { combo: "← →", key: "moveRow" },
  { combo: "Space", key: "previewResults" },
  { combo: "⌘/Ctrl+Y", key: "previewAnywhere" },
  { combo: "Enter", key: "openFile" },
  { combo: "Shift+Enter", key: "showFolder" },
  { combo: "⌘/Ctrl+C", key: "copyPath" },
  { combo: "⌘/Ctrl+Click · Shift+Click · ⌘/Ctrl+A", key: "selectMany" },
  { combo: "⌘/Ctrl+I", key: "details" },
  { combo: "⌘/Ctrl+Shift+L", key: "toggleView" },
  { combo: "⌘/Ctrl+,", key: "settings" },
  { combo: "?", key: "thisSheet" },
  { combo: "Esc", key: "escape" },
] as const;

const FILTERS = [
  { token: "type:pdf", key: "type" },
  { token: "ext:rs", key: "ext" },
  { token: "in:~/Projects", key: "in" },
  { token: "path:src", key: "path" },
  { token: "modified:7d", key: "modified" },
  { token: "size:>2mb", key: "size" },
  { token: "is:image, is:dir", key: "aliases" },
  { token: "lang:rust", key: "lang" },
  { token: "name:config", key: "name" },
  { token: "content:timeout", key: "content" },
  { token: 're:"fn \\w+"', key: "regex" },
  { token: '"last Tuesday"', key: "time" },
  { token: "mouse near[5] button", key: "near" },
] as const;

export function HelpDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation("help");
  const { closing, close } = usePresence(onClose);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "?") {
        e.preventDefault();
        close();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [close]);

  return (
    <div className={`${closing ? "fade-out pointer-events-none" : "fade-in"} fixed inset-0 z-50 flex items-center justify-center bg-scrim p-6 backdrop-blur-[6px]`}>
      <div className={`elevated ${closing ? "overlay-out" : "overlay-in"} max-h-[80vh] w-full max-w-2xl overflow-y-auto rounded-sheet p-7`}>
        <div className="flex items-center justify-between pb-3">
          <h2 className="t-title text-fg">{t("title")}</h2>
          <button
            type="button"
            onClick={close}
            className="btn btn-ghost btn-icon h-8 !rounded-full"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <section className="pb-4">
          <h3 className="t-meta pb-2 pt-4 text-subtle">{t("keyboard")}</h3>
          <table className="w-full">
            <tbody>
              {KEYS.map(({ combo, key }) => (
                <tr key={combo} className="border-t border-line">
                  <td className="t-meta py-2.5 pr-6 text-fg">{combo}</td>
                  <td className="t-body py-2.5 text-muted">{t(`keys.${key}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section>
          <h3 className="t-meta pb-2 pt-4 text-subtle">{t("filters")}</h3>
          <table className="w-full">
            <tbody>
              {FILTERS.map(({ token, key }) => (
                <tr key={token} className="border-t border-line">
                  <td className="t-meta py-2.5 pr-6 text-fg">{token}</td>
                  <td className="t-body py-2.5 text-muted">{t(`filterDescriptions.${key}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>
    </div>
  );
}
