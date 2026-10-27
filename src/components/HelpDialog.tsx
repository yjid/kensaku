import { X } from "lucide-react";

const KEYS: [string, string][] = [
  ["Alt+Space", "show or hide the window"],
  ["↑ ↓", "move selection (grid: by row)"],
  ["← →", "move within a grid row"],
  ["Space", "Quick Look preview"],
  ["Enter", "open selected file"],
  ["Shift+Enter", "show in folder"],
  ["⌘/Ctrl+C", "copy selected path (outside search box)"],
  ["⌘/Ctrl+I", "details for selected file"],
  ["⌘/Ctrl+,", "settings"],
  ["Esc", "close dialog, clear search, then hide"],
];

const FILTERS: [string, string][] = [
  ["type:pdf", "category or extension"],
  ["ext:rs", "exact extension"],
  ["in:~/Projects", "scope to a folder"],
  ["path:src", "substring match on path"],
  ["modified:7d", "changed within 7 days (1h, 30d, 7d..30d)"],
  ["size:>2mb", "size range (kb, mb, gb)"],
  ["is:image, is:dir", "category aliases"],
  ["lang:rust", "language family of extension"],
  ["name:config", "match filename only"],
  ["content:timeout", "match file contents only"],
  ["re:\"fn \\\\w+\"", "regular expression"],
  ['"last Tuesday"', "time travel filter"],
];

export function HelpDialog({ onClose }: { onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg/70 backdrop-blur-sm">
      <div className="surface w-full max-w-2xl max-h-[80vh] overflow-y-auto p-5">
        <div className="flex items-center justify-between pb-3">
          <h2 className="text-[17px] font-semibold tracking-tight text-fg">How to search</h2>
          <button
            type="button"
            onClick={onClose}
            className="flex h-7 w-7 items-center justify-center rounded-full text-muted hover:bg-sunken hover:text-fg"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <section className="pb-4">
          <h3 className="pb-2 text-[12px] font-medium uppercase tracking-wide text-subtle">Keyboard</h3>
          <table className="w-full text-[12px]">
            <tbody>
              {KEYS.map(([k, v]) => (
                <tr key={k} className="border-t border-line">
                  <td className="py-1.5 pr-4 font-mono text-fg">{k}</td>
                  <td className="py-1.5 text-muted">{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section>
          <h3 className="pb-2 text-[12px] font-medium uppercase tracking-wide text-subtle">Filters</h3>
          <table className="w-full text-[12px]">
            <tbody>
              {FILTERS.map(([k, v]) => (
                <tr key={k} className="border-t border-line">
                  <td className="py-1.5 pr-4 font-mono text-fg">{k}</td>
                  <td className="py-1.5 text-muted">{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>
    </div>
  );
}
