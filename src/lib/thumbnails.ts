import { useEffect, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

/* Thumbnail resolution shared by result rows, grid cards and the preview
   pane. The backend (`get_thumbnail`) returns an absolute cached path which
   is converted once per path into an asset URL. Everything degrades
   silently to the caller rendering the plain file icon. */

/** Extensions the backend thumbnailer handles. Everything else never asks. */
const THUMB_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff"]);

export const THUMBNAIL_LAZY_LIMIT = 30;

export function isThumbImage(filename: string): boolean {
  if (!filename.includes(".")) return false;
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return THUMB_EXTS.has(ext);
}

/* convertFileSrc is pure per path — memoize so every surface that renders
   the same thumbnail reuses the exact same asset URL. */
const srcCache = new Map<string, string>();

function assetSrc(thumbnailPath: string): string {
  let src = srcCache.get(thumbnailPath);
  if (src === undefined) {
    src = convertFileSrc(thumbnailPath);
    srcCache.set(thumbnailPath, src);
  }
  return src;
}

export interface ThumbnailState {
  /** Resolved asset URL, or null until the backend answers. */
  src: string | null;
  /** True once after a clean failure (missing command, backend error) —
      callers then fall back to the plain icon. Never logs. */
  failed: boolean;
}

/**
 * Resolves the cached thumbnail for one file path. Invoke-ray-light: a
 * single `get_thumbnail` per path per mount, cancelled with the effect.
 * `enabled=false` (non-image files, rows below the lazy limit) short-
 * circuits before any call is made.
 */
export function useThumbnail(path: string, enabled: boolean): ThumbnailState {
  const thumbable = enabled && isThumbImage(path);
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!thumbable) return;
    let cancelled = false;
    invoke<string>("get_thumbnail", { path })
      .then((abs) => {
        if (cancelled) return;
        if (abs) {
          setSrc(assetSrc(abs));
        } else {
          setFailed(true);
        }
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [path, thumbable]);

  return { src, failed };
}
