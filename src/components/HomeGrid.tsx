import React from "react";
import {
  Plus,
  RefreshCw,
  Settings,
  Eye,
  EyeOff,
  LayoutGrid,
  HardDrive,
  Database,
  Zap,
} from "lucide-react";
import { SearchResult, SystemStats } from "../types";
import { FileTypeIcon } from "./FileTypeIcon";
import { Sparkline } from "./Sparkline";
import { formatBytes, formatComma, formatRelativeTime } from "../lib/format";

/* Home / empty-query bento (12-col grid):
   Recent files (8) · Index health (4) · Quick actions (4) ·
   Storage (4) · Stats mini (4). */

interface HomeGridProps {
  recent: SearchResult[];
  stats: SystemStats | null;
  statsLive: boolean;
  onOpenFile: (path: string) => void;
  onSelectResult: (result: SearchResult) => void;
  onAddFolder: () => void;
  onReindex: () => void;
  onOpenSettings: () => void;
  onToggleView: () => void;
  view: "list" | "grid";
  onTogglePreview: () => void;
  previewOpen: boolean;
  onRunQuery: (query: string) => void;
}

function BentoCard({
  label,
  className,
  children,
  onClick,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
  onClick?: () => void;
}) {
  return (
    <section
      className={`rounded-[14px] border border-line bg-elevated p-5 ${className ?? ""} ${
        onClick ? "cursor-pointer transition-colors duration-150 hover:border-line-strong" : ""
      }`}
      onClick={onClick}
    >
      <p className="eyebrow pb-3 text-center">{label}</p>
      {children}
    </section>
  );
}

function MiniBar({ fraction, color }: { fraction: number; color: string }) {
  return (
    <div className="h-1 w-full overflow-hidden rounded-full bg-sunken">
      <div
        className="h-full rounded-full transition-[width] duration-500"
        style={{
          width: `${Math.max(2, Math.min(100, fraction * 100))}%`,
          background: color,
        }}
      />
    </div>
  );
}

const Dyad: React.FC<{ k: string; v: string; tone?: string }> = ({ k, v, tone }) => (
  <div className="flex items-baseline justify-between gap-2">
    <span className="text-[11px] text-muted">{k}</span>
    <span className={`font-mono text-[12px] ${tone ?? "text-fg"}`}>{v}</span>
  </div>
);

export function HomeGrid({
  recent,
  stats,
  statsLive,
  onOpenFile,
  onSelectResult,
  onAddFolder,
  onReindex,
  onOpenSettings,
  onToggleView,
  view,
  onTogglePreview,
  previewOpen,
  onRunQuery,
}: HomeGridProps) {
  const idx = stats?.index;
  const queries = stats?.queries;

  const storageTotal =
    (idx ? idx.tantivy_bytes + idx.metadata_db_bytes + idx.vector_bytes : 0) || 0;

  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="grid grid-cols-12 gap-3">
        {/* Recent files — span 8 */}
        <BentoCard label="Recent files" className="col-span-8 min-h-0">
          {recent.length === 0 ? (
            <div className="flex h-32 items-center justify-center">
              <p className="font-serif text-[15px] italic text-subtle">Nothing archived yet</p>
            </div>
          ) : (
            <div className="space-y-0.5">
              {recent.slice(0, 5).map((r, i) => (
                <button
                  key={r.path + i}
                  type="button"
                  onClick={() => {
                    if (r.is_dir) {
                      onSelectResult(r);
                    } else {
                      onOpenFile(r.path);
                    }
                  }}
                  className="flex h-9 w-full items-center gap-2.5 rounded-[10px] px-2 text-left transition-colors duration-150 hover:bg-sunken"
                >
                  <FileTypeIcon result={r} size={22} />
                  <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-fg">
                    {r.filename}
                  </span>
                  <span className="shrink-0 font-mono text-[10px] text-subtle">
                    {formatRelativeTime(r.modified)}
                  </span>
                </button>
              ))}
            </div>
          )}
        </BentoCard>

        {/* Index health — span 4 */}
        <BentoCard label="Index health" className="col-span-4">
          <div className="space-y-2">
            <Dyad k="Files" v={idx ? formatComma(idx.files) : "—"} />
            <Dyad k="Folders" v={idx ? formatComma(idx.dirs) : "—"} />
            <Dyad k="Docs" v={idx ? formatComma(idx.content_docs) : "—"} />
            <Dyad
              k="Semantic units"
              v={idx ? formatComma(idx.semantic_units) : "—"}
              tone={idx && idx.semantic_units > 0 ? "text-accent" : undefined}
            />
            <Dyad k="Index size" v={idx ? formatBytes(storageTotal) : "—"} />
          </div>
          {!statsLive && (
            <p className="mt-3 font-mono text-[10px] text-subtle">
              stats land when the backend ships get_stats
            </p>
          )}
        </BentoCard>

        {/* Quick actions — span 4 */}
        <BentoCard label="Quick actions" className="col-span-4">
          <div className="space-y-0.5">
            <ActionRow
              icon={<Plus className="h-4 w-4" />}
              label="Add folder"
              hint="Watch a directory"
              onClick={onAddFolder}
            />
            <ActionRow
              icon={<RefreshCw className="h-4 w-4" />}
              label="Reindex"
              hint="Full re-scan"
              onClick={onReindex}
            />
            <ActionRow
              icon={<Settings className="h-4 w-4" />}
              label="Settings"
              hint="⌘,"
              onClick={onOpenSettings}
            />
            <ActionRow
              icon={view === "list" ? <LayoutGrid className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              label={view === "list" ? "Grid view" : "List view"}
              hint="Change layout"
              onClick={onToggleView}
            />
            <ActionRow
              icon={previewOpen ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              label={previewOpen ? "Hide preview" : "Show preview"}
              hint="Tab"
              onClick={onTogglePreview}
            />
          </div>
        </BentoCard>

        {/* Storage — span 4 */}
        <BentoCard label="Storage" className="col-span-4">
          {idx ? (
            <div className="space-y-3">
              <StorageRow
                icon={<SearchGlyph />}
                label="tantivy (text)"
                bytes={idx.tantivy_bytes}
                total={storageTotal}
                color="var(--olive)"
              />
              <StorageRow
                icon={<Database className="h-3.5 w-3.5" />}
                label="vectors (i8)"
                bytes={idx.vector_bytes}
                total={storageTotal}
                color="var(--accent)"
              />
              <StorageRow
                icon={<HardDrive className="h-3.5 w-3.5" />}
                label="metadata (sqlite)"
                bytes={idx.metadata_db_bytes}
                total={storageTotal}
                color="var(--blue)"
              />
            </div>
          ) : (
            <div className="flex h-24 items-center justify-center">
              <p className="font-serif text-[15px] italic text-subtle">The ledger, unweighed</p>
            </div>
          )}
        </BentoCard>

        {/* Stats mini — span 4 */}
        <BentoCard label="Query latency" className="col-span-4">
          <div className="flex items-center gap-4">
            <div className="flex-1 space-y-2">
              <div>
                <p className="font-mono text-[22px] leading-7 text-fg">
                  {queries ? queries.p50_ms.toFixed(1) : "—"}
                  <span className="ml-1 text-[11px] text-subtle">ms</span>
                </p>
                <p className="text-[10px] uppercase tracking-wide text-subtle">p50</p>
              </div>
              <div>
                <p className="font-mono text-[22px] leading-7 text-fg">
                  {queries ? queries.p95_ms.toFixed(1) : "—"}
                  <span className="ml-1 text-[11px] text-subtle">ms</span>
                </p>
                <p className="text-[10px] uppercase tracking-wide text-subtle">p95</p>
              </div>
            </div>
            <div className="flex flex-col items-end gap-2">
              <Sparkline values={queries?.history ?? []} width={96} height={40} />
              {queries && (
                <p className="flex items-center gap-1 font-mono text-[10px] text-subtle">
                  <Zap className="h-3 w-3 text-accent" />
                  {formatComma(queries.total)} queries
                </p>
              )}
            </div>
          </div>
        </BentoCard>

        {/* Search syntax — span 12, teaches the query language and fills the fold */}
        <BentoCard label="Search syntax" className="col-span-12">
          <div className="flex flex-wrap items-center gap-1.5">
            {[
              "type:pdf",
              "in:~/Projects",
              "modified:<7d",
              "size:>2mb",
              "is:image",
              "name:config",
              "-node_modules",
              're:"fn \\w+"',
            ].map((f) => (
              <button
                key={f}
                type="button"
                onClick={() => onRunQuery(f)}
                className="kbd transition-colors duration-150 hover:text-fg hover:border-line-strong"
                title={`Run ${f}`}
              >
                {f}
              </button>
            ))}
            <span className="ml-1 text-[11px] text-muted">
              Plain typing always works — filters only narrow the set.
            </span>
          </div>
        </BentoCard>
      </div>
    </div>
  );
}

function ActionRow({
  icon,
  label,
  hint,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  hint: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={hint}
      className="group flex h-9 w-full items-center gap-2.5 rounded-[10px] px-2 text-left transition-colors duration-150 hover:bg-sunken"
    >
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-sunken text-muted transition-colors duration-150 group-hover:text-fg">
        {icon}
      </span>
      <span className="min-w-0 flex-1 truncate text-[12px] text-fg">{label}</span>
    </button>
  );
}

function StorageRow({
  icon,
  label,
  bytes,
  total,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  bytes: number;
  total: number;
  color: string;
}) {
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-[11px]">
        <span className="flex items-center gap-1.5 text-muted">
          {icon}
          {label}
        </span>
        <span className="font-mono text-fg">{formatBytes(bytes)}</span>
      </div>
      <MiniBar fraction={total > 0 ? bytes / total : 0} color={color} />
    </div>
  );
}

function SearchGlyph() {
  return (
    <svg viewBox="0 0 16 16" className="h-3.5 w-3.5" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="7" cy="7" r="4.5" />
      <line x1="10.5" y1="10.5" x2="13.5" y2="13.5" />
    </svg>
  );
}

