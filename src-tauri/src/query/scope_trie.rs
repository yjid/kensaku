use crate::indexer::walker::FileEntry;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    dir_indices: Vec<usize>,
}

type ScopeCandidateCache = Mutex<HashMap<(usize, u64), Arc<Vec<usize>>>>;

pub struct DirectoryTrie {
    root: TrieNode,
    // Cached mapping from parent directory index to children indices
    children_map: HashMap<usize, Vec<usize>>,
    // Cache for scope descendant candidate lists: (dir_idx, generation) -> Arc<Vec<usize>>
    scope_cache: ScopeCandidateCache,
    generation: u64,
}

#[derive(Debug, Clone)]
pub struct ScopeResolution {
    pub dir_idx: usize,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum TrieMatchResult {
    Single(ScopeResolution),
    Ambiguous(Vec<ScopeResolution>),
    None,
}

impl DirectoryTrie {
    pub fn build(
        entries: &[FileEntry],
        path_index: &HashMap<PathBuf, usize>,
        generation: u64,
    ) -> Self {
        let mut root = TrieNode::default();
        let mut children_map: HashMap<usize, Vec<usize>> = HashMap::new();

        // 1. Build children map for DFS
        for (idx, entry) in entries.iter().enumerate() {
            if let Some(parent_path) = entry.path.parent()
                && let Some(&parent_idx) = path_index.get(parent_path)
            {
                children_map.entry(parent_idx).or_default().push(idx);
            }
        }

        // 2. Insert all directory names into trie (folded to lowercase)
        for (idx, entry) in entries.iter().enumerate() {
            if entry.is_dir {
                let name_folded = entry.name.to_lowercase();
                let mut current = &mut root;
                for ch in name_folded.chars() {
                    current = current.children.entry(ch).or_default();
                }
                current.dir_indices.push(idx);
            }
        }

        Self {
            root,
            children_map,
            scope_cache: Mutex::new(HashMap::new()),
            generation,
        }
    }

    /// Resolve a directory-looking token or `in:` target: exact > prefix > fuzzy.
    pub fn resolve(
        &self,
        target: &str,
        entries: &[FileEntry],
        path_index: &HashMap<PathBuf, usize>,
    ) -> TrieMatchResult {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return TrieMatchResult::None;
        }

        // 1. If target looks like a path (starts with ~, ., / or has path separator)
        let expanded_path = if trimmed.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let rest = trimmed
                    .trim_start_matches('~')
                    .trim_start_matches(['/', '\\']);
                if rest.is_empty() {
                    home
                } else {
                    home.join(rest)
                }
            } else {
                PathBuf::from(trimmed)
            }
        } else {
            PathBuf::from(trimmed)
        };

        if let Some(&idx) = path_index.get(&expanded_path)
            && entries.get(idx).map(|e| e.is_dir).unwrap_or(false)
        {
            return TrieMatchResult::Single(ScopeResolution {
                dir_idx: idx,
                path: entries[idx].path.clone(),
            });
        }

        // Try matching path suffix / relative path
        let target_norm = trimmed.replace('\\', "/").to_lowercase();
        let target_clean = target_norm.trim_end_matches('/');

        let mut path_matches = Vec::new();
        for (idx, entry) in entries.iter().enumerate() {
            if entry.is_dir {
                let entry_path_str = entry
                    .path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase();
                if entry_path_str.ends_with(target_clean) {
                    path_matches.push(ScopeResolution {
                        dir_idx: idx,
                        path: entry.path.clone(),
                    });
                }
            }
        }

        if path_matches.len() == 1 {
            return TrieMatchResult::Single(path_matches.remove(0));
        } else if path_matches.len() > 1 {
            return TrieMatchResult::Ambiguous(path_matches);
        }

        // 2. Trie match on directory name: exact > prefix > fuzzy
        let name_folded = target_clean.split('/').next_back().unwrap_or(target_clean);

        // 2a. Exact match
        if let Some(indices) = self.find_exact(name_folded) {
            if indices.len() == 1 {
                let idx = indices[0];
                return TrieMatchResult::Single(ScopeResolution {
                    dir_idx: idx,
                    path: entries[idx].path.clone(),
                });
            } else if indices.len() > 1 {
                let candidates = indices
                    .into_iter()
                    .map(|idx| ScopeResolution {
                        dir_idx: idx,
                        path: entries[idx].path.clone(),
                    })
                    .collect();
                return TrieMatchResult::Ambiguous(candidates);
            }
        }

        // 2b. Prefix match
        let prefix_indices = self.find_prefix(name_folded);
        if prefix_indices.len() == 1 {
            let idx = prefix_indices[0];
            return TrieMatchResult::Single(ScopeResolution {
                dir_idx: idx,
                path: entries[idx].path.clone(),
            });
        } else if prefix_indices.len() > 1 {
            let candidates = prefix_indices
                .into_iter()
                .map(|idx| ScopeResolution {
                    dir_idx: idx,
                    path: entries[idx].path.clone(),
                })
                .collect();
            return TrieMatchResult::Ambiguous(candidates);
        }

        // 2c. Subsequence / fuzzy match on directory names
        let mut fuzzy_matches = Vec::new();
        for (idx, entry) in entries.iter().enumerate() {
            if entry.is_dir {
                let entry_name_folded = entry.name.to_lowercase();
                if entry_name_folded.contains(name_folded) {
                    fuzzy_matches.push(ScopeResolution {
                        dir_idx: idx,
                        path: entry.path.clone(),
                    });
                }
            }
        }

        if fuzzy_matches.len() == 1 {
            TrieMatchResult::Single(fuzzy_matches.remove(0))
        } else if fuzzy_matches.len() > 1 {
            TrieMatchResult::Ambiguous(fuzzy_matches)
        } else {
            TrieMatchResult::None
        }
    }

    fn find_exact(&self, name_folded: &str) -> Option<Vec<usize>> {
        let mut current = &self.root;
        for ch in name_folded.chars() {
            current = current.children.get(&ch)?;
        }
        if current.dir_indices.is_empty() {
            None
        } else {
            Some(current.dir_indices.clone())
        }
    }

    fn find_prefix(&self, prefix_folded: &str) -> Vec<usize> {
        let mut current = &self.root;
        for ch in prefix_folded.chars() {
            match current.children.get(&ch) {
                Some(next) => current = next,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        self.collect_all(current, &mut results);
        results
    }

    fn collect_all(&self, node: &TrieNode, out: &mut Vec<usize>) {
        out.extend_from_slice(&node.dir_indices);
        for child in node.children.values() {
            self.collect_all(child, out);
        }
    }

    /// Retrieve all descendant entry indices using DFS with generational cache.
    pub fn get_scope_candidates(&self, dir_idx: usize, current_gen: u64) -> Arc<Vec<usize>> {
        let key = (dir_idx, current_gen);
        {
            let cache = self.scope_cache.lock();
            if let Some(cached) = cache.get(&key) {
                return Arc::clone(cached);
            }
        }

        // Compute DFS traversal over child lists: O(subtree)
        let mut candidates = Vec::new();
        candidates.push(dir_idx);
        self.dfs_collect(dir_idx, &mut candidates);

        let arc = Arc::new(candidates);
        let mut cache = self.scope_cache.lock();
        // Evict stale generations if generation changed
        if self.generation != current_gen {
            cache.retain(|&(_, cached_generation), _| cached_generation == current_gen);
        }
        cache.insert(key, Arc::clone(&arc));
        arc
    }

    fn dfs_collect(&self, dir_idx: usize, out: &mut Vec<usize>) {
        if let Some(children) = self.children_map.get(&dir_idx) {
            for &child_idx in children {
                out.push(child_idx);
                self.dfs_collect(child_idx, out);
            }
        }
    }
}
