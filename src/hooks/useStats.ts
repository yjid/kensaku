import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SystemStats, emptyStats } from "../types";

interface UseStatsResult {
  stats: SystemStats;
  /** True when `get_stats` exists and returned live data at least once. */
  live: boolean;
  refresh: () => void;
}

/** A blocked `get_stats` must not pin the UI on "Waiting for backend". */
const STATS_TIMEOUT_MS = 5_000;

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(
      () => reject(new Error(`get_stats timed out after ${ms}ms`)),
      ms,
    );
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

/**
 * Polls `get_stats` at a given interval (ms) while mounted. The command may
 * not exist yet (backend lands it concurrently) — every failure falls back
 * to a zeroed placeholder so callers can render gracefully. Defaults to
 * polling; pass intervalMs={null} for a one-shot fetch.
 */
export function useStats(intervalMs: number | null = 2000): UseStatsResult {
  const [stats, setStats] = useState<SystemStats>(emptyStats);
  const [live, setLive] = useState(false);
  const mounted = useRef(false);
  const inFlight = useRef(false);

  const fetch = useCallback(async () => {
    // One request at a time. A large folder removal can hold the metadata
    // write lock for minutes; without this guard every 2s tick queued another
    // invocation and the panel never caught up.
    if (inFlight.current) return;
    inFlight.current = true;
    const request = invoke<SystemStats>("get_stats");
    // Keep the single-flight guard until the backend actually answers. The
    // 5s timeout only unblocks the UI; the original invoke can still be
    // waiting on the metadata lock, and a new poll must not pile up behind it.
    const release = () => {
      inFlight.current = false;
    };
    request.then(release, release);
    try {
      const res = await withTimeout(request, STATS_TIMEOUT_MS);
      if (!mounted.current) return;
      // Merge defensively — partial fields fall back to zeroes.
      const base = emptyStats();
      setStats({
        queries: { ...base.queries, ...res.queries },
        index: { ...base.index, ...res.index },
        ingestion: { ...base.ingestion, ...res.ingestion },
        runtime: { ...base.runtime, ...res.runtime },
      });
      setLive(true);
    } catch (err) {
      console.warn("get_stats not available:", err);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    fetch();
    if (intervalMs === null) {
      return () => {
        mounted.current = false;
      };
    }

    let id: number | null = null;
    const start = () => {
      if (id === null) id = window.setInterval(fetch, intervalMs);
    };
    const stop = () => {
      if (id !== null) {
        window.clearInterval(id);
        id = null;
      }
    };
    // Polling is for a visible panel. A backgrounded window kept asking for
    // stats every couple of seconds for as long as the health tab was open.
    const onVisibility = () => {
      if (document.hidden) {
        stop();
      } else {
        fetch();
        start();
      }
    };

    if (!document.hidden) start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      mounted.current = false;
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [fetch, intervalMs]);

  return { stats, live, refresh: fetch };
}
