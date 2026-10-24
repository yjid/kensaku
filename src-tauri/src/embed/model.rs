use serde::{Deserialize, Serialize};

pub const EMBEDDING_DIM: usize = 256;
pub const DEFAULT_MODEL_ID: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub model_id: u32,
    pub name: String,
    pub dim: usize,
    pub normalize: bool,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new_static()
    }
}

impl ModelRegistry {
    pub fn new_static() -> Self {
        Self {
            model_id: DEFAULT_MODEL_ID,
            name: "static-embed-256".to_string(),
            dim: EMBEDDING_DIM,
            normalize: true,
        }
    }

    /// Tokenizes input text into normalized terms and character 3-grams for subword coverage.
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
