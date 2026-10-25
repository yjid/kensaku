//! Dependency-light loader for the Potion static embedding table.
//!
//! The three artifacts downloaded by [`crate::models`] are:
//!
//! * `model.safetensors` - a single `[vocab, dim]` embedding matrix,
//! * `tokenizer.json` - a HuggingFace BERT WordPiece tokenizer,
//! * `config.json` - model hyper-parameters (normalization, hidden size).
//!
//! This module parses all three with only `serde_json` plus the standard
//! library. Anything malformed or unsupported is rejected with `None`, which
//! callers translate into the hashing fallback in [`crate::embed::model`].

use crate::embed::model::EMBEDDING_DIM;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Hard cap on the number of WordPiece ids returned per text.
pub const MAX_TOKENS: usize = 512;

/// Upper bound for the safetensors JSON header. Real headers are a few KB;
/// this only exists so a corrupt length prefix cannot allocate gigabytes.
const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;

/// An in-memory static embedding table plus the WordPiece tokenizer needed to
/// index into it. Both are immutable after construction; hold one per process.
#[derive(Debug)]
pub struct StaticTable {
    /// Row-major `[vocab, dim]` embedding matrix.
    pub embeddings: Vec<f32>,
    pub vocab: HashMap<String, u32>,
    pub dim: usize,
    pub unknown_id: u32,
    pub lowercase: bool,
    pub normalize: bool,
    pub model_id: u32,
    continuing_prefix: String,
    max_input_chars: usize,
}

impl StaticTable {
    /// Loads `model.safetensors`, `tokenizer.json` and `config.json` from `dir`.
    ///
    /// Returns `None` when any required artifact is missing or unusable, when
    /// the embedding matrix is not 2-D, or when its width is not
    /// [`EMBEDDING_DIM`]. Callers should fall back to the hashing embedder.
    pub fn load(dir: &Path) -> Option<Self> {
        let mut file = File::open(dir.join("model.safetensors")).ok()?;
        let (embeddings, vocab_size, dim) = parse_safetensors(&mut file)?;
        if dim != EMBEDDING_DIM {
            return None;
        }

        let tokenizer_bytes = std::fs::read(dir.join("tokenizer.json")).ok()?;
        let tokenizer: Value = serde_json::from_slice(&tokenizer_bytes).ok()?;
        let tokenizer_parts = parse_tokenizer(&tokenizer, vocab_size)?;

        let normalize = match std::fs::read(dir.join("config.json")) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(config) => {
                    // `hidden_size` is the transformers name; model2vec uses
                    // `hidden_dim`. Accept either, and reject a mismatch.
                    if let Some(size) = config
                        .get("hidden_size")
                        .or_else(|| config.get("hidden_dim"))
                        .and_then(|v| v.as_u64())
                        && size as usize != dim
                    {
                        return None;
                    }
                    config
                        .get("normalize")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true)
                }
                // A corrupt config is not fatal: defaults are safe.
                Err(_) => true,
            },
            Err(_) => true,
        };

        Some(Self {
            embeddings,
            vocab: tokenizer_parts.vocab,
            dim,
            unknown_id: tokenizer_parts.unknown_id,
            lowercase: tokenizer_parts.lowercase,
            normalize,
            model_id: crate::embed::model::REAL_MODEL_ID,
            continuing_prefix: tokenizer_parts.continuing_prefix,
            max_input_chars: tokenizer_parts.max_input_chars,
        })
    }

    /// Encodes `text` to WordPiece ids, without any special tokens.
    pub fn token_ids(&self, text: &str) -> Vec<u32> {
        let mut ids = Vec::new();
        for word in self.pre_tokenize(text) {
            self.tokenize_word(&word, &mut ids);
            if ids.len() >= MAX_TOKENS {
                ids.truncate(MAX_TOKENS);
                break;
            }
        }
        ids
    }

    /// Mean-pools token embedding rows, then L2-normalizes when configured.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let mut out = vec![0.0f32; self.dim];
        let ids = self.token_ids(text);
        if ids.is_empty() {
            return out;
        }

        let mut used = 0usize;
        for id in ids {
            let start = id as usize * self.dim;
            let end = start + self.dim;
            if end > self.embeddings.len() {
                continue;
            }
            for (slot, value) in out.iter_mut().zip(&self.embeddings[start..end]) {
                *slot += *value;
            }
            used += 1;
        }

        if used == 0 {
            return out;
        }
        let scale = 1.0 / used as f32;
        for value in &mut out {
            *value *= scale;
        }

        if self.normalize {
            l2_normalize(&mut out);
        }
        out
    }

    /// BERT-ish normalization + pre-tokenization: lowercase, drop control
    /// characters, collapse whitespace, and isolate punctuation and CJK.
    fn pre_tokenize(&self, text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            if ch.is_whitespace() {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            } else if ch.is_control() {
                continue;
            } else if is_punctuation(ch) || is_cjk(ch) {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                words.push(lowercase_char(ch, self.lowercase));
            } else if self.lowercase {
                current.extend(ch.to_lowercase());
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            words.push(current);
        }
        words
    }

    /// Greedy longest-match WordPiece with the `##` continuation prefix.
    fn tokenize_word(&self, word: &str, ids: &mut Vec<u32>) {
        if word.is_empty() {
            return;
        }

        // Fast path: the whole word is a token.
        if let Some(&id) = self.vocab.get(word) {
            ids.push(id);
            return;
        }

        let chars: Vec<char> = word.chars().collect();
        if chars.len() > self.max_input_chars {
            ids.push(self.unknown_id);
            return;
        }

        let mut start = 0usize;
        let mut pieces = Vec::new();
        while start < chars.len() {
            let mut end = chars.len();
            let mut matched: Option<u32> = None;
            while end > start {
                let mut piece: String = chars[start..end].iter().collect();
                if start > 0 {
                    piece.insert_str(0, &self.continuing_prefix);
                }
                if let Some(&id) = self.vocab.get(&piece) {
                    matched = Some(id);
                    break;
                }
                end -= 1;
            }

            match matched {
                Some(id) => {
                    pieces.push(id);
                    start = end;
                }
                None => {
                    // A single unmatched character invalidates the whole word.
                    ids.push(self.unknown_id);
                    return;
                }
            }
        }
        ids.extend(pieces);
    }

    #[cfg(test)]
    fn test_table(embeddings: Vec<f32>, vocab: &[(&str, u32)], dim: usize) -> Self {
        Self {
            embeddings,
            vocab: vocab.iter().map(|(t, id)| (t.to_string(), *id)).collect(),
            dim,
            unknown_id: 1,
            lowercase: true,
            normalize: true,
            model_id: crate::embed::model::REAL_MODEL_ID,
            continuing_prefix: "##".to_string(),
            max_input_chars: 100,
        }
    }
}

fn lowercase_char(ch: char, lowercase: bool) -> String {
    if lowercase {
        ch.to_lowercase().collect()
    } else {
        ch.to_string()
    }
}

/// BERT's punctuation definition (ASCII ranges), which is what
/// `BertPreTokenizer` splits on.
fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

/// `BertNormalizer` surrounds CJK ideographs/kana/hangul with spaces so each
/// becomes its own pre-token. Roughly mirrors `handle_chinese_chars`.
fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30FF       // hiragana + katakana
            | 0x3400..=0x4DBF // CJK extension A
            | 0x4E00..=0x9FFF // CJK unified ideographs
            | 0xAC00..=0xD7AF // hangul syllables
            | 0xF900..=0xFAFF // CJK compatibility ideographs
    )
}

fn l2_normalize(values: &mut [f32]) {
    let norm_sq: f32 = values.iter().map(|v| v * v).sum();
    if norm_sq > 0.0 {
        let norm = norm_sq.sqrt();
        for value in values {
            *value /= norm;
        }
    }
}

struct TokenizerParts {
    vocab: HashMap<String, u32>,
    unknown_id: u32,
    lowercase: bool,
    continuing_prefix: String,
    max_input_chars: usize,
}

fn parse_tokenizer(value: &Value, vocab_size: usize) -> Option<TokenizerParts> {
    let model = value.get("model")?;
    let vocab_object = model.get("vocab")?.as_object()?;

    let mut vocab = HashMap::with_capacity(vocab_object.len());
    for (token, id) in vocab_object {
        let Some(id) = id.as_u64() else { continue };
        if (id as usize) < vocab_size {
            vocab.insert(token.clone(), id as u32);
        }
    }
    if vocab.is_empty() {
        return None;
    }

    let unk_token = model
        .get("unk_token")
        .and_then(|v| v.as_str())
        .unwrap_or("[UNK]");
    let unknown_id = vocab.get(unk_token).copied().unwrap_or(0);
    if unknown_id as usize >= vocab_size {
        return None;
    }

    let continuing_prefix = model
        .get("continuing_subword_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("##")
        .to_string();
    let max_input_chars = model
        .get("max_input_chars_per_word")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    let normalizer = value.get("normalizer");
    let lowercase = normalizer
        .and_then(|n| n.get("lowercase"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            normalizer
                .and_then(|n| n.get("do_lower_case"))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(true);

    Some(TokenizerParts {
        vocab,
        unknown_id,
        lowercase,
        continuing_prefix,
        max_input_chars,
    })
}

/// Parses one safetensors file and returns `(embeddings, vocab_size, dim)`.
fn parse_safetensors<R: Read + Seek>(reader: &mut R) -> Option<(Vec<f32>, usize, usize)> {
    let mut length_bytes = [0u8; 8];
    reader.read_exact(&mut length_bytes).ok()?;
    let header_len = u64::from_le_bytes(length_bytes);
    if header_len == 0 || header_len > MAX_HEADER_BYTES {
        return None;
    }
    let header_len = header_len as usize;

    let mut header_bytes = vec![0u8; header_len];
    reader.read_exact(&mut header_bytes).ok()?;
    let header: Value = serde_json::from_slice(&header_bytes).ok()?;
    let tensors = header.as_object()?;

    let selected = if let Some(tensor) = tensors.get("embeddings") {
        tensor_info(tensor)?
    } else {
        // No `embeddings` key: accept only an unambiguous 2-D tensor.
        let mut found = None;
        for (name, tensor) in tensors {
            if name == "__metadata__" {
                continue;
            }
            if let Some(info) = tensor_info(tensor)
                && info.1.len() == 2
            {
                if found.is_some() {
                    return None;
                }
                found = Some(info);
            }
        }
        found?
    };

    let (dtype, shape, begin, end) = selected;
    if shape.len() != 2 {
        return None;
    }
    let vocab_size = shape[0];
    let dim = shape[1];
    if dim != EMBEDDING_DIM {
        return None;
    }
    let item_size: u64 = match dtype.as_str() {
        "F32" => 4,
        "F16" => 2,
        _ => return None,
    };
    if end < begin {
        return None;
    }
    let expected = (vocab_size as u64)
        .checked_mul(dim as u64)?
        .checked_mul(item_size)?;
    if end - begin != expected {
        return None;
    }

    let data_start = 8u64.checked_add(header_len as u64)?;
    reader
        .seek(SeekFrom::Start(data_start.checked_add(begin)?))
        .ok()?;

    let mut raw = vec![0u8; expected as usize];
    reader.read_exact(&mut raw).ok()?;

    let embeddings = if dtype == "F32" {
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    } else {
        raw.as_chunks::<2>()
            .0
            .iter()
            .map(|c| f16_to_f32(u16::from_le_bytes(*c)))
            .collect()
    };

    Some((embeddings, vocab_size, dim))
}

fn tensor_info(value: &Value) -> Option<(String, Vec<usize>, u64, u64)> {
    let dtype = value.get("dtype")?.as_str()?.to_string();
    let shape = value
        .get("shape")?
        .as_array()?
        .iter()
        .map(|v| v.as_u64().map(|n| n as usize))
        .collect::<Option<Vec<usize>>>()?;
    let offsets = value.get("data_offsets")?.as_array()?;
    if offsets.len() != 2 {
        return None;
    }
    Some((dtype, shape, offsets[0].as_u64()?, offsets[1].as_u64()?))
}

/// IEEE 754 half-precision to single-precision conversion (no dependency).
fn f16_to_f32(bits: u16) -> f32 {
    let sign = (bits >> 15) & 1;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = bits & 0x03ff;

    let magnitude = if exponent == 0 {
        // Subnormal (or zero).
        if fraction == 0 {
            0.0
        } else {
            (fraction as f32) * (2.0f32).powi(-24)
        }
    } else if exponent == 0x1f {
        // Infinity / NaN.
        if fraction == 0 {
            f32::INFINITY
        } else {
            f32::NAN
        }
    } else {
        (1.0 + fraction as f32 / 1024.0) * (2.0f32).powi(exponent as i32 - 15)
    };

    if sign == 1 { -magnitude } else { magnitude }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn safetensors_bytes(dtype: &str, shape: &[usize], data: &[u8]) -> Vec<u8> {
        let header = serde_json::json!({
            "embeddings": {
                "dtype": dtype,
                "shape": shape,
                "data_offsets": [0, data.len()],
            }
        });
        let header = serde_json::to_vec(&header).unwrap();
        let mut bytes = Vec::with_capacity(8 + header.len() + data.len());
        bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(data);
        bytes
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kensaku-table-test-{}-{}-{}",
            std::process::id(),
            tag,
            nanos
        ))
    }

    #[test]
    fn parses_f32_safetensors_header() {
        let mut data = vec![0u8; EMBEDDING_DIM * 4];
        for (index, value) in [1.0f32, 2.0, 3.5, -4.0].into_iter().enumerate() {
            data[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        let bytes = safetensors_bytes("F32", &[1, EMBEDDING_DIM], &data);
        let mut reader = Cursor::new(bytes);
        let (embeddings, vocab_size, dim) = parse_safetensors(&mut reader).expect("parse");

        assert_eq!(vocab_size, 1);
        assert_eq!(dim, EMBEDDING_DIM);
        assert_eq!(embeddings.len(), EMBEDDING_DIM);
        assert_eq!(&embeddings[..4], &[1.0, 2.0, 3.5, -4.0]);
    }

    #[test]
    fn parses_f16_safetensors_header() {
        // 1.0, -2.0, 0.5, 0.0 in IEEE 754 half precision.
        let values: [u16; 4] = [0x3c00, 0xc000, 0x3800, 0x0000];
        let mut data = Vec::new();
        for value in values {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let mut header_data = data.clone();
        header_data.resize(EMBEDDING_DIM * 2, 0);
        let bytes = safetensors_bytes("F16", &[1, EMBEDDING_DIM], &header_data);
        let mut reader = Cursor::new(bytes);
        let (embeddings, vocab_size, dim) = parse_safetensors(&mut reader).expect("parse");

        assert_eq!(vocab_size, 1);
        assert_eq!(dim, EMBEDDING_DIM);
        assert_eq!(&embeddings[..4], &[1.0, -2.0, 0.5, 0.0]);
    }

    #[test]
    fn rejects_wrong_embedding_width() {
        let data = vec![0u8; 3 * 4];
        let bytes = safetensors_bytes("F32", &[1, 3], &data);
        let mut reader = Cursor::new(bytes);
        assert!(parse_safetensors(&mut reader).is_none());
    }

    #[test]
    fn wordpiece_tokenization() {
        // 128 rows is more than enough for this tiny vocab.
        let table = StaticTable::test_table(
            vec![0.0; 8 * EMBEDDING_DIM],
            &[
                ("[PAD]", 0),
                ("[UNK]", 1),
                ("hello", 2),
                ("world", 3),
                ("##ing", 4),
                ("play", 5),
                ("##ed", 6),
            ],
            EMBEDDING_DIM,
        );

        assert_eq!(table.token_ids("hello world"), vec![2, 3]);
        assert_eq!(table.token_ids("playing"), vec![5, 4]);
        assert_eq!(table.token_ids("played"), vec![5, 6]);
        assert_eq!(table.token_ids("zzz"), vec![1]);
        assert_eq!(table.token_ids("hello zzz world"), vec![2, 1, 3]);
        // Lowercased before lookup, punctuation isolated, whitespace collapsed.
        assert_eq!(table.token_ids("  Hello,\tWORLD!  "), vec![2, 1, 3, 1]);
    }

    #[test]
    fn static_embedding_is_mean_pooled_and_normalized() {
        let mut embeddings = vec![0.0f32; 8 * EMBEDDING_DIM];
        // Row 2 = all ones, row 3 = all twos.
        for v in &mut embeddings[2 * EMBEDDING_DIM..3 * EMBEDDING_DIM] {
            *v = 1.0;
        }
        for v in &mut embeddings[3 * EMBEDDING_DIM..4 * EMBEDDING_DIM] {
            *v = 2.0;
        }
        let table = StaticTable::test_table(
            embeddings,
            &[("[PAD]", 0), ("[UNK]", 1), ("hello", 2), ("world", 3)],
            EMBEDDING_DIM,
        );

        let vector = table.embed("hello world");
        assert_eq!(vector.len(), EMBEDDING_DIM);
        // Mean of 1s and 2s is 1.5 in every dimension; normalized => 1/sqrt(256).
        let expected = 1.0 / (EMBEDDING_DIM as f32).sqrt();
        for value in &vector {
            assert!((value - expected).abs() < 1e-6, "unexpected value {value}");
        }
        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn load_returns_none_for_missing_directory() {
        let dir = temp_dir("missing");
        assert!(StaticTable::load(&dir).is_none());
    }

    #[test]
    fn load_returns_none_for_corrupt_artifacts() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("model.safetensors"), b"not a safetensors file").unwrap();
        std::fs::write(dir.join("tokenizer.json"), b"{ not json").unwrap();
        std::fs::write(dir.join("config.json"), b"{}").unwrap();

        assert!(StaticTable::load(&dir).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
