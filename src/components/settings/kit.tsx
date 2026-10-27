import React from "react";

/* Shared primitives for the Settings pages and elsewhere: 1px-bordered
   cards, field rows, switches, badges. Tokens only. */

/** Tauri rejections are usually plain strings; extract a readable reason. */
export function reasonFrom(err: unknown): string {
  if (typeof err === "string" && err) return err;
  if (err instanceof Error && err.message) return err.message;
  try {
    const text = JSON.stringify(err);
    if (text && text !== "{}" && text !== "null") return text;
  } catch {
    /* non-serializable — fall through */
  }
  return "unknown error";
}

export function Field({
  label,
  description,
  hint,
  children,
}: {
  label: string;
  description?: string;
  /** Quiet sub-line under the description, shown only when relevant. */
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-line px-4 py-3 last:border-b-0">
      <div className="min-w-0 space-y-0.5">
        <p className="text-[13px] font-medium text-fg">{label}</p>
        {description && (
          <p className="line-clamp-2 text-[11px] leading-relaxed text-muted">{description}</p>
        )}
        {hint && (
          <p className="line-clamp-2 text-[11px] leading-relaxed text-muted">{hint}</p>
        )}
      </div>
      {children}
    </div>
  );
}

export function Card({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <div className="mb-3">
      {title && <p className="eyebrow pb-2">{title}</p>}
      <div className="overflow-hidden rounded-[12px] border border-line bg-elevated">{children}</div>
    </div>
  );
}

/** Section intro block: 15px semibold title + one-line 11px muted description. */
export function SectionHeader({ title, description }: { title: string; description: string }) {
  return (
    <div className="pb-4">
      <h2 className="text-[15px] font-semibold leading-snug text-fg">{title}</h2>
      <p className="line-clamp-1 pt-0.5 text-[11px] text-muted">{description}</p>
    </div>
  );
}

export function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="rounded-full border border-line px-2.5 py-1 font-mono text-[11px] text-muted">
      {children}
    </span>
  );
}

export function Switch({
  checked,
  disabled,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 shrink-0 rounded-full border transition-colors duration-150 ${
        checked ? "border-accent bg-accent" : "border-line bg-sunken"
      } ${disabled ? "cursor-not-allowed opacity-40" : "hover:border-line-strong"}`}
    >
      <span
        className={`absolute top-[1px] h-4 w-4 rounded-full shadow-sm transition-all duration-150 ${
          checked ? "left-[18px] bg-bg" : "left-[2px] bg-subtle"
        }`}
      />
    </button>
  );
}

export function Stepper({
  value,
  min,
  max,
  step,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  return (
    <div className="flex h-8 items-center rounded-[8px] border border-line bg-sunken">
      <button
        type="button"
        onClick={() => onChange(clamp(value - step))}
        className="flex h-full w-8 items-center justify-center text-muted transition-colors hover:text-fg"
        aria-label="Decrease"
      >
        −
      </button>
      <span className="w-12 text-right font-mono text-[12px] text-fg">{value}</span>
      <button
        type="button"
        onClick={() => onChange(clamp(value + step))}
        className="flex h-full w-8 items-center justify-center text-muted transition-colors hover:text-fg"
        aria-label="Increase"
      >
        +
      </button>
    </div>
  );
}
