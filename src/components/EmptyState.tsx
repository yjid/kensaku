import React from "react";
import { queryTerms, highlightSegments } from "../lib/format";

/* Inline-SVG empty-state illustrations — isometric folder cabinet in the
   editorial style: 1.5px ink stroke, muted fills, offset shadow shape.
   Uses theme tokens only. Two variants: empty query and no results. */

type Variant = "empty" | "noresults";

const Stroked: React.FC<{ d: string; fill: string; fillOpacity: number; strokeOpacity?: number }> = ({
  d,
  fill,
  fillOpacity,
  strokeOpacity = 1,
}) => (
  <path
    d={d}
    fill={fill}
    fillOpacity={fillOpacity}
    stroke="var(--fg)"
    strokeOpacity={strokeOpacity}
    strokeWidth={1.5}
    strokeLinejoin="round"
    strokeLinecap="round"
  />
);

const FolderMark: React.FC<{ variant: Variant }> = ({ variant }) => {
  const teal = "var(--accent)";
  const pink = "var(--pink)";
  const olive = "var(--olive)";
  const blue = "var(--blue)";

  return (
    <svg width="180" height="140" viewBox="0 0 180 140" fill="none" aria-hidden="true">
      {/* offset shadow shape */}
      <path
        d="M34 74 L150 74 L134 116 Q132 122 126 122 L40 122 Q34 122 33 116 Z"
        fill="var(--fg)"
        fillOpacity={0.06}
        transform="translate(6 6)"
      />
      {/* papers peeking out */}
      <g stroke="var(--fg)" strokeOpacity={0.85} strokeWidth={1.5} strokeLinejoin="round">
        <rect x="56" y="42" width="24" height="34" rx="2" fill={olive} fillOpacity={0.42} transform="rotate(-6 68 59)" />
        <rect x="88" y="40" width="24" height="34" rx="2" fill={blue} fillOpacity={0.42} transform="rotate(3 100 57)" />
        <rect x="112" y="44" width="24" height="30" rx="2" fill={teal} fillOpacity={0.42} transform="rotate(8 124 59)" />
      </g>
      {/* folder back panel */}
      <Stroked
        d="M30 66 L30 40 Q30 33 37 33 L72 33 L84 48 L143 48 Q150 48 150 55 L150 66 Z"
        fill={teal}
        fillOpacity={0.36}
      />
      {/* drawer rails */}
      <g stroke="var(--fg)" strokeOpacity={0.25} strokeWidth={1.25} strokeLinecap="round">
        <line x1="18" y1="66" x2="30" y2="66" />
        <line x1="150" y1="66" x2="162" y2="66" />
      </g>
      {/* folder front panel */}
      <Stroked
        d="M22 66 L158 66 L141 114 Q139 120 133 120 L41 120 Q35 120 34 114 Z"
        fill={pink}
        fillOpacity={0.34}
      />
      {/* label plate */}
      <rect x="62" y="88" width="44" height="12" rx="3" fill="var(--bg)" fillOpacity={0.5} stroke="var(--fg)" strokeOpacity={0.5} strokeWidth={1.25} />

      {variant === "noresults" && (
        <g>
          {/* magnifier over the folder */}
          <circle
            cx="138"
            cy="30"
            r="17"
            fill={blue}
            fillOpacity={0.18}
            stroke="var(--fg)"
            strokeWidth={1.5}
            strokeOpacity={0.9}
          />
          {/* empty field inside the lens: a few faint tick marks */}
          <g stroke="var(--fg)" strokeOpacity={0.2} strokeWidth={1.25} strokeLinecap="round">
            <line x1="128" y1="26" x2="148" y2="26" />
            <line x1="128" y1="31" x2="144" y2="31" />
            <line x1="128" y1="36" x2="142" y2="36" />
          </g>
          <line
            x1="150"
            y1="44"
            x2="162"
            y2="56"
            stroke="var(--fg)"
            strokeWidth={2.5}
            strokeLinecap="round"
          />
        </g>
      )}

      {variant === "empty" && (
        <g stroke="var(--fg)" strokeOpacity={0.85} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round">
          {/* a single index card propped against the folder */}
          <rect x="128" y="82" width="34" height="40" rx="3" fill={teal} fillOpacity={0.42} transform="rotate(7 145 102)" />
          <line x1="136" y1="95" x2="156" y2="98" stroke="var(--fg)" strokeOpacity={0.4} strokeWidth={1.25} />
          <line x1="135" y1="100" x2="155" y2="103" stroke="var(--fg)" strokeOpacity={0.4} strokeWidth={1.25} />
        </g>
      )}
    </svg>
  );
};

interface EmptyStateProps {
  variant: Variant;
  caption: string;
  children?: React.ReactNode;
}

export const EmptyState: React.FC<EmptyStateProps> = ({ variant, caption, children }) => (
  <div className="flex h-full flex-col items-center justify-center gap-1 p-8 text-center select-none">
    <FolderMark variant={variant} />
    <p className="font-serif text-[17px] italic text-muted mt-1">{caption}</p>
    {children && <div className="mt-3">{children}</div>}
  </div>
);

/** Subtle match highlight inside a filename, per interaction spec.
    No truncation here — the wrapping parent owns ellipsis/clamping. */
export const HighlightedName: React.FC<{ text: string; query: string }> = ({
  text,
  query,
}) => {
  const segs = highlightSegments(text, queryTerms(query));
  return (
    <span>
      {segs.map((s, i) =>
        s.match ? (
          <mark key={i} className="rounded-sm bg-accent-soft text-fg px-0.5 -mx-0.5">
            {s.text}
          </mark>
        ) : (
          <span key={i}>{s.text}</span>
        ),
      )}
    </span>
  );
};
