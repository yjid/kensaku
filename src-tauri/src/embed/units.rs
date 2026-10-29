use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

static MAX_SEMANTIC_UNITS: AtomicUsize = AtomicUsize::new(20);

pub fn set_max_semantic_units(n: usize) {
    MAX_SEMANTIC_UNITS.store(n.max(1), Ordering::SeqCst);
}

pub fn max_semantic_units() -> usize {
    MAX_SEMANTIC_UNITS.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: u64,
    pub node_id: u64,
    pub byte_start: usize,
    pub byte_end: usize,
    pub model_id: u32,
}

/// Splits text into units per architecture spec:
/// - Files <= 240 tokens: one doc-level unit (byte_start=0, byte_end=file_size).
/// - Longer files: 240-token windows with 48 overlap, max 20 units per file.
pub fn chunk_text(text: &str, file_size: usize) -> Vec<(usize, usize, String)> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let max_units = max_semantic_units();
    // How many words we ever need to look at: enough to fill `max_units`
    // windows. Beyond this we can stop scanning — the rest of the document
    // is never embedded by this chunker anyway.
    let scan_cap = (max_units.saturating_sub(1)) * 192 + 240 + 1;

    // Split words with their byte offsets in text, stopping early for huge docs.
    let mut word_offsets: Vec<(usize, usize)> = Vec::with_capacity(scan_cap.min(4096));
    let mut in_word = false;
    let mut word_start = 0;
    let mut fully_scanned = true;

    for (byte_idx, ch) in text.char_indices() {
        if word_offsets.len() >= scan_cap {
            fully_scanned = false;
            break;
        }
        if ch.is_whitespace() {
            if in_word {
                word_offsets.push((word_start, byte_idx));
                in_word = false;
            }
        } else if !in_word {
            word_start = byte_idx;
            in_word = true;
        }
    }
    if in_word {
        if word_offsets.len() < scan_cap {
            word_offsets.push((word_start, text.len()));
        } else {
            fully_scanned = false;
        }
    }

    // If the whole text fit in scan_cap and has <= 240 tokens: doc-level unit.
    if fully_scanned && word_offsets.len() <= 240 {
        return vec![(0, file_size, text.to_string())];
    }

    let mut chunks = Vec::new();
    let window_size = 240;
    let step = 240 - 48; // 192 tokens step with 48 tokens overlap

    let mut start_idx = 0;
    while start_idx < word_offsets.len() && chunks.len() < max_units {
        let end_idx = (start_idx + window_size).min(word_offsets.len());
        let byte_start = word_offsets[start_idx].0;
        let byte_end = word_offsets[end_idx - 1].1;
        let chunk_slice = &text[byte_start..byte_end];

        chunks.push((byte_start, byte_end, chunk_slice.to_string()));

        if end_idx >= word_offsets.len() {
            break;
        }
        start_idx += step;
    }

    chunks
}

/// In-memory storage for semantic units and contiguous i8 vectors.
#[derive(Default)]
pub struct UnitStore {
    pub units: Vec<Unit>,
    pub vectors: Vec<i8>,
    pub node_to_units: HashMap<u64, Vec<usize>>,
    pub dim: usize,
}

impl UnitStore {
    pub fn new(dim: usize) -> Self {
        Self {
            units: Vec::new(),
            vectors: Vec::new(),
            node_to_units: HashMap::new(),
            dim,
        }
    }

    pub fn add_unit(&mut self, unit: Unit, vector: &[i8]) {
        if vector.len() != self.dim {
            return;
        }
        let unit_idx = self.units.len();
        self.node_to_units
            .entry(unit.node_id)
            .or_default()
            .push(unit_idx);
        self.units.push(unit);
        self.vectors.extend_from_slice(vector);
    }

    pub fn remove_units_for_node(&mut self, node_id: u64) {
        let mut set = std::collections::HashSet::new();
        set.insert(node_id);
        self.remove_units_for_nodes(&set);
    }

    /// Removes every unit belonging to any node in `node_ids` in one pass.
    ///
    /// The crawl uses this to clear the semantic state of all files it is about
    /// to re-extract. Doing it in one pass keeps a full re-index O(units)
    /// instead of O(units × files), which matters on large corpora.
    pub fn remove_units_for_nodes(&mut self, node_ids: &std::collections::HashSet<u64>) {
        if node_ids.is_empty() || self.units.is_empty() {
            return;
        }

        let mut new_units = Vec::with_capacity(self.units.len());
        let mut new_vectors = Vec::with_capacity(self.vectors.len());
        let mut new_node_map: HashMap<u64, Vec<usize>> = HashMap::new();

        for (old_idx, unit) in self.units.drain(..).enumerate() {
            if node_ids.contains(&unit.node_id) {
                continue;
            }
            let new_idx = new_units.len();
            new_node_map.entry(unit.node_id).or_default().push(new_idx);
            new_units.push(unit);
            let v_start = old_idx * self.dim;
            let v_end = v_start + self.dim;
            new_vectors.extend_from_slice(&self.vectors[v_start..v_end]);
        }

        self.units = new_units;
        self.vectors = new_vectors;
        self.node_to_units = new_node_map;
    }

    pub fn clear(&mut self) {
        self.units.clear();
        self.vectors.clear();
        self.node_to_units.clear();
    }

    /// Retains only units matching `expected_model_id`, discarding any units
    /// produced by a previous or incompatible embedding model.
    pub fn retain_model(&mut self, expected_model_id: u32) {
        if self.units.iter().all(|u| u.model_id == expected_model_id) {
            return;
        }
        let mut new_units = Vec::new();
        let mut new_vectors = Vec::new();
        let mut new_node_to_units: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, unit) in self.units.iter().enumerate() {
            if unit.model_id == expected_model_id {
                let start = idx * self.dim;
                let end = start + self.dim;
                let new_idx = new_units.len();
                new_node_to_units
                    .entry(unit.node_id)
                    .or_default()
                    .push(new_idx);
                new_units.push(unit.clone());
                new_vectors.extend_from_slice(&self.vectors[start..end]);
            }
        }
        self.units = new_units;
        self.vectors = new_vectors;
        self.node_to_units = new_node_to_units;
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn get_unit(&self, idx: usize) -> Option<&Unit> {
        self.units.get(idx)
    }

    pub fn get_vector(&self, idx: usize) -> Option<&[i8]> {
        let start = idx * self.dim;
        let end = start + self.dim;
        if end <= self.vectors.len() {
            Some(&self.vectors[start..end])
        } else {
            None
        }
    }
}
