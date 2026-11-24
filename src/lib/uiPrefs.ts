/* UI preferences that are applied through data attributes and layout numbers.
   Kept in one place so Settings, App and VirtualGrid agree on the values. */

export type MotionLevel = "full" | "subtle" | "off";
export type GridDensity = "compact" | "comfortable" | "spacious";

export function normalizeMotion(raw: string | null | undefined): MotionLevel {
  switch ((raw ?? "").trim().toLowerCase()) {
    case "subtle":
      return "subtle";
    case "off":
    case "none":
    case "reduced":
      return "off";
    default:
      return "full";
  }
}

export function normalizeDensity(raw: string | null | undefined): GridDensity {
  switch ((raw ?? "").trim().toLowerCase()) {
    case "compact":
      return "compact";
    case "spacious":
      return "spacious";
    default:
      return "comfortable";
  }
}

/** Drives the motion-level CSS in App.css. */
export function applyMotion(raw: string | null | undefined): MotionLevel {
  const level = normalizeMotion(raw);
  document.documentElement.dataset.motion = level;
  return level;
}

export interface GridMetrics {
  /** Gap between grid cells. */
  gap: number;
  /** Side gutter. */
  pad: number;
  /** Top/bottom inset. */
  padY: number;
  /** Minimum cell width; governs the column count. */
  minCell: number;
  listGap: number;
  listPad: number;
  listRow: number;
}

export const GRID_METRICS: Record<GridDensity, GridMetrics> = {
  compact: { gap: 10, pad: 14, padY: 6, minCell: 152, listGap: 2, listPad: 8, listRow: 48 },
  comfortable: { gap: 16, pad: 20, padY: 10, minCell: 168, listGap: 4, listPad: 12, listRow: 56 },
  spacious: { gap: 20, pad: 26, padY: 14, minCell: 184, listGap: 6, listPad: 16, listRow: 64 },
};

export function densityMetrics(raw: string | null | undefined): GridMetrics {
  return GRID_METRICS[normalizeDensity(raw)];
}
