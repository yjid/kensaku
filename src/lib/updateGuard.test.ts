import { beforeEach, describe, expect, it } from "vitest";
import {
  clearAttempt,
  classifyBoot,
  readAttempt,
  recordAttempt,
  UPDATE_ATTEMPT_KEY,
  UPDATE_ATTEMPT_TTL_MS,
} from "./updateGuard";

function memoryStore(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => {
      map.set(key, value);
    },
    removeItem: (key: string) => {
      map.delete(key);
    },
    keys: () => [...map.keys()],
  };
}

describe("update attempt record", () => {
  let store: ReturnType<typeof memoryStore>;

  beforeEach(() => {
    store = memoryStore();
  });

  it("round-trips the version and timestamp", () => {
    recordAttempt(store, "1.0.20", 1000);
    expect(readAttempt(store)).toEqual({ version: "1.0.20", at: 1000 });
  });

  it("ignores an empty version instead of writing a record nobody can match", () => {
    recordAttempt(store, "");
    expect(store.keys()).toEqual([]);
  });

  it("treats corrupt records as absent", () => {
    expect(readAttempt(memoryStore({ [UPDATE_ATTEMPT_KEY]: "{not json" }))).toBeNull();
    expect(readAttempt(memoryStore({ [UPDATE_ATTEMPT_KEY]: '{"version":"1.0.20"}' }))).toBeNull();
    expect(readAttempt(memoryStore({ [UPDATE_ATTEMPT_KEY]: '{"at":1}' }))).toBeNull();
    expect(readAttempt(memoryStore({ [UPDATE_ATTEMPT_KEY]: '{"version":"","at":1}' }))).toBeNull();
  });

  it("clears the record", () => {
    recordAttempt(store, "1.0.20", 1000);
    clearAttempt(store);
    expect(readAttempt(store)).toBeNull();
  });
});

describe("classifyBoot", () => {
  const now = 10_000_000;

  it("is none when no install was attempted", () => {
    expect(classifyBoot(null, "1.0.19", now)).toBe("none");
  });

  it("is landed when the recorded version is the one running", () => {
    expect(classifyBoot({ version: "1.0.20", at: now - 5_000 }, "1.0.20", now)).toBe("landed");
  });

  it("is failed when a recent attempt came back on another version", () => {
    expect(classifyBoot({ version: "1.0.21", at: now - 60_000 }, "1.0.20", now)).toBe("failed");
  });

  it("is stale once the record is too old to blame on the install", () => {
    expect(classifyBoot({ version: "1.0.21", at: now - UPDATE_ATTEMPT_TTL_MS - 1 }, "1.0.20", now)).toBe(
      "stale",
    );
  });

  it("treats a timestamp in the future as stale, not as a failure", () => {
    expect(classifyBoot({ version: "1.0.21", at: now + 60_000 }, "1.0.20", now)).toBe("stale");
  });

  it("still reports landed after the TTL, because the version matched", () => {
    expect(
      classifyBoot({ version: "1.0.20", at: now - UPDATE_ATTEMPT_TTL_MS - 1 }, "1.0.20", now),
    ).toBe("landed");
  });
});
