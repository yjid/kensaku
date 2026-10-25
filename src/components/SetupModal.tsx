import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Plus, X, ArrowRight, ArrowLeft, LoaderCircle, Sun, Moon } from "lucide-react";
import { EmptyState } from "./EmptyState";
import { useTheme } from "../hooks/useTheme";

/* First run: three screens, one decision each.
   1 Welcome → 2 Pick folders → 3 Exclusions + start. */

const DEFAULT_EXCLUSIONS = [
  "node_modules", ".git", "target", "dist", "build", "out",
  ".next", ".nuxt", ".output", ".cache", ".venv", "venv",
  "__pycache__", "vendor", ".gradle", ".idea", ".vscode",
  ".Trash", "lost+found",
];

const PRESETS: Record<string, string[]> = {
  Developer: DEFAULT_EXCLUSIONS.slice(0, 17),
  Creative: ["node_modules", ".git", ".Trash", ".cache", "Backups", "Library", "lost+found"],
  Minimal: [".Trash", "lost+found"],
};

export function SetupModal({ onSetupCompleted }: { onSetupCompleted: () => void }) {
  const [step, setStep] = useState<0 | 1 | 2>(0);
  const [suggestedDirs, setSuggestedDirs] = useState<string[]>([]);
  const [selectedDirs, setSelectedDirs] = useState<string[]>([]);
  const [customDirInput, setCustomDirInput] = useState("");
  const [excludedDirs, setExcludedDirs] = useState<string[]>(DEFAULT_EXCLUSIONS);
  const [customExcludeInput, setCustomExcludeInput] = useState("");
  const [shortcut, setShortcut] = useState("Alt+Space");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const { theme, setTheme } = useTheme();

  useEffect(() => {
    invoke<string[]>("get_suggested_directories")
      .then((dirs) => {
        setSuggestedDirs(dirs);
        if (dirs.length > 0) setSelectedDirs(dirs.slice(0, 3));
      })
      .catch((err) => console.error("Failed to load suggested dirs:", err));
  }, []);

  const toggleDir = (dir: string) =>
    setSelectedDirs((prev) =>
      prev.includes(dir) ? prev.filter((d) => d !== dir) : [...prev, dir],
    );

  const addCustomDir = () => {
    const t = customDirInput.trim();
    if (t && !selectedDirs.includes(t)) {
      setSelectedDirs((prev) => [...prev, t]);
      setCustomDirInput("");
    }
  };

  const handleFinish = async () => {
    if (selectedDirs.length === 0) return;
    setIsSubmitting(true);
    try {
      await invoke("complete_setup", {
        watchedDirs: selectedDirs,
        excludedDirs: excludedDirs,
        shortcut: shortcut,
      });
      onSetupCompleted();
    } catch (err) {
      console.error("Setup failed:", err);
      alert("Setup failed: " + err);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="absolute inset-0 z-50 flex flex-col bg-bg">
      {/* Header: wordmark + step indicator + theme toggle */}
      <header className="flex h-14 shrink-0 items-center justify-between px-5">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-semibold tracking-tight text-fg">Kensaku</span>
          <span className="font-serif text-[13px] italic text-subtle">検索 — to search</span>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            {[0, 1, 2].map((s) => (
              <span
                key={s}
                className={`h-1 w-6 rounded-full transition-colors duration-300 ${
                  s <= step ? "bg-accent" : "bg-line"
                }`}
              />
            ))}
          </div>
          <button
            type="button"
            title="Toggle theme"
            onClick={() => setTheme(theme === "paper" ? "void" : "paper")}
            className="flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors hover:bg-sunken hover:text-fg"
          >
            {theme === "paper" ? <Moon className="h-3.5 w-3.5" /> : <Sun className="h-3.5 w-3.5" />}
          </button>
        </div>
      </header>

      {/* Body — one decision per screen */}
      <main className="flex min-h-0 flex-1 flex-col items-center overflow-y-auto px-8 pb-6 pt-2">
        {step === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
            <EmptyState variant="empty" caption="Everything, already in its place" />
            <h1 className="pt-2 text-[20px] font-semibold tracking-tight text-fg">
              A precision index of your machine.
            </h1>
            <p className="max-w-sm text-[13px] leading-relaxed text-muted">
              Pick a few folders. Kensaku walks them once — every file, every line inside,
              searchable in milliseconds. Nothing ever leaves this device.
            </p>
          </div>
        )}

        {step === 1 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                What should we index?
              </h2>
              <p className="text-[12px] text-muted">
                Suggested starting points — toggle freely, add a custom path below.
              </p>
            </div>
            <div className="grid grid-cols-2 gap-2">
              {suggestedDirs.map((dir) => {
                const selected = selectedDirs.includes(dir);
                const name = dir.split("/").filter(Boolean).pop() || dir;
                return (
                  <button
                    key={dir}
                    type="button"
                    onClick={() => toggleDir(dir)}
                    className={`flex h-14 items-center gap-2.5 rounded-[14px] border px-3 text-left transition-colors duration-150 ${
                      selected
                        ? "border-accent bg-accent-soft"
                        : "bg-elevated hover:border-line-strong"
                    }`}
                  >
                    <span
                      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                        selected ? "border-accent bg-accent text-accent-fg" : "border-line-strong"
                      }`}
                    >
                      {selected && <Check className="h-2.5 w-2.5" />}
                    </span>
                    <span className="min-w-0">
                      <span className="block truncate text-[12px] font-medium text-fg">{name}</span>
                      <span className="block truncate font-mono text-[10px] text-subtle">{dir}</span>
                    </span>
                  </button>
                );
              })}
            </div>
            <div className="flex gap-1.5">
              <input
                type="text"
                value={customDirInput}
                placeholder="Or a custom path, e.g. /workspace"
                onChange={(e) => setCustomDirInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addCustomDir()}
                spellCheck={false}
                className="h-9 min-w-0 flex-1 rounded-[10px] border border-line bg-elevated px-3 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
              />
              <button
                type="button"
                onClick={addCustomDir}
                className="flex h-9 items-center gap-1 rounded-[10px] border border-line bg-elevated px-3 text-[11px] text-fg transition-colors hover:border-line-strong"
              >
                <Plus className="h-3.5 w-3.5" /> Add
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="w-full max-w-lg space-y-4">
            <div className="pb-1">
              <h2 className="text-[16px] font-semibold tracking-tight text-fg">
                What should we skip?
              </h2>
              <p className="text-[12px] text-muted">
                Ignored names keep the index lean and every query instant. A preset is a good default.
              </p>
            </div>
            <div className="flex gap-1.5">
              {Object.keys(PRESETS).map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => setExcludedDirs(PRESETS[p])}
                  className="flex h-7 items-center rounded-full border border-line px-3 text-[11px] text-muted transition-colors hover:border-line-strong hover:text-fg"
                >
                  {p} preset
                </button>
              ))}
            </div>
            <div className="flex max-h-28 flex-wrap gap-1.5 overflow-y-auto rounded-[14px] border border-line bg-elevated p-3">
              {excludedDirs.map((d) => (
                <button
                  key={d}
                  type="button"
                  title={`Remove ${d}`}
                  onClick={() => setExcludedDirs((prev) => prev.filter((x) => x !== d))}
                  className="group flex h-6 items-center gap-1.5 rounded-full bg-sunken px-2.5 font-mono text-[10px] text-muted transition-colors hover:text-danger"
                >
                  {d}
                  <X className="h-3 w-3 opacity-50 group-hover:opacity-100" />
                </button>
              ))}
            </div>
            <input
              type="text"
              value={customExcludeInput}
              placeholder="Add an ignored name — Enter"
              onChange={(e) => setCustomExcludeInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && customExcludeInput.trim()) {
                  const t = customExcludeInput.trim();
                  setExcludedDirs((prev) => (prev.includes(t) ? prev : [...prev, t]));
                  setCustomExcludeInput("");
                }
              }}
              spellCheck={false}
              className="h-9 w-full rounded-[10px] border border-line bg-elevated px-3 font-mono text-[11px] text-fg outline-none transition-colors focus:border-accent placeholder:text-subtle"
            />
          </div>
        )}
      </main>

      {/* Footer: step nav */}
      <footer className="flex h-16 shrink-0 items-center justify-between border-t border-line px-5">
        <div className="min-w-0 text-[11px] text-subtle">
          {step === 1 && `${selectedDirs.length} folder${selectedDirs.length === 1 ? "" : "s"} selected`}
          {step === 2 && (
            <span className="font-mono">
              shortcut{" "}
              <input
                type="text"
                value={shortcut}
                onChange={(e) => setShortcut(e.target.value)}
                className="w-24 rounded border border-line bg-elevated px-1.5 py-0.5 font-mono text-[10px] text-fg outline-none focus:border-accent"
              />{" "}
              toggles the window anywhere
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {step > 0 && (
            <button
              type="button"
              onClick={() => setStep((s) => (s - 1) as 0 | 1 | 2)}
              className="flex h-8 items-center gap-1.5 rounded-[10px] border border-line px-3 text-[12px] text-muted transition-colors hover:border-line-strong hover:text-fg"
            >
              <ArrowLeft className="h-3.5 w-3.5" /> Back
            </button>
          )}
          {step < 2 ? (
            <button
              type="button"
              onClick={() => setStep((s) => (s + 1) as 0 | 1 | 2)}
              disabled={step === 1 && selectedDirs.length === 0}
              className="flex h-8 items-center gap-1.5 rounded-[10px] bg-accent px-4 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              Continue <ArrowRight className="h-3.5 w-3.5" />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleFinish}
              disabled={isSubmitting || selectedDirs.length === 0}
              className="flex h-8 items-center gap-2 rounded-[10px] bg-accent px-4 text-[12px] font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-40"
            >
              {isSubmitting && <LoaderCircle className="h-3.5 w-3.5 animate-spin" />}
              Start indexing
              {!isSubmitting && <ArrowRight className="h-3.5 w-3.5" />}
            </button>
          )}
        </div>
      </footer>
    </div>
  );
}
