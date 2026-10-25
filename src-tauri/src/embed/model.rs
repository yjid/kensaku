use crate::config::KensakuConfig;
use crate::embed::table::StaticTable;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, OnceLock};

pub const EMBEDDING_DIM: usize = 256;
/// Id of the hashing estimator used when the real model files are unavailable.
pub const DEFAULT_MODEL_ID: u32 = 1;
/// Id of the real `potion-base-8M` static embedding table.
pub const REAL_MODEL_ID: u32 = 2;

static GLOBAL: OnceLock<ModelRegistry> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub model_id: u32,
    pub name: String,
    pub dim: usize,
    pub normalize: bool,
    /// Real embedding table when the model files were loaded; `None` means the
    /// hashing estimator is in use.
    #[serde(skip)]
    table: Option<Arc<StaticTable>>,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new_static()
    }
}

impl ModelRegistry {
    /// Deterministic hashing estimator. Used when the downloaded model files
    /// are absent or unusable, and kept as the `Default` implementation.
    pub fn new_static() -> Self {
        Self {
            model_id: DEFAULT_MODEL_ID,
            name: "static-embed-256".to_string(),
            dim: EMBEDDING_DIM,
            normalize: true,
            table: None,
        }
    }

    /// Loads the real static table from `dir` (the downloaded model directory).
    ///
    /// Returns `None` when the artifacts are missing/corrupt or the embedding
    /// width is not [`EMBEDDING_DIM`].
    pub fn load(dir: &Path) -> Option<Self> {
        let table = StaticTable::load(dir)?;
        Some(Self {
            model_id: table.model_id,
            name: "potion-base-8M".to_string(),
            dim: table.dim,
            normalize: table.normalize,
            table: Some(Arc::new(table)),
        })
    }

    /// Process-wide registry. Loads the ~30 MB embedding table exactly once,
    /// falling back to the hashing estimator when the files are unavailable.
    ///
    /// Hot paths must use this instead of `ModelRegistry::default()`.
    pub fn global() -> &'static ModelRegistry {
        GLOBAL.get_or_init(|| {
            let dir = KensakuConfig::config_dir().join("models");
            let registry = match Self::load(&dir) {
                Some(registry) => registry,
                None => {
                    log::info!(
                        "[Kensaku] No usable model files in {}; using hashing fallback",
                        dir.display()
                    );
                    Self::new_static()
                }
            };
            log::info!(
                "[Kensaku] Semantic model: {} (id {})",
                registry.name,
                registry.model_id
            );
            registry
        })
    }

    /// True when real model weights back this registry.
    pub fn has_static_table(&self) -> bool {
        self.table.is_some()
    }

    /// Tokenizes input text into normalized terms and character 3-grams for subword coverage.
    /// Only used by the hashing estimator; the static table has its own tokenizer.
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let cleaned = text.to_lowercase();

        for word in cleaned.split(|c: char| !c.is_alphanumeric()) {
            let trimmed = word.trim();
            if trimmed.len() < 2 {
                continue;
            }
            tokens.push(trimmed.to_string());

            // Add character trigrams for words longer than 4 chars
            let chars: Vec<char> = trimmed.chars().collect();
            if chars.len() >= 4 {
                for window in chars.windows(3) {
                    tokens.push(window.iter().collect::<String>());
                }
            }
        }
        tokens
    }

    /// Embeds text into a 256-dimensional unit-normalized float vector.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        if let Some(table) = &self.table {
            return table.embed(text);
        }
        self.embed_hashing(text)
    }

    fn embed_hashing(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dim];
        let tokens = self.tokenize(text);
        if tokens.is_empty() {
            return vec;
        }

        for token in &tokens {
            let (h1, h2) = hash_token(token);
            let idx1 = (h1 as usize) % self.dim;
            let idx2 = (h2 as usize) % self.dim;

            let sign1 = if (h1 >> 32) & 1 == 0 { 1.0f32 } else { -1.0f32 };
            let sign2 = if (h2 >> 32) & 1 == 0 { 1.0f32 } else { -1.0f32 };

            vec[idx1] += sign1;
            vec[idx2] += sign2 * 0.5;
        }

        if self.normalize {
            let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
            if norm_sq > 0.0 {
                let norm = norm_sq.sqrt();
                for v in &mut vec {
                    *v /= norm;
                }
            }
        }

        vec
    }

    /// Embeds text directly to signed 8-bit quantized vector (-127..=127).
    pub fn embed_to_i8(&self, text: &str) -> Vec<i8> {
        let floats = self.embed(text);
        floats
            .into_iter()
            .map(|f| (f * 127.0).round().clamp(-127.0, 127.0) as i8)
            .collect()
    }

    /// Computes cosine similarity between two unit-normalized i8 vectors in [-1.0, 1.0].
    pub fn cosine_similarity_i8(a: &[i8], b: &[i8]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let mut dot: i32 = 0;
        for i in 0..a.len() {
            dot += (a[i] as i32) * (b[i] as i32);
        }

        // 127.0 * 127.0 = 16129.0
        (dot as f32) / 16129.0
    }
}

/// Computes two independent 64-bit FNV-1a hashes with distinct seeds.
fn hash_token(s: &str) -> (u64, u64) {
    let mut h1: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;

    for b in s.as_bytes() {
        h1 ^= *b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);

        h2 ^= (*b as u64).rotate_left(4);
        h2 = h2.wrapping_mul(0x100000001b3);
    }

    (h1, h2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_fallback_has_expected_dim_and_unit_norm() {
        let model = ModelRegistry::new_static();
        assert_eq!(model.model_id, DEFAULT_MODEL_ID);
        assert!(!model.has_static_table());

        let vector = model.embed("hello world, this is Kensaku");
        assert_eq!(vector.len(), EMBEDDING_DIM);
        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}");

        let quantized = model.embed_to_i8("hello world");
        assert_eq!(quantized.len(), EMBEDDING_DIM);
    }

    #[test]
    fn default_is_the_hashing_fallback() {
        let model = ModelRegistry::default();
        assert_eq!(model.model_id, DEFAULT_MODEL_ID);
        assert!(!model.has_static_table());
    }
}
