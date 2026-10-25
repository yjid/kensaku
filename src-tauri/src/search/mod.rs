pub mod fuzzy;

use crate::embed::{ExactScanner, ModelRegistry, UnitStore};
use crate::indexer::content::ContentIndex;
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::FileEntry;
use crate::query::ast::{ActiveFilterDto, FileKind, Filter};
use crate::query::parse_query;
use crate::search::fuzzy::FuzzySearcher;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchType {
    FileName,
    Content,
    Semantic,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub filename: String,
    pub snippet: Option<String>,
    pub score: f64,
    pub file_size: u64,
    pub modified: u64,
    pub is_dir: bool,
    pub match_type: MatchType,
    pub best_range: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub seq: u64,
    pub active_filters: Vec<ActiveFilterDto>,
    pub hints: Vec<String>,
    pub search_time_ms: u64,
}

pub struct SearchDispatcher;

impl SearchDispatcher {
    pub fn search(
        query: &str,
        metadata: &MetadataStore,
        content_index: &Option<ContentIndex>,
        unit_store: Option<&UnitStore>,
        semantic_enabled: bool,
        limit: usize,
    ) -> Vec<SearchResult> {
        let resp = Self::search_response(query, metadata, content_index, unit_store, semantic_enabled, limit, 0);
        resp.results
    }

    pub fn search_response(
        raw_query: &str,
        metadata: &MetadataStore,
        content_index: &Option<ContentIndex>,
        unit_store: Option<&UnitStore>,
        semantic_enabled: bool,
        limit: usize,
        seq: u64,
    ) -> SearchResponse {
        let start_time = SystemTime::now();
        let trimmed = raw_query.trim();
        if trimmed.is_empty() {
            return SearchResponse {
                results: Vec::new(),
                seq,
                active_filters: Vec::new(),
                hints: Vec::new(),
                search_time_ms: 0,
            };
        }

        let now_sec = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let trie = metadata.get_directory_trie();
        let entries = metadata.get_entries();
        let path_index = metadata.get_path_index();
        let generation = metadata.generation();

        // 1. Parse AST from query
        let parsed = parse_query(raw_query, entries, path_index, Some(&trie), generation);
        let route = crate::query::router::route_query(&parsed);

        // 2. Resolve scopes and filter candidates
        let candidate_indices: Vec<usize> = if !parsed.scopes.is_empty() {
            let mut included = HashSet::new();
            let mut has_include_scope = false;

            for scope in &parsed.scopes {
                if scope.include {
                    has_include_scope = true;
                    let scope_candidates = trie.get_scope_candidates(scope.dir_id as usize, generation);
                    for &idx in scope_candidates.iter() {
                        included.insert(idx);
                    }
                }
            }

            let base_set: Vec<usize> = if has_include_scope {
                included.into_iter().collect()
            } else {
                (0..entries.len()).collect()
            };

            // Remove any excluded scopes
            let mut excluded = HashSet::new();
            for scope in &parsed.scopes {
                if !scope.include {
                    let scope_candidates = trie.get_scope_candidates(scope.dir_id as usize, generation);
                    for &idx in scope_candidates.iter() {
                        excluded.insert(idx);
                    }
                }
            }

            base_set
                .into_iter()
                .filter(|idx| !excluded.contains(idx))
                .filter(|&idx| {
                    if let Some(entry) = entries.get(idx) {
                        matches_all_filters(&parsed.filters, entry)
                    } else {
                        false
                    }
                })
                .collect()
        } else if !parsed.filters.is_empty() {
            (0..entries.len())
                .filter(|&idx| matches_all_filters(&parsed.filters, &entries[idx]))
                .collect()
        } else {
            (0..entries.len()).collect()
        };

        // Extract positive terms text
        let search_terms: Vec<&str> = parsed.terms.iter().map(|t| t.text.as_str()).collect();
        let search_text = search_terms.join(" ");
        let search_text_lower = search_text.to_lowercase();

        // Build a membership bitmap for O(1) candidate checks (replaces O(n) Vec::contains)
        let candidate_set: std::collections::HashSet<usize> = candidate_indices.iter().copied().collect();

        // 3. Tier 1: Fuzzy filename search (< 5ms)
        let mut fuzzy_ranks: HashMap<usize, usize> = HashMap::new();
        if !search_text.is_empty() {
            let mut fuzzy_searcher = FuzzySearcher::new();
            // Search across candidate entries
            let candidate_entries: Vec<FileEntry> = candidate_indices
                .iter()
                .filter_map(|&idx| entries.get(idx).cloned())
                .collect();

            let fuzzy_matches = fuzzy_searcher.search(&search_text, &candidate_entries, limit * 3);
            for (rank, (cand_rel_idx, _score)) in fuzzy_matches.into_iter().enumerate() {
                if let Some(&original_idx) = candidate_indices.get(cand_rel_idx) {
                    fuzzy_ranks.insert(original_idx, rank + 1);
                }
            }
        }

        // 4. Tier 2: Tantivy BM25 content search (< 50ms)
        let mut bm25_ranks: HashMap<PathBuf, usize> = HashMap::new();
        if !search_text.is_empty()
            && let Some(c_index) = content_index {
                let content_matches = c_index.search(&search_text, limit * 3);
                for (rank, cm) in content_matches.into_iter().enumerate() {
                    bm25_ranks.insert(cm.path, rank + 1);
                }
            }

        // 5. Tier 3: Static semantic vector scan (< 5ms)
        let mut semantic_ranks: HashMap<PathBuf, (usize, (usize, usize))> = HashMap::new();
        if semantic_enabled
            && !search_text.is_empty()
            && let Some(store) = unit_store
            && !store.is_empty()
        {
            let model = ModelRegistry::default();
            let query_vec = model.embed_to_i8(&search_text);
            let hits = ExactScanner::scan(&query_vec, store, limit * 3);
            for (rank, hit) in hits.into_iter().enumerate() {
                if let Some(entry) = metadata.get_by_node_id(hit.node_id)
                    && let Some(&orig_idx) = path_index.get(&entry.path)
                    && candidate_set.contains(&orig_idx)
                {
                    semantic_ranks.insert(
                        entry.path.clone(),
                        (rank + 1, (hit.best_unit.byte_start, hit.best_unit.byte_end)),
                    );
                }
            }
        }

        // 6. 3-tier RRF Fusion (k = 60.0) with dynamic route weights
        struct MatchCandidate {
            entry_idx: usize,
            in_fuzzy: bool,
            in_bm25: bool,
            in_semantic: bool,
            rrf_score: f64,
            best_range: Option<(usize, usize)>,
        }

        let mut candidates: HashMap<PathBuf, MatchCandidate> = HashMap::new();

        if search_text.is_empty() {
            // Filter-only / folder navigation browsing mode
            for &idx in &candidate_indices {
                if let Some(entry) = entries.get(idx) {
                    candidates.insert(
                        entry.path.clone(),
                        MatchCandidate {
                            entry_idx: idx,
                            in_fuzzy: true,
                            in_bm25: false,
                            in_semantic: false,
                            rrf_score: 0.1,
                            best_range: None,
                        },
                    );
                }
            }
        } else {
            let (w_fuzzy, w_bm25, w_semantic) = match route {
                crate::query::router::QueryRoute::NameCode => (1.3, 0.8, 0.8),
                crate::query::router::QueryRoute::ContentPhrase => (0.8, 1.3, 1.0),
                crate::query::router::QueryRoute::SemanticSearch => (0.8, 1.0, 1.5),
                _ => (1.0, 1.0, 1.0),
            };

            // Merge fuzzy matches
            for (&idx, &rank) in &fuzzy_ranks {
                if let Some(entry) = entries.get(idx) {
                    let rrf = w_fuzzy * (1.0 / (60.0 + (rank as f64)));
                    candidates.insert(
                        entry.path.clone(),
                        MatchCandidate {
                            entry_idx: idx,
                            in_fuzzy: true,
                            in_bm25: false,
                            in_semantic: false,
                            rrf_score: rrf,
                            best_range: None,
                        },
                    );
                }
            }

            // Merge BM25 matches
            for (path, &rank) in &bm25_ranks {
                if let Some(&orig_idx) = path_index.get(path) {
                    if !candidate_set.contains(&orig_idx) {
                        continue;
                    }
                    let rrf = w_bm25 * (1.0 / (60.0 + (rank as f64)));
                    if let Some(cand) = candidates.get_mut(path) {
                        cand.in_bm25 = true;
                        cand.rrf_score += rrf;
                    } else {
                        candidates.insert(
                            path.clone(),
                            MatchCandidate {
                                entry_idx: orig_idx,
                                in_fuzzy: false,
                                in_bm25: true,
                                in_semantic: false,
                                rrf_score: rrf,
                                best_range: None,
                            },
                        );
                    }
                }
            }

            // Merge Semantic matches
            for (path, &(rank, range)) in &semantic_ranks {
                if let Some(&orig_idx) = path_index.get(path) {
                    if !candidate_set.contains(&orig_idx) {
                        continue;
                    }
                    let rrf = w_semantic * (1.0 / (60.0 + (rank as f64)));
                    if let Some(cand) = candidates.get_mut(path) {
                        cand.in_semantic = true;
                        cand.rrf_score += rrf;
                        cand.best_range = Some(range);
                    } else {
                        candidates.insert(
                            path.clone(),
                            MatchCandidate {
                                entry_idx: orig_idx,
                                in_fuzzy: false,
                                in_bm25: false,
                                in_semantic: true,
                                rrf_score: rrf,
                                best_range: Some(range),
                            },
                        );
                    }
                }
            }
        }

        // 7. Apply ranking bonuses & build SearchResults
        let mut results: Vec<SearchResult> = candidates
            .into_iter()
            .filter_map(|(path, cand)| {
                let entry = entries.get(cand.entry_idx)?;
                let mut score = cand.rrf_score;

                // MatchType resolution: Semantic badge if only found by semantic tier
                let match_type = if cand.in_semantic && !cand.in_fuzzy && !cand.in_bm25 {
                    MatchType::Semantic
                } else if cand.in_fuzzy && (cand.in_bm25 || cand.in_semantic) {
                    MatchType::Both
                } else if cand.in_bm25 {
                    MatchType::Content
                } else {
                    MatchType::FileName
                };

                // Exact substring in name: +0.20
                let name_lower = entry.name.to_lowercase();
                if !search_text_lower.is_empty() && name_lower.contains(&search_text_lower) {
                    score += 0.20;
                }

                // Prefix in name: +0.15
                if !search_text_lower.is_empty() && name_lower.starts_with(&search_text_lower) {
                    score += 0.15;
                }

                // Smooth recency decay: +0.10 * exp(-age_days / 7)
                let age_days = (now_sec.saturating_sub(entry.modified) as f64) / 86400.0;
                score += 0.10 * (-age_days / 7.0).exp();

                // Frecency bonus: +0.08 * log1p(opens) * exp(-last_open_days / 14)
                let (opens, last_open) = metadata.get_usage(&entry.path);
                if opens > 0 {
                    let last_open_days = (now_sec.saturating_sub(last_open) as f64) / 86400.0;
                    score += 0.08 * (opens as f64).ln_1p() * (-last_open_days / 14.0).exp();
                }

                // Depth penalty: -0.02 * depth
                let depth = path.components().count();
                score -= 0.02 * (depth as f64);

                Some(SearchResult {
                    path: path.to_string_lossy().to_string(),
                    filename: entry.name.clone(),
                    snippet: None,
                    score,
                    file_size: entry.size,
                    modified: entry.modified,
                    is_dir: entry.is_dir,
                    match_type,
                    best_range: cand.best_range,
                })
            })
            .collect();

        // 8. Deterministic tiebreak: sort by score descending, then path ascending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.path.cmp(&b.path))
        });

        if results.len() > limit {
            results.truncate(limit);
        }

        // 9. Post-rank snippet extraction for top 20 files
        if !search_text.is_empty() {
            for item in results.iter_mut().take(20) {
                if item.is_dir {
                    continue;
                }
                if let Some(ci) = content_index.as_ref() {
                    item.snippet = ci.get_snippet(Path::new(&item.path), &search_text);
                }
                if item.snippet.is_none()
                    && let Some((b_start, b_end)) = item.best_range
                    && let Some(content) = crate::indexer::content::read_text_content(
                        Path::new(&item.path),
                        ((b_end + 1000).max(10_000)) as u64,
                    )
                {
                    let snippet_start = b_start.saturating_sub(60);
                    let snippet_end = (b_end + 60).min(content.len());
                    let safe_start = content
                        .char_indices()
                        .map(|(i, _)| i)
                        .rfind(|&i| i <= snippet_start)
                        .unwrap_or(0);
                    let safe_end = content
                        .char_indices()
                        .map(|(i, _)| i)
                        .find(|&i| i >= snippet_end)
                        .unwrap_or(content.len());
                    if safe_end > safe_start {
                        let slice = &content[safe_start..safe_end];
                        item.snippet = Some(slice.replace('\n', " ").trim().to_string());
                    }
                }
            }
        }

        let elapsed = start_time.elapsed().map(|d| d.as_millis() as u64).unwrap_or(0);

        SearchResponse {
            results,
            seq,
            active_filters: parsed.active_filters_dto,
            hints: parsed.hints,
            search_time_ms: elapsed,
        }
    }
}

fn matches_all_filters(filters: &[Filter], entry: &FileEntry) -> bool {
    for f in filters {
        if !matches_filter(f, entry) {
            return false;
        }
    }
    true
}

fn matches_filter(filter: &Filter, entry: &FileEntry) -> bool {
    match filter {
        Filter::Ext(ext) => entry
            .extension
            .as_deref()
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false),
        Filter::Type(typ) => matches_type_category(typ, entry.extension.as_deref(), entry.is_dir),
        Filter::Modified(range) => range.matches(entry.modified),
        Filter::Size(range) => range.matches(entry.size),
        Filter::Kind(FileKind::Dir) => entry.is_dir,
        Filter::Kind(FileKind::File) => !entry.is_dir,
        Filter::Lang(lang) => matches_language(lang, entry.extension.as_deref()),
        Filter::Name(name) => entry.name.to_lowercase().contains(&name.to_lowercase()),
        Filter::Path(path_substr) => entry
            .path
            .to_string_lossy()
            .to_lowercase()
            .contains(&path_substr.to_lowercase()),
        Filter::Not(inner) => !matches_filter(inner, entry),
    }
}

fn matches_type_category(typ: &str, ext_opt: Option<&str>, is_dir: bool) -> bool {
    let ext = match ext_opt {
        Some(e) => e.to_lowercase(),
        None => {
            if is_dir && (typ == "dir" || typ == "directory" || typ == "folder") {
                return true;
            }
            return false;
        }
    };
    match typ.to_lowercase().as_str() {
        "image" => matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "tif"
        ),
        "pdf" => ext == "pdf",
        "doc" | "document" => matches!(
            ext.as_str(),
            "doc" | "docx" | "odt" | "rtf" | "pdf" | "txt" | "md" | "pages" | "epub"
        ),
        "sheet" | "spreadsheet" | "table" => matches!(
            ext.as_str(),
            "xlsx" | "xls" | "ods" | "csv" | "tsv" | "numbers"
        ),
        "presentation" | "slide" | "slides" => matches!(
            ext.as_str(),
            "pptx" | "ppt" | "odp" | "key"
        ),
        "ebook" | "book" => matches!(
            ext.as_str(),
            "epub" | "mobi" | "azw3" | "pdf"
        ),
        "code" => matches!(
            ext.as_str(),
            "rs"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "java"
                | "kt"
                | "swift"
                | "rb"
                | "php"
                | "sh"
                | "html"
                | "css"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "xml"
                | "sql"
        ),
        "video" => matches!(ext.as_str(), "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv"),
        "audio" => matches!(ext.as_str(), "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a"),
        "archive" => matches!(
            ext.as_str(),
            "zip" | "tar" | "gz" | "bz2" | "7z" | "rar" | "xz"
        ),
        "dir" | "directory" | "folder" => is_dir,
        other => ext == other,
    }
}

fn matches_language(lang: &str, ext_opt: Option<&str>) -> bool {
    let ext = match ext_opt {
        Some(e) => e.to_lowercase(),
        None => return false,
    };
    match lang.to_lowercase().as_str() {
        "rust" => ext == "rs",
        "python" | "py" => ext == "py",
        "typescript" | "ts" => ext == "ts" || ext == "tsx",
        "javascript" | "js" => ext == "js" || ext == "jsx" || ext == "mjs" || ext == "cjs",
        "go" => ext == "go",
        "c" => ext == "c" || ext == "h",
        "cpp" | "c++" => matches!(ext.as_str(), "cpp" | "cc" | "cxx" | "hpp" | "h"),
        "java" => ext == "java",
        "kotlin" | "kt" => ext == "kt" || ext == "kts",
        "swift" => ext == "swift",
        "ruby" | "rb" => ext == "rb",
        "php" => ext == "php",
        "shell" | "bash" | "sh" => matches!(ext.as_str(), "sh" | "bash" | "zsh"),
        "html" => ext == "html" || ext == "htm",
        "css" => ext == "css" || ext == "scss" || ext == "sass",
        "json" => ext == "json",
        "markdown" | "md" => ext == "md" || ext == "markdown",
        "sql" => ext == "sql",
        other => ext == other,
    }
}
