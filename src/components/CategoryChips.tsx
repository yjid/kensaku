import React from "react";
import { CATEGORY_CHIPS, categoryCounts } from "../lib/categories";
import { Facets, SearchResult } from "../types";
import { formatComma } from "../lib/format";

/* Category pill row. Counts come from server facets over the full matched
   set. Click toggles include; Alt-click toggles exclude. OR within the row,
   AND against the text query. */

interface CategoryChipsProps {
  results: SearchResult[];
  facets?: Facets;
  selected: Set<string>;
  excluded: Set<string>;
  onToggle: (id: string, alt: boolean) => void;
}

export const CategoryChips: React.FC<CategoryChipsProps> = ({
  results,
  facets,
  selected,
  excluded,
  onToggle,
}) => {
  const counts = facets
    ? { all: facets.all, recent: facets.recent, ...facets.types }
    : categoryCounts(results);
  const empty = selected.size === 0 && excluded.size === 0;
  return (
    <div className="no-scrollbar flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
      {CATEGORY_CHIPS.map((chip) => {
        const isExcluded = excluded.has(chip.id);
        const isSelected =
          chip.id === "all" ? empty : selected.has(chip.id);
        const count = counts[chip.id];
        return (
          <button
            key={chip.id}
            type="button"
            onClick={(e) => onToggle(chip.id, e.altKey)}
            title={isExcluded ? "Excluded — click to clear" : "Click to include, Alt-click to exclude"}
            className={`flex h-7 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full px-3 text-[12px] font-medium transition-colors duration-150 ${
              isExcluded
                ? "bg-danger/10 text-danger line-through"
                : isSelected
                  ? "bg-fg text-bg"
                  : "text-muted hover:bg-sunken hover:text-fg"
            }`}
          >
            <span>{chip.label}</span>
            {count != null && count > 0 && (
              <span
                className={`font-mono text-[11px] ${
                  isSelected ? "opacity-70" : "text-subtle"
                }`}
              >
                {formatComma(count)}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
};