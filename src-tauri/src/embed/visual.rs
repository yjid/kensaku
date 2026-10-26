//! Visual embedding storage and inference pipeline (K18).
//!
//! Provides `VisualStore` for holding 512-dimensional quantized i8 vectors
//! per image node, image preprocessing (224x224 RGB with CLIP normalization),
//! and vision encoder inference (ONNX with deterministic mock fallback).

use crate::embed::model::ModelRegistry;
use image::{DynamicImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const VISUAL_DIM: usize = 512;
pub const MOCK_VISUAL_MODEL_ID: u32 = 100;
pub const ONNX_VISUAL_MODEL_ID: u32 = 101;

/// Standard CLIP image normalization constants.
pub const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
pub const CLIP_STD: [f32; 3] = [0.26862954, 0.261_302_6, 0.275_777_1];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualUnit {
    pub node_id: u64,
    pub model_id: u32,
    pub dhash: u64,
}

/// Contiguous in-memory store for image visual embeddings.
#[derive(Debug, Clone)]
pub struct VisualStore {
    pub units: Vec<VisualUnit>,
    pub vectors: Vec<i8>,
    pub node_to_idx: HashMap<u64, usize>,
    pub dim: usize,
    pub model_id: u32,
}

impl Default for VisualStore {
    fn default() -> Self {
        Self::new(VISUAL_DIM, MOCK_VISUAL_MODEL_ID)
    }
}

impl VisualStore {
    pub fn new(dim: usize, model_id: u32) -> Self {
        Self {
            units: Vec::new(),
            vectors: Vec::new(),
            node_to_idx: HashMap::new(),
            dim,
            model_id,
        }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn clear(&mut self) {
        self.units.clear();
        self.vectors.clear();
        self.node_to_idx.clear();
    }

    /// True if `node_id` is not present, or was embedded with a different model or dHash.
    pub fn needs_embedding(&self, node_id: u64, current_dhash: u64) -> bool {
        match self.node_to_idx.get(&node_id) {
            Some(&idx) => {
                let unit = &self.units[idx];
                unit.model_id != self.model_id || unit.dhash != current_dhash
            }
            None => true,
        }
    }

    pub fn add_or_update(&mut self, node_id: u64, dhash: u64, vector: &[i8]) {
        if vector.len() != self.dim {
            return;
        }

        if let Some(&idx) = self.node_to_idx.get(&node_id) {
            self.units[idx] = VisualUnit {
                node_id,
                model_id: self.model_id,
                dhash,
            };
            let start = idx * self.dim;
            let end = start + self.dim;
            self.vectors[start..end].copy_from_slice(vector);
        } else {
            let idx = self.units.len();
            self.node_to_idx.insert(node_id, idx);
            self.units.push(VisualUnit {
                node_id,
                model_id: self.model_id,
                dhash,
            });
            self.vectors.extend_from_slice(vector);
        }
    }

    pub fn get_vector(&self, node_id: u64) -> Option<&[i8]> {
        let &idx = self.node_to_idx.get(&node_id)?;
        let start = idx * self.dim;
        let end = start + self.dim;
        Some(&self.vectors[start..end])
    }

    pub fn remove_node(&mut self, node_id: u64) {
        if let Some(idx) = self.node_to_idx.remove(&node_id) {
            let last_idx = self.units.len() - 1;
            if idx == last_idx {
                self.units.pop();
                self.vectors.truncate(idx * self.dim);
            } else {
                let moved_node = self.units[last_idx].node_id;
                self.units.swap_remove(idx);
                let start_idx = idx * self.dim;
                let start_last = last_idx * self.dim;
                for d in 0..self.dim {
                    self.vectors[start_idx + d] = self.vectors[start_last + d];
                }
                self.vectors.truncate(last_idx * self.dim);
                self.node_to_idx.insert(moved_node, idx);
            }
        }
    }

    pub fn scan(
        &self,
        query_vec: &[i8],
        allowed_nodes: Option<&HashSet<u64>>,
        floor: f32,
        limit: usize,
    ) -> Vec<(u64, f32)> {
        if query_vec.len() != self.dim || self.units.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();
        for (idx, unit) in self.units.iter().enumerate() {
            if unit.model_id != self.model_id {
                continue;
            }
            if let Some(allowed) = allowed_nodes
                && !allowed.contains(&unit.node_id)
            {
                continue;
            }
            let start = idx * self.dim;
            let end = start + self.dim;
            let sim = ModelRegistry::cosine_similarity_i8(query_vec, &self.vectors[start..end]);
            if sim >= floor {
                hits.push((unit.node_id, sim));
            }
        }

        hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if hits.len() > limit {
            hits.truncate(limit);
        }
        hits
    }
}

/// Preprocesses any dynamic image into 224x224 RGB planar float tensor [1, 3, 224, 224].
pub fn preprocess_image(img: &DynamicImage) -> Vec<f32> {
    let resized = img.resize_exact(224, 224, FilterType::Triangle).to_rgb8();
    let num_pixels = 224 * 224;
    let mut tensor = vec![0.0f32; 3 * num_pixels];

    for y in 0..224 {
        for x in 0..224 {
            let pixel = resized.get_pixel(x, y);
            let idx = (y * 224 + x) as usize;
            // Planar format NCHW: R, then G, then B
            tensor[idx] = ((pixel[0] as f32 / 255.0) - CLIP_MEAN[0]) / CLIP_STD[0];
            tensor[num_pixels + idx] = ((pixel[1] as f32 / 255.0) - CLIP_MEAN[1]) / CLIP_STD[1];
            tensor[2 * num_pixels + idx] = ((pixel[2] as f32 / 255.0) - CLIP_MEAN[2]) / CLIP_STD[2];
        }
    }

    tensor
}

/// Abstract vision encoder trait.
pub trait VisualEncoder: Send + Sync {
    fn model_id(&self) -> u32;
    fn dim(&self) -> usize;
    fn embed_image(&self, img: &DynamicImage) -> Vec<i8>;
    fn embed_image_from_thumbnail(&self, thumb_path: &Path) -> Option<Vec<i8>> {
        let img = image::open(thumb_path).ok()?;
        Some(self.embed_image(&img))
    }
    fn embed_text(&self, text: &str) -> Vec<i8>;
}

/// Deterministic mock vision encoder used when ONNX visual artifacts are absent.
#[derive(Debug, Clone, Default)]
pub struct MockVisualEncoder;

impl VisualEncoder for MockVisualEncoder {
    fn model_id(&self) -> u32 {
        MOCK_VISUAL_MODEL_ID
    }

    fn dim(&self) -> usize {
        VISUAL_DIM
    }

    fn embed_image(&self, img: &DynamicImage) -> Vec<i8> {
        let tensor = preprocess_image(img);
        let mut vec = vec![0.0f32; VISUAL_DIM];

        // Project 224x224x3 into 512 dimensions using deterministic pseudo-random projection
        for (i, &val) in tensor.iter().enumerate() {
            let target_dim = (i * 31 + 17) % VISUAL_DIM;
            let sign = if (i * 13 + 7).is_multiple_of(2) {
                1.0f32
            } else {
                -1.0f32
            };
            vec[target_dim] += val * sign;
        }

        // L2 normalize
        let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
        if norm_sq > 0.0 {
            let norm = norm_sq.sqrt();
            for v in &mut vec {
                *v /= norm;
            }
        }

        vec.iter()
            .map(|&v| (v * 127.0).round().clamp(-127.0, 127.0) as i8)
            .collect()
    }

    fn embed_text(&self, text: &str) -> Vec<i8> {
        let mut vec = vec![0.0f32; VISUAL_DIM];
        for (i, b) in text.as_bytes().iter().enumerate() {
            let target_dim = (i * 37 + *b as usize * 19) % VISUAL_DIM;
            let sign = if (*b as usize + i).is_multiple_of(2) {
                1.0f32
            } else {
                -1.0f32
            };
            vec[target_dim] += sign;
        }

        let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
        if norm_sq > 0.0 {
            let norm = norm_sq.sqrt();
            for v in &mut vec {
                *v /= norm;
            }
        }

        vec.iter()
            .map(|&v| (v * 127.0).round().clamp(-127.0, 127.0) as i8)
            .collect()
    }
}

/// Checks `cancel_signal` during processing.
pub fn is_cancelled(cancel_signal: Option<&AtomicBool>) -> bool {
    cancel_signal
        .map(|s| s.load(Ordering::Relaxed))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_test_image(r: u8, g: u8, b: u8) -> DynamicImage {
        let mut img = RgbImage::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb([r, g, b]);
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn pipeline_extracts_vector_from_test_image() {
        let encoder = MockVisualEncoder;
        let img = make_test_image(200, 50, 50);
        let vec = encoder.embed_image(&img);
        assert_eq!(vec.len(), VISUAL_DIM);
        assert!(vec.iter().any(|&v| v != 0));
    }

    #[test]
    fn mock_encoder_produces_deterministic_vector() {
        let encoder = MockVisualEncoder;
        let img1 = make_test_image(120, 150, 180);
        let img2 = make_test_image(120, 150, 180);

        let vec1 = encoder.embed_image(&img1);
        let vec2 = encoder.embed_image(&img2);
        assert_eq!(vec1, vec2);

        let img_diff = make_test_image(0, 0, 0);
        let vec_diff = encoder.embed_image(&img_diff);
        assert_ne!(vec1, vec_diff);
    }

    #[test]
    fn thumbnail_reuse_does_not_require_source_file() {
        let encoder = MockVisualEncoder;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("kensaku-thumb-test-{}", nanos));
        fs::create_dir_all(&temp_dir).unwrap();

        let thumb_path = temp_dir.join("thumb.jpg");
        let img = make_test_image(255, 100, 50);
        img.save(&thumb_path).unwrap();

        // embed_image_from_thumbnail operates directly on the thumbnail file
        let vec = encoder.embed_image_from_thumbnail(&thumb_path);
        assert!(vec.is_some());
        assert_eq!(vec.unwrap().len(), VISUAL_DIM);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn unchanged_image_skips_re_embedding() {
        let mut store = VisualStore::new(VISUAL_DIM, MOCK_VISUAL_MODEL_ID);
        let node_id = 42;
        let dhash = 0x12345678_abcdef00;
        let vector = vec![1i8; VISUAL_DIM];

        assert!(store.needs_embedding(node_id, dhash));
        store.add_or_update(node_id, dhash, &vector);

        // Same dhash and same model -> skip
        assert!(!store.needs_embedding(node_id, dhash));

        // Changed dhash -> needs re-embedding
        assert!(store.needs_embedding(node_id, 0x99999999));

        // Different model -> needs re-embedding
        store.model_id = MOCK_VISUAL_MODEL_ID + 1;
        assert!(store.needs_embedding(node_id, dhash));
    }

    #[test]
    fn cancel_safety_yields_on_query_signal() {
        let cancel = AtomicBool::new(false);
        assert!(!is_cancelled(Some(&cancel)));

        cancel.store(true, Ordering::Relaxed);
        assert!(is_cancelled(Some(&cancel)));
    }
}
