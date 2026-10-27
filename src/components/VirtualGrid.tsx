import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

/* Windowed results. Only rows inside the viewport (plus overscan) are mounted,
   so long result sets stay as light as short ones. Grid mode fits as many
   square-ish cards per row as the width allows. List mode is one column of
   fixed-height rows. Row heights are known up front, so nothing is measured
   per item. */

const MIN_CELL = 156;
const GAP = 16;
const PAD = 16;
const LIST_GAP = 2;
const LIST_PAD = 8;
const LIST_ROW = 56;
const OVERSCAN_ROWS = 4;

export interface VirtualGridProps<T> {
  items: T[];
  getKey: (item: T, index: number) => string;
  renderItem: (item: T, index: number) => React.ReactNode;
  selectedIndex: number;
  hasMore: boolean;
  loading: boolean;
  onNearEnd: () => void;
  /** Reports the column count so keyboard navigation can use it. */
  onColumns?: (cols: number) => void;
  prefetchRows?: number;
  /** Card height for a given cell width (grid mode). */
  cardHeight?: (cellWidth: number) => number;
  /** One column of fixed-height rows instead of a grid of cards. */
  list?: boolean;
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
  cardHeight = (w) => w,
  list = false,
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

  const pad = list ? LIST_PAD : PAD;
  const gap = list ? LIST_GAP : GAP;
  const inner = Math.max(0, size.width - pad * 2);
  const cols = list ? 1 : Math.max(1, Math.floor((inner + gap) / (MIN_CELL + gap)));
  const cell = list ? inner : (inner - gap * (cols - 1)) / cols;
  const cardH = list ? LIST_ROW : cardHeight(cell);
  const rowH = cardH + gap;
  const totalRows = Math.ceil(items.length / cols);
  const contentHeight = totalRows > 0 ? totalRows * rowH - gap + pad * 2 : 0;

  useEffect(() => {
    onColumns?.(cols);
  }, [cols, onColumns]);

  const firstVisible = Math.floor(Math.max(0, scrollTop - pad) / rowH);
  const lastVisible = Math.ceil((scrollTop - pad + size.height) / rowH);
  const startRow = Math.max(0, firstVisible - OVERSCAN_ROWS);
  const endRow = Math.min(totalRows - 1, lastVisible + OVERSCAN_ROWS);

  useEffect(() => {
    if (!hasMore || loading || totalRows === 0) return;
    if (endRow >= totalRows - prefetchRows) onNearEnd();
  }, [endRow, totalRows, hasMore, loading, prefetchRows, onNearEnd]);

  // Keep the keyboard selection on screen.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el || selectedIndex < 0 || cols === 0) return;
    const row = Math.floor(selectedIndex / cols);
    const top = pad + row * rowH;
    const bottom = top + cardH;
    if (top < el.scrollTop) el.scrollTop = top - pad;
    else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight + pad;
  }, [selectedIndex, cols, rowH, cardH, pad]);

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
          gap,
          height: cardH,
          position: "absolute",
          left: pad,
          right: pad,
          top: pad + row * rowH,
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
