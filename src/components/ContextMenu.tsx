import { useEffect, useRef } from "react";
import { usePresence } from "../lib/usePresence";

export interface ContextMenuItem {
  label: string;
  onSelect: () => void;
  separatorBefore?: boolean;
}

/**
 * Right-click menu for a result. Closes on outside click, Escape, scroll,
 * or after an item runs. Clamped to the viewport so it never clips.
 */
export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const { closing, close } = usePresence(onClose);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      }
    };
    const dismiss = () => close();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("scroll", dismiss, true);
    window.addEventListener("resize", dismiss);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("resize", dismiss);
    };
  }, [close]);

  const width = 200;
  const height = items.length * 36 + 12;
  const left = Math.min(x, window.innerWidth - width - 8);
  const top = Math.min(y, window.innerHeight - height - 8);

  return (
    <div
      ref={ref}
      role="menu"
      style={{ left, top, width }}
      className={`${closing ? "menu-pop-out pointer-events-none" : "menu-pop"} elevated fixed z-[70] overflow-hidden p-1`}
    >
      {items.map((item) => (
        <div key={item.label}>
          {item.separatorBefore && <div className="my-1 h-px bg-line" />}
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              close();
              item.onSelect();
            }}
            className="menu-row w-full text-left"
          >
            {item.label}
          </button>
        </div>
      ))}
    </div>
  );
}
