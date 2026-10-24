use crate::indexer::metadata_db::MetadataDb;
use crate::indexer::walker::FileEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct MetadataStore {
    entries: Vec<FileEntry>,
    path_index: HashMap<PathBuf, usize>,
    db: Option<MetadataDb>,
}

impl MetadataStore {
    pub fn new() -> Self {
        let db = match MetadataDb::open() {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("[Kensaku] Failed to open metadata database: {}", e);
                None
            }
        };

        let mut store = Self {
            entries: Vec::new(),
            path_index: HashMap::new(),
            db,
        };

        if store.db.is_some() {
            let loaded = store.load_from_db();
            if loaded > 0 {
                println!("[Kensaku] Loaded {} metadata entries from database", loaded);
            }
        }

        store
    }

    /// Reload the in-memory index from the database, replacing any current state.
    /// Returns the number of entries loaded (0 if the database is unavailable).
    pub fn load_from_db(&mut self) -> usize {
        let db = match self.db.as_ref() {
            Some(db) => db,
            None => return 0,
        };

        let entries = match db.load_all() {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("[Kensaku] Failed to load metadata from database: {}", e);
                return 0;
            }
        };

        self.entries.clear();
        self.path_index.clear();

        let count = entries.len();
        for entry in entries {
            let idx = self.entries.len();
            self.path_index.insert(entry.path.clone(), idx);
            self.entries.push(entry);
        }

        count
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.path_index.clear();

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.clear()
        {
            eprintln!("[Kensaku] Failed to clear metadata database: {}", e);
        }
    }

    pub fn add(&mut self, entry: FileEntry) {
        let idx = if let Some(&idx) = self.path_index.get(&entry.path) {
            self.entries[idx] = entry;
            idx
        } else {
            let idx = self.entries.len();
            self.path_index.insert(entry.path.clone(), idx);
            self.entries.push(entry);
            idx
        };

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.upsert(&self.entries[idx])
        {
            eprintln!(
                "[Kensaku] Failed to persist metadata entry '{}': {}",
                self.entries[idx].path.display(),
                e
            );
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

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.remove(path)
        {
            eprintln!(
                "[Kensaku] Failed to delete metadata entry '{}': {}",
                path.display(),
                e
            );
        }
    }

    pub fn get(&self, path: &Path) -> Option<&FileEntry> {
        self.path_index.get(path).map(|&idx| &self.entries[idx])
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
