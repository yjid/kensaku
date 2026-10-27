import { Bookmark, LayoutGrid, Rows3 } from "lucide-react";

/* Right side of the results row: layout (grid or list) and save-search.
   Same pill language as the category chips on the left, so the whole row
   reads as one control bar. Icon-only, with a tooltip for each. */

const pill =
  "flex h-7 shrink-0 items-center justify-center rounded-full px-2.5 text-[12px] font-medium transition-colors duration-150 [transition-timing-function:var(--ease-apple)]";

export function ResultControls({
  view,
  onView,
  onSave,
  canSave,
}: {
  view: "grid" | "list";
  onView: (v: "grid" | "list") => void;
  onSave: () => void;
  canSave: boolean;
}) {
  const option = (v: "grid" | "list", Icon: typeof LayoutGrid, label: string) => {
    const active = view === v;
    return (
      <button
        key={v}
        type="button"
        role="radio"
        aria-checked={active}
        aria-label={label}
        title={label}
        onClick={() => onView(v)}
        className={`${pill} ${active ? "bg-fg text-bg" : "text-muted hover:bg-sunken hover:text-fg"}`}
      >
        <Icon className="h-3.5 w-3.5" strokeWidth={1.75} />
      </button>
    );
  };

  return (
    <div className="flex shrink-0 items-center gap-1 pl-2">
      <div role="radiogroup" aria-label="Layout" className="flex items-center gap-0.5 rounded-full bg-sunken p-0.5">
        {option("grid", LayoutGrid, "Grid")}
        {option("list", Rows3, "List")}
      </div>
      {canSave && (
        <button type="button" title="Save this search" aria-label="Save this search" onClick={onSave}
          className={`${pill} text-muted hover:bg-sunken hover:text-fg`}>
          <Bookmark className="h-3.5 w-3.5" strokeWidth={1.75} />
        </button>
      )}
    </div>
  );
}
