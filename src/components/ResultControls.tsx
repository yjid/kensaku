import { Bookmark, ChevronDown, LayoutGrid, Rows3 } from "lucide-react";

/* Results bar, right side: sort dropdown, layout group, save. Same pill
   look as the category chips on the left. */

export const SORT_OPTIONS: { value: string; label: string }[] = [
  { value: "best", label: "Best match" },
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "largest", label: "Largest" },
  { value: "name", label: "Name" },
];

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
  return (
    <div className="flex shrink-0 items-center gap-2 pl-2">
      <label className="pill-select" title="Sort results">
        <span className="sr-only">Sort</span>
        <select value={sort} onChange={(e) => onSort(e.target.value)} aria-label="Sort results">
          {SORT_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        <ChevronDown className="h-3 w-3" strokeWidth={2} />
      </label>

      <div role="radiogroup" aria-label="Layout" className="pill-group">
        <button type="button" role="radio" aria-checked={view === "grid"} aria-label="Grid" title="Grid"
          onClick={() => onView("grid")} className="pill">
          <LayoutGrid className="h-3.5 w-3.5" strokeWidth={1.75} />
        </button>
        <button type="button" role="radio" aria-checked={view === "list"} aria-label="List" title="List"
          onClick={() => onView("list")} className="pill">
          <Rows3 className="h-3.5 w-3.5" strokeWidth={1.75} />
        </button>
      </div>

      {canSave && (
        <button type="button" title="Save this search" aria-label="Save this search" onClick={onSave} className="pill">
          <Bookmark className="h-3.5 w-3.5" strokeWidth={1.75} />
        </button>
      )}
    </div>
  );
}
