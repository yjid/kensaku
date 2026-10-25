use crate::embed::model::ModelRegistry;
use crate::embed::units::{Unit, UnitStore};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SemanticHit {
    pub node_id: u64,
    pub score: f32,
    pub best_unit: Unit,
}

pub struct ExactScanner;

impl ExactScanner {
    /// Exact parallel scan over memory-mapped / in-memory i8 vectors.
    /// Returns top-k hits grouped by node_id with max score and best unit range.
    ///
    /// When `allowed` is `Some`, units whose `node_id` is not in the set are
    /// skipped before computing cosine similarity. This keeps scoped/filtered
    /// queries from scoring the entire corpus.
    pub fn scan(
        query_vec: &[i8],
        store: &UnitStore,
        allowed: Option<&HashSet<u64>>,
        limit: usize,
    ) -> Vec<SemanticHit> {
        if store.is_empty() || query_vec.len() != store.dim {
            return Vec::new();
        }

        let dim = store.dim;
        let units = &store.units;
        let vectors = &store.vectors;

        // 8-way parallel scan over units using Rayon
        let scored_units: Vec<(usize, f32)> = (0..units.len())
            .into_par_iter()
            .filter_map(|idx| {
                let node_id = units[idx].node_id;
                if let Some(allowed) = allowed
                    && !allowed.contains(&node_id)
                {
                    return None;
                }

                let start = idx * dim;
                let end = start + dim;
                let unit_vec = &vectors[start..end];
                let sim = ModelRegistry::cosine_similarity_i8(query_vec, unit_vec);
                Some((idx, sim))
            })
            .filter(|&(_, sim)| sim > 0.05)
            .collect();

        // Group by node_id, keeping highest scoring unit per node
        let mut node_best: HashMap<u64, (f32, usize)> = HashMap::new();
        for (unit_idx, sim) in scored_units {
            let node_id = units[unit_idx].node_id;
            node_best
                .entry(node_id)
                .and_modify(|existing| {
                    if sim > existing.0 {
                        *existing = (sim, unit_idx);
                    }
                })
                .or_insert((sim, unit_idx));
        }

        let mut hits: Vec<SemanticHit> = node_best
            .into_iter()
            .map(|(node_id, (score, best_idx))| SemanticHit {
                node_id,
                score,
                best_unit: units[best_idx].clone(),
            })
            .collect();

        if hits.len() > limit {
            hits.select_nth_unstable_by(limit, |a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            hits.truncate(limit);
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: u64, node_id: u64) -> Unit {
        Unit {
            id,
            node_id,
            byte_start: 0,
            byte_end: 10,
            model_id: 1,
        }
    }

    #[test]
    fn prefilter_skips_non_candidate_nodes_before_scoring() {
        let mut store = UnitStore::new(4);
        store.add_unit(unit(0, 1), &[127, 0, 0, 0]);
        store.add_unit(unit(1, 2), &[0, 127, 0, 0]);

        // Query matches node 2 exactly.
        let query = [0i8, 127, 0, 0];

        let all = ExactScanner::scan(&query, &store, None, 10);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].node_id, 2);

        let allowed: HashSet<u64> = [1u64].into_iter().collect();
        let filtered = ExactScanner::scan(&query, &store, Some(&allowed), 10);
        assert!(filtered.is_empty(), "node 2 should be skipped by prefilter");

        let allowed: HashSet<u64> = [1u64, 2u64].into_iter().collect();
        let both = ExactScanner::scan(&query, &store, Some(&allowed), 10);
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].node_id, 2);
    }
}
