import React from "react";
import { ActiveFilter } from "../types";
import { X, Filter as FilterIcon, Folder, AlertCircle } from "lucide-react";

interface FilterChipsProps {
  filters: ActiveFilter[];
  hints: string[];
  onRemoveFilter: (filter: ActiveFilter) => void;
  onApplyHint: (hint: string) => void;
}

export const FilterChips: React.FC<FilterChipsProps> = ({
  filters,
  hints,
  onRemoveFilter,
  onApplyHint,
}) => {
  if (filters.length === 0 && hints.length === 0) {
    return null;
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5 px-4 py-2 border-b border-white/5 bg-black/20 text-xs">
      {filters.length > 0 && (
        <div className="flex items-center gap-1 text-slate-500 mr-1 text-[11px]">
          <FilterIcon className="w-3 h-3" />
          <span>Filters:</span>
        </div>
      )}

      {filters.map((filter, idx) => (
        <span
          key={filter.raw + idx}
          className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md border text-[11px] font-mono transition-colors ${
            filter.negated
              ? "bg-rose-500/10 text-rose-300 border-rose-500/20"
              : filter.filter_type === "in"
              ? "bg-amber-500/10 text-amber-300 border-amber-500/20"
              : filter.filter_type === "type" || filter.filter_type === "ext"
              ? "bg-blue-500/10 text-blue-300 border-blue-500/20"
              : "bg-purple-500/10 text-purple-300 border-purple-500/20"
          }`}
        >
          <span>{filter.raw}</span>
          <button
            type="button"
            title={`Remove ${filter.raw}`}
            onClick={(e) => {
              e.stopPropagation();
              onRemoveFilter(filter);
            }}
            className="hover:text-white rounded transition-colors"
          >
            <X className="w-3 h-3" />
          </button>
        </span>
      ))}

      {hints.map((hint, idx) => {
        const isDir = hint.includes("📁");
        return (
          <button
            key={hint + idx}
            type="button"
            onClick={() => onApplyHint(hint)}
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-white/5 hover:bg-white/10 text-slate-300 hover:text-white border border-white/10 text-[11px] transition-colors cursor-pointer"
          >
            {isDir ? <Folder className="w-3 h-3 text-amber-400" /> : <AlertCircle className="w-3 h-3 text-sky-400" />}
            <span>{hint}</span>
          </button>
        );
      })}
    </div>
  );
};
