import type { LucideIcon } from "lucide-react";
import { ArrowDownAZ, Clock, HardDrive, History, ListFilter } from "lucide-react";

/* Sort as a quiet segmented row: icon first, label after. Icons carry the
   meaning on narrow widths; the label appears on the active option. */

const OPTIONS: { value: string; label: string; icon: LucideIcon }[] = [
  { value: "best", label: "Best", icon: ListFilter },
  { value: "newest", label: "Newest", icon: Clock },
  { value: "oldest", label: "Oldest", icon: History },
  { value: "largest", label: "Largest", icon: HardDrive },
  { value: "name", label: "Name", icon: ArrowDownAZ },
];

export function SortControl({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div role="radiogroup" aria-label="Sort" className="flex items-center gap-0.5 rounded-[8px] bg-sunken p-0.5">
      {OPTIONS.map(({ value: v, label, icon: Icon }) => {
        const active = v === value;
        return (
          <button
            key={v}
            type="button"
            role="radio"
            aria-checked={active}
            title={`Sort: ${label}`}
            onClick={() => onChange(v)}
            className={`flex h-7 items-center gap-1.5 rounded-[8px] px-2 text-[12px] transition-colors duration-150 [transition-timing-function:var(--ease-apple)] ${
              active ? "bg-elevated text-fg shadow-sm" : "text-muted hover:text-fg"
            }`}
          >
            <Icon className="h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
            {active && <span className="font-medium">{label}</span>}
          </button>
        );
      })}
    </div>
  );
}
