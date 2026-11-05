import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bookmark, Check, ChevronDown, LayoutGrid, Rows3 } from "lucide-react";

/* Right side of the toolbar: sort, layout, save. Same capsule material as
   the category segments. */

/* The sort control is an in-app listbox, not a native <select>. On Linux a
   <select> opens its option list as a separate GTK popup that steals focus
   from the webview, which the launcher read as "user left" and hid the
   palette (#95). It also renders differently per OS; this does not. */

export function ResultControls({
  sort,
  onSort,
  view,
  onView,
  onSave,
  canSave,
}: {
  sort: string;
  onSort: (value: string) => void;
  view: "grid" | "list";
  onView: (v: "grid" | "list") => void;
  onSave: () => void;
  canSave: boolean;
}) {
  const { t } = useTranslation("search");
  const [sortOpen, setSortOpen] = useState(false);
  const sortRef = useRef<HTMLDivElement>(null);
  const sortButtonRef = useRef<HTMLButtonElement>(null);

  const SORT_OPTIONS = [
    { value: "best", label: t("sort.best") },
    { value: "newest", label: t("sort.newest") },
    { value: "oldest", label: t("sort.oldest") },
    { value: "largest", label: t("sort.largest") },
    { value: "name", label: t("sort.name") },
  ];
  const currentLabel = SORT_OPTIONS.find((o) => o.value === sort)?.label ?? sort;

  useEffect(() => {
    if (!sortOpen) return;
    const onDown = (e: MouseEvent) => {
      if (!sortRef.current?.contains(e.target as Node)) setSortOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setSortOpen(false);
        sortButtonRef.current?.focus();
      }
    };
    const onOpen = () => {
      sortRef.current
        ?.querySelector<HTMLButtonElement>('[role="option"][aria-selected="true"]')
        ?.focus();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    const raf = requestAnimationFrame(onOpen);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      cancelAnimationFrame(raf);
    };
  }, [sortOpen]);

  const pickSort = (value: string) => {
    setSortOpen(false);
    sortButtonRef.current?.focus();
    if (value !== sort) onSort(value);
  };

  return (
    <div className="flex shrink-0 items-center gap-2">
      <div ref={sortRef} className="relative">
        <button
          type="button"
          ref={sortButtonRef}
          aria-haspopup="listbox"
          aria-expanded={sortOpen}
          aria-label={t("sort.label")}
          title={t("sort.label")}
          onClick={() => setSortOpen((v) => !v)}
          onKeyDown={(e) => {
            if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
            e.preventDefault();
            setSortOpen(true);
          }}
          className="control flex h-[38px] cursor-pointer items-center gap-2 rounded-control pl-3.5 pr-2.5 text-[13px] font-medium text-fg outline-none"
        >
          <span>{currentLabel}</span>
          <ChevronDown
            className={`h-3.5 w-3.5 text-subtle transition-transform ${sortOpen ? "rotate-180" : ""}`}
            strokeWidth={2}
          />
        </button>

        {sortOpen && (
          <div
            role="listbox"
            aria-label={t("sort.label")}
            className="menu-pop elevated absolute right-0 top-[calc(100%+6px)] z-[70] min-w-[160px] overflow-hidden p-1"
          >
            {SORT_OPTIONS.map((o) => (
              <button
                key={o.value}
                type="button"
                role="option"
                aria-selected={o.value === sort}
                onClick={() => pickSort(o.value)}
                className="menu-row flex w-full items-center justify-between text-left"
              >
                <span>{o.label}</span>
                {o.value === sort && <Check className="h-3.5 w-3.5 shrink-0" strokeWidth={2} />}
              </button>
            ))}
          </div>
        )}
      </div>

      <div role="radiogroup" aria-label={t("view.layout")} className="control flex items-center gap-0.5 rounded-control p-1">
        <button type="button" role="radio" aria-checked={view === "grid"} aria-label={t("view.gridShort")} title={t("view.gridShort")} onClick={() => onView("grid")} className="seg !px-2.5">
          <LayoutGrid className="h-4 w-4" strokeWidth={1.75} />
        </button>
        <button type="button" role="radio" aria-checked={view === "list"} aria-label={t("view.listShort")} title={t("view.listShort")} onClick={() => onView("list")} className="seg !px-2.5">
          <Rows3 className="h-4 w-4" strokeWidth={1.75} />
        </button>
      </div>

      {canSave && (
        <button type="button" title={t("saveSearch")} aria-label={t("saveSearch")} onClick={onSave} className="control flex h-[38px] w-[38px] items-center justify-center rounded-control text-muted transition-colors hover:text-fg">
          <Bookmark className="h-4 w-4" strokeWidth={1.75} />
        </button>
      )}
    </div>
  );
}
