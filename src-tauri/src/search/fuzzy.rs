use crate::indexer::walker::FileEntry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Computes a 64-bit character presence bitmask for fast prefiltering.
pub fn compute_char_mask(text: &str) -> u64 {
    let mut mask = 0u64;
    for ch in text.chars() {
        let ch_lower = ch.to_ascii_lowercase();
        let bit = match ch_lower {
            'a'..='z' => ch_lower as u8 - b'a',
            '0'..='9' => 26 + (ch_lower as u8 - b'0'),
            '_' => 36,
            '-' => 37,
            '.' => 38,
            '/' | '\\' => 39,
            ' ' => 40,
            _ => 41 + ((ch as u32 % 23) as u8),
        };
        mask |= 1u64 << (bit & 63);
    }
    mask
}

pub struct FuzzySearcher {
    matcher: Matcher,
}

impl Default for FuzzySearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzySearcher {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        }
    }

    pub fn search(&mut self, query: &str, entries: &[FileEntry], limit: usize) -> Vec<(usize, u32)> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let pattern = Pattern::parse(trimmed, CaseMatching::Ignore, Normalization::Smart);
        let mut char_buf = Vec::new();
        let mut scored_indices: Vec<(usize, u32)> = Vec::new();

        let query_mask = compute_char_mask(trimmed);
        let match_path = trimmed.contains('/') || trimmed.contains('\\');

        for (idx, entry) in entries.iter().enumerate() {
            // Char-bloom prefilter: 64-bit letter mask per name.
            // If query does not contain path separators, the filename must contain all query characters.
            if !match_path && (entry.char_bloom & query_mask) != query_mask {
                continue;
            }

            let path_str = entry.path.to_string_lossy();
            let utf32 = Utf32Str::new(&path_str, &mut char_buf);

            if let Some(score) = pattern.score(utf32, &mut self.matcher) {
                let mut final_score = score;
                let name_utf32 = Utf32Str::new(&entry.name, &mut char_buf);
                if let Some(name_score) = pattern.score(name_utf32, &mut self.matcher) {
                    final_score += name_score * 2;
                }
                scored_indices.push((idx, final_score));
            }
        }

        // Top-k heap/selection using select_nth_unstable: O(N) instead of O(N log N)
        if scored_indices.len() > limit {
            scored_indices.select_nth_unstable_by(limit, |a, b| b.1.cmp(&a.1));
            scored_indices.truncate(limit);
        }
        scored_indices.sort_by_key(|a| std::cmp::Reverse(a.1));

        scored_indices
    }
}
