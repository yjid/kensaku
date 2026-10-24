export interface SearchResult {
  path: string;
  filename: string;
  snippet: string | null;
  score: number;
  file_size: number;
  modified: number;
  is_dir: boolean;
  match_type: "FileName" | "Content" | "Both";
}

export interface IndexingStatus {
  is_indexing: boolean;
  files_indexed: number;
  content_indexed: number;
  current_status: string;
}

export interface KensakuConfig {
  watched_dirs: string[];
  excluded_dirs: string[];
  excluded_extensions: string[];
  shortcut: string;
  max_results: number;
  is_setup_complete: boolean;
}
