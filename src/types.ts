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
}

export interface FilePreview {
  path: string;
  filename: string;
  file_size: number;
  modified: number;
  is_dir: boolean;
  preview_type: "text" | "image" | "pdf" | "dir" | "binary";
  content?: string | null;
  total_lines?: number | null;
  child_count?: number | null;
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
}
