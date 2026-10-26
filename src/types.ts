/* ---- get_snippets DTO. Snippets are fetched in a second phase after ---- */
/* ---- results land; `null` means "no snippet for this path".          ---- */

export interface SnippetDto {
  path: string;
  snippet: string | null;
}

export interface SearchResult {
  path: string;
  filename: string;
  snippet: string | null;
  score: number;
  file_size: number;
  modified: number;
  is_dir: boolean;
  match_type: "FileName" | "Content" | "Semantic" | "Both";
  best_range?: [number, number] | null;
}

export interface ActiveFilter {
  raw: string;
  filter_type: string;
  value: string;
  negated: boolean;
}

export interface SearchResponse {
  results: SearchResult[];
  seq: number;
  active_filters: ActiveFilter[];
  hints: string[];
  search_time_ms: number;
  /** Matches before the display limit (collected count for exhaustive mode). */
  total?: number;
  /** "ranked" or "exhaustive". */
  mode?: string;
}

export interface FilePreview {
  path: string;
  filename: string;
  file_size: number;
  modified: number;
  is_dir: boolean;
  preview_type: "text" | "image" | "pdf" | "doc" | "presentation" | "sheet" | "dir" | "binary";
  content?: string | null;
  total_lines?: number | null;
  child_count?: number | null;
  /** Pixel dimensions, image previews only — best-effort from the backend. */
  width?: number | null;
  height?: number | null;
}

export interface DuplicateGroup {
  kind: "content" | "image" | string;
  hash: string;
  paths: string[];
}

export interface CommandOutput {
  success: boolean;
  message: string;
}

export interface IndexingStatus {
  is_indexing: boolean;
  files_indexed: number;
  content_indexed: number;
  current_status: string;
  is_embedding?: boolean;
  units_indexed?: number;
  semantic_ready?: boolean;
  phase?: "idle" | "walking" | "extracting" | "embedding" | "committing";
}

export interface KensakuConfig {
  watched_dirs: string[];
  excluded_dirs: string[];
  excluded_extensions: string[];
  shortcut: string;
  max_results: number;
  is_setup_complete: boolean;
  semantic_enabled?: boolean;
  semantic_model?: string;
  skip_hidden?: boolean;
  ocr_enabled?: boolean;
  theme?: "paper" | "void";
  config_version?: number;
}

/* ---- OCR (get_ocr_status / download_ocr_language). The backend may ---- */
/* ---- not expose these yet; consumers must degrade to "unknown".     ---- */

export interface OcrStatus {
  engine_present: boolean;
  languages: string[];
  /** Engine present AND at least one language data file on disk. */
  available: boolean;
  tessdata_dir: string;
  error: string | null;
}

/* ---- SystemStats (get_stats). The backend may not expose this yet; ---- */
/* ---- all consumers must degrade gracefully to zeroed fallbacks.      ---- */

export interface QueryStats {
  total: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  last_ms: number;
  history: number[];
}

export interface IndexStats {
  files: number;
  dirs: number;
  content_docs: number;
  semantic_units: number;
  metadata_db_bytes: number;
  tantivy_bytes: number;
  vector_bytes: number;
  last_reconcile_secs_ago: number | null;
}

export type IngestPhase =
  | "idle"
  | "walking"
  | "extracting"
  | "embedding"
  | "committing";

export interface IngestionStats {
  is_indexing: boolean;
  phase: IngestPhase;
  files_indexed: number;
  content_indexed: number;
  units_indexed: number;
  files_per_sec: number;
  eta_secs: number | null;
  embed_queue_depth: number;
}

export interface RuntimeStats {
  uptime_secs: number;
  rss_bytes: number;
  watcher_ok: boolean;
  semantic_ready: boolean;
  model_id: string;
}

export interface SystemStats {
  queries: QueryStats;
  index: IndexStats;
  ingestion: IngestionStats;
  runtime: RuntimeStats;
}

/* ---- Model files (get_models_status / download_models / delete_models). ---- */
/* ---- The backend may not expose these yet; consumers degrade gracefully. ---- */

export interface ModelArtifact {
  name: string;
  url: string;
  bytes_expected: number;
  bytes_on_disk: number;
  present: boolean;
}

export interface ModelProgress {
  file: string;
  downloaded: number;
  total: number;
}

export interface ModelsStatus {
  dir: string;
  ready: boolean;
  downloading: boolean;
  progress: ModelProgress | null;
  error: string | null;
  artifacts: ModelArtifact[];
  total_bytes: number;
  total_present_bytes: number;
}

/* ---- Doctor (run_doctor). One-shot diagnostics; never auto-blocks UI. ---- */

export interface DoctorCheck {
  id: string;
  label: string;
  status: "ok" | "warn" | "fail";
  detail: string;
  fix: string | null;
}

export interface DoctorReport {
  checks: DoctorCheck[];
  ran_at: number;
  summary: { ok: number; warn: number; fail: number };
}

/** Fully quantified, safe-to-render fallback for when `get_stats` is missing. */
export function emptyStats(): SystemStats {
  return {
    queries: { total: 0, p50_ms: 0, p95_ms: 0, p99_ms: 0, last_ms: 0, history: [] },
    index: {
      files: 0,
      dirs: 0,
      content_docs: 0,
      semantic_units: 0,
      metadata_db_bytes: 0,
      tantivy_bytes: 0,
      vector_bytes: 0,
      last_reconcile_secs_ago: null,
    },
    ingestion: {
      is_indexing: false,
      phase: "idle",
      files_indexed: 0,
      content_indexed: 0,
      units_indexed: 0,
      files_per_sec: 0,
      eta_secs: null,
      embed_queue_depth: 0,
    },
    runtime: {
      uptime_secs: 0,
      rss_bytes: 0,
      watcher_ok: false,
      semantic_ready: false,
      model_id: "",
    },
  };
}
