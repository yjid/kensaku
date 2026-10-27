import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatBytes, formatRelativeTime } from "../lib/format";
import type { SearchResult } from "../types";

/* Hover info. A slim bar rises from the card's bottom edge on hover: the
   modified date, the size, and image dimensions when known. It fades in
   after a short pause and never covers the thumbnail. Dimensions are read
   on hover only, never during search. Right-click keeps the full menu. */

interface Details {
  width?: number | null;
  height?: number | null;
}

export function HoverDetails({ result }: { result: SearchResult }) {
  const [details, setDetails] = useState<Details>({});

  useEffect(() => {
    if (result.is_dir) return;
    let cancelled = false;
    invoke<Details>("get_file_details", { path: result.path })
      .then((d) => {
        if (!cancelled) setDetails(d);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [result.path, result.is_dir]);

  const parts = [
    result.is_dir ? "Folder" : formatBytes(result.file_size),
    details.width && details.height ? `${details.width}×${details.height}` : null,
    formatRelativeTime(result.modified),
  ].filter(Boolean) as string[];

  return (
    <span
      aria-hidden="true"
      className="hover-reveal pointer-events-none absolute inset-x-0 bottom-0 z-20 flex flex-col gap-0.5 rounded-b-[16px] bg-elevated/95 px-3 pb-2.5 pt-2 backdrop-blur-md"
    >
      <span className="truncate font-mono text-[11px] leading-[14px] text-muted">
        {parts.join("  ·  ")}
      </span>
    </span>
  );
}
