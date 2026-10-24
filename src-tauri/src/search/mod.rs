pub mod fuzzy;

use crate::indexer::content::{ContentIndex, ContentMatch};
use crate::indexer::metadata::MetadataStore;
use crate::search::fuzzy::FuzzySearcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchType {
    FileName,
    Content,
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
}

pub struct SearchDispatcher;

impl SearchDispatcher {
    pub fn search(
        query: &str,
        metadata: &MetadataStore,
        content_index: &Option<ContentIndex>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let now_sec = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut results_map: HashMap<PathBuf, SearchResult> = HashMap::new();

        // 1. Fuzzy filename search (< 5ms)
        let mut fuzzy_searcher = FuzzySearcher::new();
        let entries = metadata.get_entries();
        let fuzzy_matches = fuzzy_searcher.search(trimmed, entries, limit * 2);

        for (entry_idx, score) in fuzzy_matches {
            if let Some(entry) = entries.get(entry_idx) {
                let recency_bonus = calculate_recency_bonus(entry.modified, now_sec);
                let depth_penalty = calculate_depth_penalty(&entry.path);
                let normalized_score = (score as f64) * 2.0 + recency_bonus - depth_penalty;

                results_map.insert(
                    entry.path.clone(),
                    SearchResult {
                        path: entry.path.to_string_lossy().to_string(),
                        filename: entry.name.clone(),
                        snippet: None,
                        score: normalized_score,
                        file_size: entry.size,
                        modified: entry.modified,
                        is_dir: entry.is_dir,
                        match_type: MatchType::FileName,
                    },
                );
            }
        }

        // 2. Full-text content search (< 50ms)
        if let Some(c_index) = content_index {
            let content_matches: Vec<ContentMatch> = c_index.search(trimmed, limit * 2);
            for cm in content_matches {
                let recency_bonus = calculate_recency_bonus(cm.modified, now_sec);
                let depth_penalty = calculate_depth_penalty(&cm.path);
                let content_score = (cm.score as f64) * 15.0 + recency_bonus - depth_penalty;

                if let Some(existing) = results_map.get_mut(&cm.path) {
                    existing.score += content_score;
                    existing.snippet = cm.snippet;
                    existing.match_type = MatchType::Both;
                } else {
                    let file_size = std::fs::metadata(&cm.path).map(|m| m.len()).unwrap_or(0);
                    results_map.insert(
                        cm.path.clone(),
                        SearchResult {
                            path: cm.path.to_string_lossy().to_string(),
                            filename: cm.filename,
                            snippet: cm.snippet,
                            score: content_score,
                            file_size,
                            modified: cm.modified,
                            is_dir: false,
                            match_type: MatchType::Content,
                        },
                    );
                }
            }
        }

        let mut results: Vec<SearchResult> = results_map.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        if results.len() > limit {
            results.truncate(limit);
        }

        results
    }
}

fn calculate_recency_bonus(modified_sec: u64, now_sec: u64) -> f64 {
    if modified_sec == 0 || now_sec < modified_sec {
        return 0.0;
    }
    let diff = now_sec - modified_sec;
    if diff < 86_400 {
        // Last 24 hours
        50.0
    } else if diff < 86_400 * 7 {
        // Last 7 days
        25.0
    } else if diff < 86_400 * 30 {
        // Last 30 days
        10.0
    } else {
        0.0
    }
}

fn calculate_depth_penalty(path: &Path) -> f64 {
    let depth = path.components().count();
    (depth as f64) * 1.5
}
