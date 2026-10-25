import React from "react";
import { CATEGORY_CHIPS, CategoryChip, categoryCounts } from "../lib/categories";
import { SearchResult } from "../types";
import { formatComma } from "../lib/format";

/* Category pill row. Active = inverted (fg on bg). Counts in a muted badge.
   Horizontally scrollable, scrollbar hidden. */

interface CategoryChipsProps {
  query: string;
  results: SearchResult[];
  onPick: (chip: CategoryChip) => void;
}

function chipActive(query: string, chip: CategoryChip): boolean {
  if (!chip.token) return !/\b(type|modified):/i.test(query);
  return query.toLowerCase().includes(chip.token.toLowerCase());
}

export const CategoryChips: React.FC<CategoryChipsProps> = ({ query, results, onPick }) => {
  const counts = categoryCounts(results);
  return (
    <div className="no-scrollbar flex shrink-0 items-center gap-1 overflow-x-auto px-4 pb-2">
      {CATEGORY_CHIPS.map((chip) => {
        const active = chipActive(query, chip);
        const count = counts[chip.id];
        return (
          <button
            key={chip.id}
            type="button"
            onClick={() => onPick(chip)}
            className={`flex h-7 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full px-3 text-[12px] font-medium transition-colors duration-150 ${
              active
                ? "bg-fg text-bg"
                : "text-muted hover:bg-sunken hover:text-fg"
            }`}
          >
            <span>{chip.label}</span>
            {count != null && count > 0 && chip.id === "all" && (
              <span
                className={`font-mono text-[10px] ${active ? "opacity-70" : "text-subtle"}`}
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
