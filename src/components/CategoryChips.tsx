import React from "react";
import { CATEGORY_CHIPS, categoryCounts } from "../lib/categories";
import { Facets, SearchResult } from "../types";
import { formatComma } from "../lib/format";

/* Category tabs. Text, with the count set small beside it. Click toggles
   include; Alt-click toggles exclude. OR within the row, AND with the query.
   Categories with no matches step back instead of disappearing, so the row
   never reflows while typing. */

interface CategoryChipsProps {
  results: SearchResult[];
  facets?: Facets;
  selected: Set<string>;
  excluded: Set<string>;
  onToggle: (id: string, alt: boolean) => void;
}

export const CategoryChips: React.FC<CategoryChipsProps> = ({ results, facets, selected, excluded, onToggle }) => {
  const counts = facets ? { all: facets.all, recent: facets.recent, ...facets.types } : categoryCounts(results);
  const empty = selected.size === 0 && excluded.size === 0;
  const hasAny = results.length > 0;
  return (
    <div className="no-scrollbar -ml-2.5 flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto">
      {CATEGORY_CHIPS.map((chip) => {
        const isExcluded = excluded.has(chip.id);
        const isSelected = chip.id === "all" ? empty : selected.has(chip.id);
        const count = counts[chip.id];
        const dim = hasAny && chip.id !== "all" && !count && !isSelected;
        return (
          <button
            key={chip.id}
            type="button"
            onClick={(e) => onToggle(chip.id, e.altKey)}
            title={isExcluded ? "Excluded. Click to clear" : "Click to include, Alt-click to exclude"}
            data-active={isSelected && !isExcluded}
            className={`pill ${isExcluded ? "text-danger line-through" : ""} ${dim ? "opacity-40" : ""}`}
          >
            <span>{chip.label}</span>
            {hasAny && count != null && count > 0 && (
              <span className="t-meta -mt-1.5 text-[10px] text-subtle">{formatComma(count)}</span>
            )}
          </button>
        );
      })}
    </div>
  );
};
