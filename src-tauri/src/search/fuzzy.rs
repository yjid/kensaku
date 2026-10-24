use crate::indexer::walker::FileEntry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

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
        if query.trim().is_empty() {
            return Vec::new();
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut char_buf = Vec::new();
        let mut scored_indices: Vec<(usize, u32)> = Vec::new();

        for (idx, entry) in entries.iter().enumerate() {
            let path_str = entry.path.to_string_lossy();
            let utf32 = Utf32Str::new(&path_str, &mut char_buf);
            
            if let Some(score) = pattern.score(utf32, &mut self.matcher) {
                // Give a bonus if the filename itself matches well
                let mut final_score = score as u32;
                let name_utf32 = Utf32Str::new(&entry.name, &mut char_buf);
                if let Some(name_score) = pattern.score(name_utf32, &mut self.matcher) {
                    final_score += (name_score as u32) * 2;
                }
                scored_indices.push((idx, final_score));
            }
        }

        scored_indices.sort_by(|a, b| b.1.cmp(&a.1));
        if scored_indices.len() > limit {
            scored_indices.truncate(limit);
        }

        scored_indices
    }
}
