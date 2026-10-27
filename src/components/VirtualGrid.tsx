import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

/* Windowed square-card grid for infinite results.

   Only the rows inside the viewport (plus a small overscan) are mounted, so
   a list of 50k matches costs the same as a list of 50. Columns are derived
   from the container width and the minimum card width, so no per-item
   measuring happens. Cards are square, so every row height is known up
   front: cell width + gap.

   When the user scrolls within `prefetchRows` of the last loaded row, the
   next page is requested once, guarded by `loading`. */

const MIN_CELL = 156;
const GAP = 12;
const PAD = 16;
const OVERSCAN_ROWS = 3;

export interface VirtualGridProps<T> {
  items: T[];
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => React.ReactNode;
  /** Index of the keyboard-selected item; the grid scrolls it into view. */
  selectedIndex: number;
  hasMore: boolean;
  loading: boolean;
  onNearEnd: () => void;
  /** Reports the current column count so keyboard navigation can use it. */
  onColumns?: (cols: number) => void;
  prefetchRows?: number;
  containerRef?: React.RefObject<HTMLDivElement | null>;
}

export function VirtualGrid<T>({
  items,
  getKey,
  renderItem,
  selectedIndex,
  hasMore,
  loading,
  onNearEnd,
  onColumns,
  prefetchRows = 4,
  containerRef,
}: VirtualGridProps<T>) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const setRefs = useCallback(
    (el: HTMLDivElement | null) => {
      scrollRef.current = el;
      if (containerRef) (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
    },
    [containerRef],
  );

  const [size, setSize] = useState({ width: 0, height: 0 });
  const [scrollTop, setScrollTop] = useState(0);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const inner = Math.max(0, size.width - PAD * 2);
  const cols = Math.max(1, Math.floor((inner + GAP) / (MIN_CELL + GAP)));
  const cell = cols > 0 ? (inner - GAP * (cols - 1)) / cols : MIN_CELL;
  const rowH = cell + GAP;
  const totalRows = Math.ceil(items.length / cols);
  const contentHeight = totalRows > 0 ? totalRows * rowH - GAP + PAD * 2 : 0;

  useEffect(() => {
    onColumns?.(cols);
  }, [cols, onColumns]);

  // Visible row window.
  const firstVisible = Math.floor(Math.max(0, scrollTop - PAD) / rowH);
  const lastVisible = Math.ceil((scrollTop - PAD + size.height) / rowH);
  const startRow = Math.max(0, firstVisible - OVERSCAN_ROWS);
  const endRow = Math.min(totalRows - 1, lastVisible + OVERSCAN_ROWS);

  // Infinite: request the next page when the bottom of the loaded list is near.
  useEffect(() => {
    if (!hasMore || loading || totalRows === 0) return;
    if (endRow >= totalRows - prefetchRows) onNearEnd();
  }, [endRow, totalRows, hasMore, loading, prefetchRows, onNearEnd]);

  // Keep the keyboard selection on screen.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el || selectedIndex < 0 || cols === 0) return;
    const row = Math.floor(selectedIndex / cols);
    const top = PAD + row * rowH;
    const bottom = top + cell;
    if (top < el.scrollTop) el.scrollTop = top - PAD;
    else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight + PAD;
  }, [selectedIndex, cols, rowH, cell]);

  // Scroll updates are cheap: state only changes when the window moves.
  const onScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const next = e.currentTarget.scrollTop;
    setScrollTop((prev) => (Math.abs(prev - next) >= rowH / 2 ? next : prev));
  };

  const rows: React.ReactNode[] = [];
  for (let row = startRow; row <= endRow; row++) {
    const cells: React.ReactNode[] = [];
    for (let col = 0; col < cols; col++) {
      const index = row * cols + col;
      if (index >= items.length) break;
      const item = items[index];
      cells.push(
        <div key={getKey(item, index)} data-index={index} className="min-w-0">
          {renderItem(item, index)}
        </div>,
      );
    }
    rows.push(
      <div
        key={row}
        className="grid"
        style={{
          gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
          gap: GAP,
          height: cell,
          position: "absolute",
          left: PAD,
          right: PAD,
          top: PAD + row * rowH,
        }}
      >
        {cells}
      </div>,
    );
  }

  return (
    <div ref={setRefs} onScroll={onScroll} className="relative min-h-0 flex-1 overflow-y-auto">
      <div style={{ height: contentHeight, position: "relative" }}>{rows}</div>
      {loading && (
        <div className="pointer-events-none sticky bottom-3 flex justify-center">
          <span className="fade-in rounded-full border border-line bg-elevated px-3 py-1 text-[11px] text-muted shadow-sm">
            Loading more
          </span>
        </div>
      )}
    </div>
  );
}
