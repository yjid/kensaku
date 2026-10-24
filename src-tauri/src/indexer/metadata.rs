use crate::indexer::walker::FileEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct MetadataStore {
    entries: Vec<FileEntry>,
    path_index: HashMap<PathBuf, usize>,
}

impl MetadataStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            path_index: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.path_index.clear();
    }

    pub fn add(&mut self, entry: FileEntry) {
        if let Some(&idx) = self.path_index.get(&entry.path) {
            self.entries[idx] = entry;
        } else {
            let idx = self.entries.len();
            self.path_index.insert(entry.path.clone(), idx);
            self.entries.push(entry);
        }
    }

    pub fn remove(&mut self, path: &Path) {
        if let Some(idx) = self.path_index.remove(path) {
            let last_idx = self.entries.len().saturating_sub(1);
            if idx < last_idx {
                self.entries.swap(idx, last_idx);
                let swapped_path = self.entries[idx].path.clone();
                self.path_index.insert(swapped_path, idx);
            }
            self.entries.pop();
        }
    }

    pub fn get_entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
