//! WordPiece tokenizer for BERT-family models (Potion, BGE).

use serde_json::Value;
use std::collections::HashMap;

pub const MAX_TOKENS: usize = 512;

#[derive(Debug, Clone)]
pub struct WordPieceTokenizer {
    pub vocab: HashMap<String, u32>,
    pub unknown_id: u32,
    pub cls_id: u32,
    pub sep_id: u32,
    pub lowercase: bool,
    pub continuing_prefix: String,
    pub max_input_chars: usize,
}

impl WordPieceTokenizer {
    pub fn parse(value: &Value, vocab_size_opt: Option<usize>) -> Option<Self> {
        let model = value.get("model")?;
        let vocab_object = model.get("vocab")?.as_object()?;
        if let Some(expected_size) = vocab_size_opt
            && vocab_object.len() != expected_size
        {
            return None;
        }
        let mut vocab = HashMap::with_capacity(vocab_object.len());
        for (k, v) in vocab_object {
            let id = v.as_u64()? as u32;
            vocab.insert(k.clone(), id);
        }

        let unknown_id = vocab.get("[UNK]").copied().unwrap_or(100);
        let cls_id = vocab.get("[CLS]").copied().unwrap_or(101);
        let sep_id = vocab.get("[SEP]").copied().unwrap_or(102);

        let continuing_prefix = model
            .get("continuing_subword_prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("##")
            .to_string();

        let max_input_chars = model
            .get("max_input_chars_per_word")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let lowercase = value
            .get("normalizer")
            .and_then(|n| n.get("lowercase"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Some(Self {
            vocab,
            unknown_id,
            cls_id,
            sep_id,
            lowercase,
            continuing_prefix,
            max_input_chars,
        })
    }

    pub fn pre_tokenize(&self, text: &str) -> Vec<String> {
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

    pub fn tokenize_word(&self, word: &str, ids: &mut Vec<u32>) {
        if word.is_empty() {
            return;
        }

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
                    ids.push(self.unknown_id);
                    return;
                }
            }
        }
        ids.extend(pieces);
    }

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

    pub fn encode_bert(&self, text: &str, max_len: usize) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
        let inner_cap = max_len.saturating_sub(2);
        let mut ids = Vec::new();
        for word in self.pre_tokenize(text) {
            self.tokenize_word(&word, &mut ids);
            if ids.len() >= inner_cap {
                ids.truncate(inner_cap);
                break;
            }
        }

        let mut input_ids = Vec::with_capacity(ids.len() + 2);
        input_ids.push(self.cls_id as i64);
        for id in ids {
            input_ids.push(id as i64);
        }
        input_ids.push(self.sep_id as i64);

        let seq_len = input_ids.len();
        let attention_mask = vec![1i64; seq_len];
        let token_type_ids = vec![0i64; seq_len];

        (input_ids, attention_mask, token_type_ids)
    }
}

pub fn lowercase_char(ch: char, lowercase: bool) -> String {
    if lowercase {
        ch.to_lowercase().collect()
    } else {
        ch.to_string()
    }
}

pub fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

pub fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30FF       // hiragana + katakana
            | 0x3400..=0x4DBF // CJK extension A
            | 0x4E00..=0x9FFF // CJK unified ideographs
            | 0xAC00..=0xD7AF // hangul syllables
            | 0xF900..=0xFAFF // CJK compatibility ideographs
    )
}

pub fn l2_normalize(values: &mut [f32]) {
    let norm_sq: f32 = values.iter().map(|v| v * v).sum();
    if norm_sq > 0.0 {
        let norm = norm_sq.sqrt();
        for value in values {
            *value /= norm;
        }
    }
}
