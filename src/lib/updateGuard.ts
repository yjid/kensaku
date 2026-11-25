/**
 * Records the version an install was started for, so the next launch can tell
 * "the update landed" from "the install never took".
 *
 * Without this, a failing install (wrong permissions, a signature the new
 * binary rejects, an installer that exits early) looks identical to a fresh
 * one: every launch re-downloads the same release, restarts, and comes back on
 * the old version. That loop is worse than no automatic updates at all, so one
 * failed attempt disables the automatic path for that session and leaves the
 * manual controls in Settings untouched.
 *
 * What it cannot catch: a build that crashes before any app code runs. Nothing
 * on the device can detect that, which is why the release itself is staged as a
 * pre-release first (docs/RELEASING.md).
 */
export interface UpdateAttempt {
  version: string;
  at: number;
}

export type AttemptOutcome = "none" | "landed" | "failed" | "stale";

export const UPDATE_ATTEMPT_KEY = "kensaku.updateAttempt";
/** An attempt older than this says nothing useful about the current build. */
export const UPDATE_ATTEMPT_TTL_MS = 24 * 60 * 60 * 1000;

type Store = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export function readAttempt(store: Store): UpdateAttempt | null {
  try {
    const raw = store.getItem(UPDATE_ATTEMPT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<UpdateAttempt> | null;
    if (typeof parsed?.version !== "string" || parsed.version === "") return null;
    if (typeof parsed.at !== "number" || !Number.isFinite(parsed.at)) return null;
    return { version: parsed.version, at: parsed.at };
  } catch {
    return null;
  }
}

export function recordAttempt(store: Store, version: string, now = Date.now()): void {
  if (!version) return;
  try {
    store.setItem(UPDATE_ATTEMPT_KEY, JSON.stringify({ version, at: now }));
  } catch {
    /* Storage can be unavailable; the guard is best-effort by design. */
  }
}

export function clearAttempt(store: Store): void {
  try {
    store.removeItem(UPDATE_ATTEMPT_KEY);
  } catch {
    /* See recordAttempt. */
  }
}

/**
 * Classifies a stored attempt against the version that just booted.
 *
 * - `landed`: the version we recorded is the one running — the update worked.
 * - `failed`: a different version is running and the attempt is recent.
 * - `stale`: a different version is running, but the record is too old to blame
 *   on a failed install (manual reinstall, a downgrade, clock skew).
 */
export function classifyBoot(
  attempt: UpdateAttempt | null,
  currentVersion: string,
  now = Date.now(),
): AttemptOutcome {
  if (!attempt) return "none";
  if (attempt.version === currentVersion) return "landed";
  if (now - attempt.at > UPDATE_ATTEMPT_TTL_MS) return "stale";
  if (now < attempt.at) return "stale";
  return "failed";
}
