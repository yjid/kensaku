use crate::indexer::metadata_db::MetadataDb;
use crate::indexer::walker::FileEntry;
use crate::query::scope_trie::DirectoryTrie;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub struct MetadataStore {
    entries: Vec<FileEntry>,
    path_index: HashMap<PathBuf, usize>,
    node_id_to_idx: HashMap<u64, usize>,
    db: Option<Arc<MetadataDb>>,
    generation: AtomicU64,
    dir_generation: AtomicU64,
    usage: RwLock<HashMap<PathBuf, (u32, u64)>>,
    cached_trie: RwLock<Option<(u64, Arc<DirectoryTrie>)>>,
}

impl Default for MetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataStore {
    pub fn new() -> Self {
        let db = match MetadataDb::open() {
            Ok(db) => Some(Arc::new(db)),
            Err(e) => {
                eprintln!("[Kensaku] Failed to open metadata database: {}", e);
                None
            }
        };

        let mut store = Self {
            entries: Vec::new(),
            path_index: HashMap::new(),
            node_id_to_idx: HashMap::new(),
            db,
            generation: AtomicU64::new(1),
            dir_generation: AtomicU64::new(1),
            usage: RwLock::new(HashMap::new()),
            cached_trie: RwLock::new(None),
        };

        if store.db.is_some() {
            let loaded = store.load_from_db();
            if loaded > 0 {
                println!("[Kensaku] Loaded {} metadata entries from database", loaded);
            }
        }

        store
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Builds a store around an already-open database.
    ///
    /// Unlike [`Self::new`] this never opens the user's config directory and
    /// never writes to stdout, which matters for the headless CLI/MCP surface
    /// (stdout is the protocol stream there).
    pub fn with_db(db: MetadataDb) -> Self {
        let mut store = Self {
            entries: Vec::new(),
            path_index: HashMap::new(),
            node_id_to_idx: HashMap::new(),
            db: Some(Arc::new(db)),
            generation: AtomicU64::new(1),
            dir_generation: AtomicU64::new(1),
            usage: RwLock::new(HashMap::new()),
            cached_trie: RwLock::new(None),
        };
        store.load_from_db();
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

        // Also load usage
        if let Ok(loaded_usage) = db.load_usage() {
            *self.usage.write() = loaded_usage;
        }

        self.entries.clear();
        self.path_index.clear();
        self.node_id_to_idx.clear();

        let count = entries.len();
        for entry in entries {
            let idx = self.entries.len();
            if entry.id > 0 {
                self.node_id_to_idx.insert(entry.id, idx);
            }
            self.path_index.insert(entry.path.clone(), idx);
            self.entries.push(entry);
        }

        self.generation.fetch_add(1, Ordering::SeqCst);
        self.dir_generation.fetch_add(1, Ordering::SeqCst);
        *self.cached_trie.write() = None;

        count
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.path_index.clear();
        self.node_id_to_idx.clear();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.dir_generation.fetch_add(1, Ordering::SeqCst);
        *self.cached_trie.write() = None;

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.clear()
        {
            eprintln!("[Kensaku] Failed to clear metadata database: {}", e);
        }
    }

    /// Adds or updates a file entry, returning its durable row id.
    ///
    /// Callers that go on to extract content (the crawl pipeline, semantic
    /// indexing) need the id right away; returning it here avoids a second
    /// database lookup per file.
    pub fn add(&mut self, entry: FileEntry) -> u64 {
        let is_dir = entry.is_dir;
        let mut entry_id = entry.id;
        let idx = if let Some(&idx) = self.path_index.get(&entry.path) {
            let mut entry = entry;
            // `mark_images_pending` sets this flag; a crawl must not erase it
            // by re-adding walker data before the image has been re-read.
            if self.entries[idx].content_pending {
                entry.content_pending = true;
            }
            self.entries[idx] = entry;
            idx
        } else {
            let idx = self.entries.len();
            self.path_index.insert(entry.path.clone(), idx);
            self.entries.push(entry);
            idx
        };

        // Bump metadata generation for search consistency (metadata changed).
        self.generation.fetch_add(1, Ordering::SeqCst);

        // Only bump dir_generation (and invalidate trie) when a directory changes.
        if is_dir {
            self.dir_generation.fetch_add(1, Ordering::SeqCst);
            *self.cached_trie.write() = None;
        }

        if let Some(db) = self.db.as_ref() {
            match db.upsert(&self.entries[idx]) {
                Ok(id) => {
                    // Keep the in-memory entry attributable to the same row so
                    // semantic units can be attached without another query.
                    self.entries[idx].id = id;
                    entry_id = id;
                    self.node_id_to_idx.insert(id, idx);
                }
                Err(e) => {
                    eprintln!(
                        "[Kensaku] Failed to persist metadata entry '{}': {}",
                        self.entries[idx].path.display(),
                        e
                    );
                }
            }
        }

        entry_id
    }

    /// Marks every in-memory raster image for a content re-read and mirrors the
    /// change into SQLite. Used when the OCR setting changes.
    ///
    /// Returns the number of in-memory entries marked.
    pub fn mark_images_pending(&mut self) -> usize {
        let mut marked = 0;
        for entry in &mut self.entries {
            if !entry.is_dir
                && entry
                    .extension
                    .as_deref()
                    .is_some_and(crate::indexer::extract::image::is_supported_image_extension)
            {
                entry.content_pending = true;
                marked += 1;
            }
        }

        if marked > 0 {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.mark_images_pending()
        {
            eprintln!("[Kensaku] Failed to mark images pending: {}", e);
        }

        marked
    }

    /// Clears the pending flag for `path` in memory and in SQLite.
    pub fn clear_content_pending(&mut self, path: &Path) {
        if let Some(&idx) = self.path_index.get(path)
            && self.entries[idx].content_pending
        {
            self.entries[idx].content_pending = false;
            self.generation.fetch_add(1, Ordering::SeqCst);
        }

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.clear_content_pending(path)
        {
            eprintln!(
                "[Kensaku] Failed to clear pending content for '{}': {}",
                path.display(),
                e
            );
        }
    }

    pub fn remove(&mut self, path: &Path) {
        if let Some(idx) = self.path_index.remove(path) {
            let was_dir = self.entries[idx].is_dir;
            let removed_id = self.entries[idx].id;
            if removed_id > 0 {
                self.node_id_to_idx.remove(&removed_id);
            }
            let last_idx = self.entries.len().saturating_sub(1);
            if idx < last_idx {
                self.entries.swap(idx, last_idx);
                let swapped_path = self.entries[idx].path.clone();
                let swapped_id = self.entries[idx].id;
                self.path_index.insert(swapped_path, idx);
                if swapped_id > 0 {
                    self.node_id_to_idx.insert(swapped_id, idx);
                }
            }
            self.entries.pop();
            self.generation.fetch_add(1, Ordering::SeqCst);
            if was_dir {
                self.dir_generation.fetch_add(1, Ordering::SeqCst);
                *self.cached_trie.write() = None;
            }
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

    /// Rewrites every in-memory entry that is `old` or lives under `old` to
    /// live under `new`, and persists the same rewrite in one database
    /// transaction.
    ///
    /// Bumps the generation counters and invalidates the cached directory trie
    /// so searches see the new layout. Returns the previous paths, which callers
    /// use to remove stale content-index documents.
    pub fn rename_prefix(&mut self, old: &Path, new: &Path) -> Vec<PathBuf> {
        if old.as_os_str().is_empty() || old == new {
            return Vec::new();
        }

        // Entries already sitting at the destination are stale (the rename
        // replaced them); drop them so renames cannot create duplicate paths.
        let conflicts: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|entry| !entry.path.starts_with(old) && entry.path.starts_with(new))
            .map(|entry| entry.path.clone())
            .collect();
        for path in conflicts {
            self.remove(&path);
        }

        let mut old_paths = Vec::new();
        for entry in &mut self.entries {
            let Ok(suffix) = entry.path.strip_prefix(old) else {
                continue;
            };
            old_paths.push(entry.path.clone());

            entry.path = if suffix.as_os_str().is_empty() {
                new.to_path_buf()
            } else {
                new.join(suffix)
            };
            entry.name = entry
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            entry.char_bloom = crate::search::fuzzy::compute_char_mask(&entry.name);
        }

        if old_paths.is_empty() {
            return old_paths;
        }

        // Paths changed but indices did not, so the lookup tables can be
        // rebuilt in place.
        self.path_index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.path_index.insert(entry.path.clone(), idx);
        }

        self.generation.fetch_add(1, Ordering::SeqCst);
        self.dir_generation.fetch_add(1, Ordering::SeqCst);
        *self.cached_trie.write() = None;

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.rename_prefix(old, new)
        {
            eprintln!(
                "[Kensaku] Failed to persist directory rename '{}' -> '{}': {}",
                old.display(),
                new.display(),
                e
            );
        }

        old_paths
    }

    pub fn record_open(&self, path: &Path) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        {
            let mut usage = self.usage.write();
            let entry = usage.entry(path.to_path_buf()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = now;
        }

        if let Some(db) = self.db.as_ref()
            && let Err(e) = db.record_open(path)
        {
            eprintln!("[Kensaku] Failed to persist open in database: {}", e);
        }
    }

    pub fn get_usage(&self, path: &Path) -> (u32, u64) {
        self.usage.read().get(path).copied().unwrap_or((0, 0))
    }

    pub fn get_directory_trie(&self) -> Arc<DirectoryTrie> {
        let current_gen = self.dir_generation.load(Ordering::SeqCst);
        {
            let cache = self.cached_trie.read();
            if let Some((cached_gen, ref trie)) = *cache
                && cached_gen == current_gen
            {
                return Arc::clone(trie);
            }
        }

        let new_trie = Arc::new(DirectoryTrie::build(
            &self.entries,
            &self.path_index,
            current_gen,
        ));
        let mut cache = self.cached_trie.write();
        *cache = Some((current_gen, Arc::clone(&new_trie)));
        new_trie
    }

    pub fn get(&self, path: &Path) -> Option<&FileEntry> {
        self.path_index.get(path).map(|&idx| &self.entries[idx])
    }

    pub fn get_entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn get_path_index(&self) -> &HashMap<PathBuf, usize> {
        &self.path_index
    }

    pub fn get_by_node_id(&self, node_id: u64) -> Option<&FileEntry> {
        self.node_id_to_idx
            .get(&node_id)
            .and_then(|&idx| self.entries.get(idx))
    }

    pub fn db(&self) -> Option<Arc<MetadataDb>> {
        self.db.as_ref().map(Arc::clone)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempStore {
        store: Option<MetadataStore>,
        dir: PathBuf,
    }

    impl TempStore {
        fn new(tag: &str) -> Self {
            let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "kensaku-metadata-store-test-{}-{}-{}",
                tag,
                std::process::id(),
                seq
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            let db = MetadataDb::open_in_dir(&dir).expect("open temp db");
            Self {
                store: Some(MetadataStore::with_db(db)),
                dir,
            }
        }

        fn store(&mut self) -> &mut MetadataStore {
            self.store.as_mut().expect("store open")
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            // Drop the store (and its SQLite handle) before removing the dir.
            self.store.take();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn entry(path: &Path, is_dir: bool) -> FileEntry {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        FileEntry {
            id: 0,
            path: path.to_path_buf(),
            name,
            extension: if is_dir {
                None
            } else {
                path.extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase())
            },
            size: 1,
            modified: 1,
            is_dir,
            dev: 0,
            ino: 0,
            char_bloom: 0,
            content_pending: false,
        }
    }

    #[test]
    fn mark_images_pending_updates_memory_and_only_images() {
        let mut temp = TempStore::new("mark");
        let image = temp.dir.join("photo.jpg");
        let doc = temp.dir.join("notes.txt");
        let dir_path = temp.dir.join("folder.png");

        let store = temp.store();
        store.add(entry(&image, false));
        store.add(entry(&doc, false));
        store.add(entry(&dir_path, true));

        assert_eq!(store.mark_images_pending(), 1);
        assert!(store.get(&image).expect("image").content_pending);
        assert!(!store.get(&doc).expect("doc").content_pending);
        assert!(!store.get(&dir_path).expect("dir").content_pending);

        store.clear_content_pending(&image);
        assert!(!store.get(&image).expect("image").content_pending);

        let db = store.db().expect("database");
        let loaded = db.load_all().expect("load nodes");
        assert!(
            !loaded
                .iter()
                .find(|entry| entry.path == image)
                .expect("image row")
                .content_pending,
            "clear must be persisted"
        );
    }

    #[test]
    fn add_preserves_pending_when_replacing_an_entry() {
        let mut temp = TempStore::new("preserve");
        let image = temp.dir.join("photo.png");

        let mut first = entry(&image, false);
        first.content_pending = true;
        first.size = 10;
        temp.store().add(first);

        let mut replacement = entry(&image, false);
        replacement.size = 20;
        temp.store().add(replacement);

        let stored = temp.store().get(&image).expect("image entry");
        assert!(
            stored.content_pending,
            "replacing an entry must preserve its pending flag"
        );
        assert_eq!(stored.size, 20, "the new metadata must still be applied");
    }
}
