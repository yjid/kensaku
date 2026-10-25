pub mod fuzzy;

use crate::embed::{ExactScanner, ModelRegistry, UnitStore};
use crate::indexer::content::{ContentIndex, FieldKind};
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::FileEntry;
use crate::query::ast::{ActiveFilterDto, Filter, Mode, TermField};
use crate::query::parse_query;
use crate::search::fuzzy::FuzzySearcher;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Hard cap for `mode:exhaustive` result enumeration.
const EXHAUSTIVE_LIMIT: usize = 10_000;

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
    /// Count of matches before the display limit (collected count for
    /// `mode:exhaustive`).
    pub total: usize,
    /// `"ranked"` or `"exhaustive"`.
    pub mode: &'static str,
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
        let resp = Self::search_response(
            query,
            metadata,
            content_index,
            unit_store,
            semantic_enabled,
            limit,
            0,
        );
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
                total: 0,
                mode: "ranked",
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
        let mut parsed = parse_query(raw_query, entries, path_index, Some(&trie), generation);
        let route = crate::query::router::route_query(&parsed);

        // Pre-compile filters once per query (lowercase strings, intern extensions)
        let compiled_filters: Vec<CompiledFilter> =
            parsed.filters.iter().map(CompiledFilter::from).collect();

        // 2. Resolve scopes and filter candidates
        let candidate_indices: Vec<usize> = if !parsed.scopes.is_empty() {
            let mut included = HashSet::new();
            let mut has_include_scope = false;

            for scope in &parsed.scopes {
                if scope.include {
                    has_include_scope = true;
                    let scope_candidates =
                        trie.get_scope_candidates(scope.dir_id as usize, generation);
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
                    let scope_candidates =
                        trie.get_scope_candidates(scope.dir_id as usize, generation);
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
                        matches_all_compiled_filters(&compiled_filters, entry)
                    } else {
                        false
                    }
                })
                .collect()
        } else if !parsed.filters.is_empty() {
            (0..entries.len())
                .filter(|&idx| matches_all_compiled_filters(&compiled_filters, &entries[idx]))
                .collect()
        } else {
            (0..entries.len()).collect()
        };

        // Extract positive terms text, split by field scoping (name:/content:/path:)
        // so that field-scoped terms only search their target field. Regex terms
        // (`re:`) never enter the fuzzy/BM25 text tiers.
        let mut fuzzy_terms: Vec<&str> = Vec::new(); // Any, Name, Path → fuzzy (filename + path)
        let mut bm25_groups: Vec<(FieldKind, &str)> = Vec::new();
        let mut regex_patterns: Vec<&str> = Vec::new();
        for t in &parsed.terms {
            if t.is_regex {
                regex_patterns.push(&t.text);
                continue;
            }
            match t.field {
                TermField::Any => {
                    fuzzy_terms.push(&t.text);
                    bm25_groups.push((FieldKind::Any, &t.text));
                }
                TermField::Name => {
                    fuzzy_terms.push(&t.text);
                    bm25_groups.push((FieldKind::Name, &t.text));
                }
                TermField::Content => {
                    bm25_groups.push((FieldKind::Content, &t.text));
                }
                TermField::Path => {
                    fuzzy_terms.push(&t.text);
                }
            }
        }
        let fuzzy_text = fuzzy_terms.join(" ");
        let bm25_text: String = bm25_groups
            .iter()
            .map(|(_, text)| *text)
            .collect::<Vec<_>>()
            .join(" ");
        let search_text = if fuzzy_text.is_empty() {
            bm25_text.as_str()
        } else {
            fuzzy_text.as_str()
        };
        let search_text_lower = search_text.to_lowercase();

        // Build a membership bitmap for O(1) candidate checks (replaces O(n) Vec::contains)
        let candidate_set: std::collections::HashSet<usize> =
            candidate_indices.iter().copied().collect();

        // Node-id map for the semantic tier: built once for the candidate set so
        // the scanner can prefilter units and hits need no per-hit metadata lookup.
        let mut node_to_idx: HashMap<u64, usize> = HashMap::new();
        for &idx in &candidate_indices {
            if let Some(entry) = entries.get(idx)
                && entry.id > 0
            {
                node_to_idx.insert(entry.id, idx);
            }
        }
        let allowed_node_ids: Option<HashSet<u64>> = if node_to_idx.is_empty() {
            None
        } else {
            Some(node_to_idx.keys().copied().collect())
        };

        // 2b. Exhaustive mode: no fuzzy/semantic tiers, enumerate by path.
        if parsed.mode == Mode::Exhaustive {
            let mut hints = parsed.hints;
            let mut results = exhaustive_results(
                content_index,
                entries,
                path_index,
                &candidate_set,
                &candidate_indices,
                &bm25_groups,
                &regex_patterns,
                EXHAUSTIVE_LIMIT,
                &mut hints,
            );
            let total = results.len();
            if results.len() > EXHAUSTIVE_LIMIT {
                results.truncate(EXHAUSTIVE_LIMIT);
            }
            if results.is_empty() && !parsed.active_filters_dto.is_empty() {
                hints.push("No results match the active filters; try removing one.".to_string());
            }
            let elapsed = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            return SearchResponse {
                results,
                seq,
                active_filters: parsed.active_filters_dto,
                hints,
                search_time_ms: elapsed,
                total,
                mode: "exhaustive",
            };
        }

        // 3. Tier 1: Fuzzy filename search (< 5ms)
        let mut fuzzy_ranks: HashMap<usize, usize> = HashMap::new();
        if !fuzzy_text.is_empty() {
            let mut fuzzy_searcher = FuzzySearcher::new();
            // Search across candidate entries by index (no clone needed)
            let fuzzy_matches =
                fuzzy_searcher.search(&fuzzy_text, entries, &candidate_indices, limit * 3);
            for (rank, (orig_idx, _score)) in fuzzy_matches.into_iter().enumerate() {
                fuzzy_ranks.insert(orig_idx, rank + 1);
            }
        }

        // 4. Tier 2: Tantivy BM25 content search (< 50ms)
        let mut bm25_ranks: HashMap<PathBuf, usize> = HashMap::new();
        if (!bm25_groups.is_empty() || !regex_patterns.is_empty())
            && let Some(c_index) = content_index
        {
            let outcome =
                c_index.search_groups_with_regex(&bm25_groups, &regex_patterns, limit * 3, None);
            parsed.hints.extend(outcome.hints);
            for (rank, cm) in outcome.matches.into_iter().enumerate() {
                bm25_ranks.insert(cm.path, rank + 1);
            }
        }

        // 5. Tier 3: Static semantic vector scan (< 5ms)
        // Weak vectors are kept in the map (with their raw cosine) so hits that
        // also match fuzzy/BM25 still contribute; the floor below is applied
        // only when creating a semantic-only candidate.
        let semantic_floor = if ModelRegistry::global().has_static_table() {
            0.28
        } else {
            0.12
        };
        let mut semantic_ranks: HashMap<PathBuf, (usize, f32, (usize, usize))> = HashMap::new();
        if semantic_enabled
            && !search_text.is_empty()
            && let Some(store) = unit_store
            && !store.is_empty()
        {
            let model = ModelRegistry::global();
            let query_vec = model.embed_to_i8(search_text);
            let hits = ExactScanner::scan(&query_vec, store, allowed_node_ids.as_ref(), limit * 3);
            for (rank, hit) in hits.into_iter().enumerate() {
                let orig_idx = match node_to_idx.get(&hit.node_id).copied() {
                    Some(idx) => idx,
                    // Fallback for stores whose entries carry no node ids.
                    None if node_to_idx.is_empty() => metadata
                        .get_by_node_id(hit.node_id)
                        .and_then(|entry| path_index.get(&entry.path).copied())
                        .unwrap_or(usize::MAX),
                    None => continue,
                };
                if orig_idx != usize::MAX
                    && candidate_set.contains(&orig_idx)
                    && let Some(entry) = entries.get(orig_idx)
                {
                    semantic_ranks.insert(
                        entry.path.clone(),
                        (
                            rank + 1,
                            hit.score,
                            (hit.best_unit.byte_start, hit.best_unit.byte_end),
                        ),
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

        if fuzzy_text.is_empty() && bm25_groups.is_empty() && regex_patterns.is_empty() {
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
            for (path, &(rank, cosine, range)) in &semantic_ranks {
                let Some(&orig_idx) = path_index.get(path) else {
                    continue;
                };
                if !candidate_set.contains(&orig_idx) {
                    continue;
                }
                let rrf = w_semantic * (1.0 / (60.0 + (rank as f64)));
                if let Some(cand) = candidates.get_mut(path) {
                    cand.in_semantic = true;
                    cand.rrf_score += rrf;
                    cand.best_range = Some(range);
                } else {
                    // Semantic-only candidates must clear the relevance floor;
                    // hits already found by fuzzy/BM25 are merged above.
                    if cosine < semantic_floor {
                        continue;
                    }
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

        // 7. Apply ranking bonuses & build SearchResults
        // Normalize RRF before bonuses so an absolute depth term cannot
        // dominate the fused relevance signal.
        let max_rrf = candidates
            .values()
            .map(|candidate| candidate.rrf_score)
            .fold(0.0f64, f64::max);

        let mut results: Vec<SearchResult> = candidates
            .into_iter()
            .filter_map(|(path, cand)| {
                let entry = entries.get(cand.entry_idx)?;

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

                let usage = metadata.get_usage(&entry.path);
                let score = finalize_score(
                    cand.rrf_score,
                    max_rrf,
                    entry,
                    &path,
                    &search_text_lower,
                    now_sec,
                    usage,
                );

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

        let total = results.len();

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
                    item.snippet = ci.get_snippet(Path::new(&item.path), search_text);
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

        if results.is_empty() && !parsed.active_filters_dto.is_empty() {
            parsed
                .hints
                .push("No results match the active filters; try removing one.".to_string());
        }

        let elapsed = start_time
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        SearchResponse {
            results,
            seq,
            active_filters: parsed.active_filters_dto,
            hints: parsed.hints,
            search_time_ms: elapsed,
            total,
            mode: "ranked",
        }
    }
}

/// Final score for a fused candidate.
///
/// RRF is normalized to `[0, 1]` before bonuses so scores are comparable
/// regardless of how many tiers matched. The depth penalty is bounded (max
/// `-0.10`) and can never push a score below zero.
fn finalize_score(
    rrf_score: f64,
    max_rrf: f64,
    entry: &FileEntry,
    path: &Path,
    search_text_lower: &str,
    now_sec: u64,
    usage: (u32, u64),
) -> f64 {
    let base = if max_rrf > 0.0 {
        rrf_score / max_rrf
    } else {
        0.0
    };
    let mut score = base;

    // Exact substring in name: +0.20
    let name_lower = entry.name.to_lowercase();
    if !search_text_lower.is_empty() && name_lower.contains(search_text_lower) {
        score += 0.20;
    }

    // Prefix in name: +0.15
    if !search_text_lower.is_empty() && name_lower.starts_with(search_text_lower) {
        score += 0.15;
    }

    // Smooth recency decay: +0.10 * exp(-age_days / 7)
    let age_days = (now_sec.saturating_sub(entry.modified) as f64) / 86400.0;
    score += 0.10 * (-age_days / 7.0).exp();

    // Frecency bonus: +0.08 * log1p(opens) * exp(-last_open_days / 14)
    let (opens, last_open) = usage;
    if opens > 0 {
        let last_open_days = (now_sec.saturating_sub(last_open) as f64) / 86400.0;
        score += 0.08 * (opens as f64).ln_1p() * (-last_open_days / 14.0).exp();
    }

    // Bounded depth penalty: at most -0.10, saturating at depth 20.
    let depth = path.components().count();
    score -= 0.10 * ((depth as f64) / 20.0).min(1.0);

    score.max(0.0)
}

/// Enumerates matches for `mode:exhaustive`, ordered by path.
///
/// Fuzzy and semantic tiers are deliberately skipped: they rank, they do not
/// enumerate. The returned vector is capped only by the caller.
#[allow(clippy::too_many_arguments)]
fn exhaustive_results(
    content_index: &Option<ContentIndex>,
    entries: &[FileEntry],
    path_index: &HashMap<PathBuf, usize>,
    candidate_set: &HashSet<usize>,
    candidate_indices: &[usize],
    groups: &[(FieldKind, &str)],
    regex_patterns: &[&str],
    cap: usize,
    hints: &mut Vec<String>,
) -> Vec<SearchResult> {
    let mut results = Vec::new();

    if groups.is_empty() && regex_patterns.is_empty() {
        for &idx in candidate_indices {
            if let Some(entry) = entries.get(idx) {
                results.push(SearchResult {
                    path: entry.path.to_string_lossy().to_string(),
                    filename: entry.name.clone(),
                    snippet: None,
                    score: 0.0,
                    file_size: entry.size,
                    modified: entry.modified,
                    is_dir: entry.is_dir,
                    match_type: MatchType::FileName,
                    best_range: None,
                });
            }
        }
    } else if let Some(c_index) = content_index {
        let outcome = c_index.search_groups_with_regex(groups, regex_patterns, cap, None);
        hints.extend(outcome.hints);
        for cm in outcome.matches {
            let Some(&idx) = path_index.get(&cm.path) else {
                continue;
            };
            if !candidate_set.contains(&idx) {
                continue;
            }
            let Some(entry) = entries.get(idx) else {
                continue;
            };
            results.push(SearchResult {
                path: cm.path.to_string_lossy().to_string(),
                filename: entry.name.clone(),
                snippet: None,
                score: cm.score as f64,
                file_size: entry.size,
                modified: entry.modified,
                is_dir: entry.is_dir,
                match_type: MatchType::Content,
                best_range: None,
            });
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));

    if results.len() >= cap {
        hints.push(format!(
            "Exhaustive mode capped at {} results; refine the query to narrow it.",
            cap
        ));
    }

    results
}

fn matches_all_compiled_filters(filters: &[CompiledFilter], entry: &FileEntry) -> bool {
    for f in filters {
        if !f.matches(entry) {
            return false;
        }
    }
    true
}

/// Pre-compiled filter that pre-lowercases strings to avoid per-entry allocation.
#[derive(Clone)]
enum CompiledFilter {
    Ext(String),
    Type(String),
    Modified(crate::query::ast::RangeOp),
    Size(crate::query::ast::RangeOp),
    Kind(crate::query::ast::FileKind),
    Lang(String),
    Name(String),
    Path(String),
    Not(Box<CompiledFilter>),
}

impl CompiledFilter {
    fn from(f: &Filter) -> Self {
        match f {
            Filter::Ext(e) => CompiledFilter::Ext(e.to_lowercase()),
            Filter::Type(t) => CompiledFilter::Type(t.to_lowercase()),
            Filter::Modified(r) => CompiledFilter::Modified(*r),
            Filter::Size(r) => CompiledFilter::Size(*r),
            Filter::Kind(k) => CompiledFilter::Kind(*k),
            Filter::Lang(l) => CompiledFilter::Lang(l.to_lowercase()),
            Filter::Name(n) => CompiledFilter::Name(n.to_lowercase()),
            Filter::Path(p) => CompiledFilter::Path(p.to_lowercase()),
            Filter::Not(inner) => CompiledFilter::Not(Box::new(CompiledFilter::from(inner))),
        }
    }

    fn matches(&self, entry: &FileEntry) -> bool {
        match self {
            CompiledFilter::Ext(ext) => entry
                .extension
                .as_deref()
                .map(|e| e.eq_ignore_ascii_case(ext))
                .unwrap_or(false),
            CompiledFilter::Type(typ) => {
                matches_type_category(typ, entry.extension.as_deref(), entry.is_dir)
            }
            CompiledFilter::Modified(range) => range.matches(entry.modified),
            CompiledFilter::Size(range) => range.matches(entry.size),
            CompiledFilter::Kind(crate::query::ast::FileKind::Dir) => entry.is_dir,
            CompiledFilter::Kind(crate::query::ast::FileKind::File) => !entry.is_dir,
            CompiledFilter::Lang(lang) => matches_language(lang, entry.extension.as_deref()),
            CompiledFilter::Name(name) => {
                // Avoid allocation: compare lowercase name directly
                // (name is already lowercase in CompiledFilter)
                entry.name.to_lowercase().contains(name)
            }
            CompiledFilter::Path(path_substr) => entry
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(path_substr),
            CompiledFilter::Not(inner) => !inner.matches(entry),
        }
    }
}

// Removed: replaced by CompiledFilter::from + CompiledFilter::matches

// Removed: replaced by matches_all_compiled_filters + CompiledFilter

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
        "presentation" | "slide" | "slides" => {
            matches!(ext.as_str(), "pptx" | "ppt" | "odp" | "key")
        }
        "ebook" | "book" => matches!(ext.as_str(), "epub" | "mobi" | "azw3" | "pdf"),
        "code" => matches!(
            ext.as_str(),
            "rs" | "ts"
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempIndex {
        index: Option<ContentIndex>,
        dir: PathBuf,
    }

    impl Drop for TempIndex {
        fn drop(&mut self) {
            self.index.take();
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn temp_index(tag: &str) -> TempIndex {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kensaku-search-test-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ));
        let index = ContentIndex::new(&dir).expect("create content index");
        TempIndex {
            index: Some(index),
            dir,
        }
    }

    fn entry(id: u64, path: &str, name: &str) -> FileEntry {
        FileEntry {
            id,
            path: PathBuf::from(path),
            name: name.to_string(),
            extension: Some("rs".to_string()),
            size: 10,
            modified: 0,
            is_dir: false,
            dev: 0,
            ino: 0,
            char_bloom: 0,
            content_pending: false,
        }
    }

    #[test]
    fn exhaustive_results_ignore_display_limit() {
        let temp = temp_index("exhaustive");
        let index = temp.index.as_ref().unwrap();
        let mut entries = Vec::new();
        let mut path_index = HashMap::new();
        let mut candidate_indices = Vec::new();

        for i in 0..12 {
            let path = format!("/tmp/kensaku/e/file-{:02}.rs", i);
            let name = format!("file-{:02}.rs", i);
            index
                .add_or_update(Path::new(&path), &name, "shared needle", 0)
                .expect("add document");
            let idx = entries.len();
            entries.push(entry(i as u64 + 1, &path, &name));
            path_index.insert(PathBuf::from(&path), idx);
            candidate_indices.push(idx);
        }
        index.commit().unwrap();

        let candidate_set: HashSet<usize> = candidate_indices.iter().copied().collect();
        let mut hints = Vec::new();
        let results = exhaustive_results(
            &temp.index,
            &entries,
            &path_index,
            &candidate_set,
            &candidate_indices,
            &[(FieldKind::Content, "needle")],
            &[],
            EXHAUSTIVE_LIMIT,
            &mut hints,
        );

        // A normal display limit (e.g. 5) must not apply here.
        assert_eq!(results.len(), 12);
        assert!(results.len() > 5);
        let sorted = {
            let mut paths = results.iter().map(|r| r.path.clone()).collect::<Vec<_>>();
            paths.sort();
            paths
        };
        assert_eq!(
            results.iter().map(|r| r.path.clone()).collect::<Vec<_>>(),
            sorted
        );
        assert!(!hints.iter().any(|h| h.contains("capped")));
    }

    #[test]
    fn depth_penalty_is_bounded_and_does_not_dominate_relevance() {
        let now = 1_700_000_000;
        let deep_path = Path::new("/tmp/k/a/b/c/d/e/f/g/h/i/j/k/deep.rs");
        let shallow_path = Path::new("/tmp/k/shallow.rs");
        let deep = entry(1, deep_path.to_str().unwrap(), "deep.rs");
        let shallow = entry(2, shallow_path.to_str().unwrap(), "shallow.rs");

        // Equal relevance and metadata: only depth differs, so the shallower
        // path must win.
        let deep_score = finalize_score(0.5, 0.5, &deep, deep_path, "zzz", now, (0, 0));
        let shallow_score = finalize_score(0.5, 0.5, &shallow, shallow_path, "zzz", now, (0, 0));
        assert!(
            shallow_score > deep_score,
            "shallow {shallow_score} should beat deep {deep_score}"
        );

        // A very deep but highly relevant file still beats a shallow weak one.
        let relevant_deep = entry(3, deep_path.to_str().unwrap(), "alpha.rs");
        let weak_shallow = entry(4, shallow_path.to_str().unwrap(), "other.rs");
        let relevant_score =
            finalize_score(0.9, 1.0, &relevant_deep, deep_path, "alpha", now, (0, 0));
        let weak_score =
            finalize_score(0.1, 1.0, &weak_shallow, shallow_path, "alpha", now, (0, 0));
        assert!(
            relevant_score > weak_score,
            "relevant deep {relevant_score} should beat weak shallow {weak_score}"
        );
        assert!(relevant_score >= 0.0);

        // Scores never go negative, even for extremely deep unranked files.
        let very_deep = Path::new("/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z.rs");
        let bottom = finalize_score(0.0, 1.0, &deep, very_deep, "", now, (0, 0));
        assert_eq!(bottom, 0.0);
    }

    #[test]
    fn semantic_floor_drops_weak_semantic_only_hits() {
        use crate::embed::Unit;
        use crate::indexer::metadata_db::MetadataDb;

        let model = ModelRegistry::global();
        let query_vec = model.embed_to_i8("needle");

        // Build a vector with a deliberately small positive cosine (~0.07):
        // above the scanner's 0.05 prefilter, below either semantic floor.
        let (idx, value) = query_vec
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| value.unsigned_abs())
            .map(|(idx, value)| (idx, *value))
            .expect("non-empty embedding");
        assert_ne!(value, 0, "embedding should have a non-zero component");
        let scaled = (1200.0 / f32::from(value.abs().max(1))).round().clamp(1.0, 127.0) as i8;
        let mut weak_vec = vec![0i8; query_vec.len()];
        weak_vec[idx] = scaled * value.signum();

        let mut units = UnitStore::new(model.dim);
        units.add_unit(
            Unit {
                id: 0,
                node_id: 1,
                byte_start: 0,
                byte_end: 10,
                model_id: model.model_id,
            },
            &query_vec,
        );
        units.add_unit(
            Unit {
                id: 1,
                node_id: 2,
                byte_start: 0,
                byte_end: 10,
                model_id: model.model_id,
            },
            &weak_vec,
        );
        assert_eq!(
            ExactScanner::scan(&query_vec, &units, None, 10).len(),
            2,
            "the weak vector must pass the scanner prefilter"
        );

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kensaku-semantic-floor-{}-{}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        let db = MetadataDb::open_in_dir(&dir).expect("open metadata db");
        db.upsert(&entry(1, "/tmp/kensaku/sem/strong.txt", "strong.txt"))
            .unwrap();
        db.upsert(&entry(2, "/tmp/kensaku/sem/weak.txt", "weak.txt"))
            .unwrap();
        let metadata = MetadataStore::with_db(db);

        let response = SearchDispatcher::search_response(
            "needle",
            &metadata,
            &None,
            Some(&units),
            true,
            10,
            0,
        );

        assert_eq!(
            response.results.len(),
            1,
            "weak semantic-only hit should be dropped: {:?}",
            response
                .results
                .iter()
                .map(|result| (&result.path, result.score))
                .collect::<Vec<_>>()
        );
        assert!(response.results[0].path.ends_with("strong.txt"));
        assert!(response.results[0].score >= 0.0);

        let _ = fs::remove_dir_all(&dir);
    }
}
