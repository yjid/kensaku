import { useCallback, useEffect, useRef, useState } from "react";

/* Dialogs animate their exit before the parent unmounts them. `close`
   starts the exit class and calls the real onClose once it has played;
   repeated calls while closing are ignored. */
export function usePresence(onClose: () => void, exitMs = 140) {
  const [closing, setClosing] = useState(false);
  const closingRef = useRef(false);
  const timer = useRef<number | undefined>(undefined);

  const close = useCallback(() => {
    if (closingRef.current) return;
    closingRef.current = true;
    setClosing(true);
    timer.current = window.setTimeout(onClose, exitMs);
  }, [onClose, exitMs]);

  useEffect(
    () => () => {
      if (timer.current !== undefined) window.clearTimeout(timer.current);
    },
    [],
  );

  return { closing, close };
}
