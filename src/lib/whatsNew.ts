import i18n from "../i18n";

/**
 * i18next treats dots as nesting, so a version cannot be used as a key
 * directly: "1.0.20" would look for an object at 1 -> 0 -> 20.
 */
export function whatsNewKey(version: string): string {
  return version.trim().replace(/\./g, "_");
}

/** Localized "what's new" bullets for the running version, if any are bundled. */
export function whatsNewItems(version: string): string[] {
  const key = whatsNewKey(version);
  if (!key) return [];
  const translate = i18n.getFixedT(null, "whatsNew");
  // The key is a dynamic version, which the typed resources cannot express.
  const value = translate(key as never, { returnObjects: true }) as unknown;
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}
