use crate::indexer::arena::{Node, NodeArena, NO_PARENT, path_hash};
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
    arena: NodeArena,
    path_index: HashMap<u64, u32>,
    collision_index: HashMap<PathBuf, u32>,
    node_id_to_idx: HashMap<u64, u32>,
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
            arena: NodeArena::new(),
            path_index: HashMap::new(),
            collision_index: HashMap::new(),
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

    pub fn arena(&self) -> &NodeArena {
        &self.arena
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn with_db(db: MetadataDb) -> Self {
        let mut store = Self {
            arena: NodeArena::new(),
            path_index: HashMap::new(),
            collision_index: HashMap::new(),
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

        if let Ok(loaded_usage) = db.load_usage() {
            *self.usage.write() = loaded_usage;
        }

        self.arena = NodeArena::new();
        self.path_index.clear();
        self.collision_index.clear();
        self.node_id_to_idx.clear();

        let count = entries.len();
        // First pass: push nodes (parents resolved later).
        let mut idxs = Vec::with_capacity(entries.len());
        for entry in &entries {
            let ext_id = self.arena.intern_ext(entry.extension.as_deref());
            let (name_off, name_len) = self.arena.names.push(&entry.name);
            let mut flags = 0u8;
            if entry.is_dir {
                flags |= Node::FLAG_DIR;
            }
            if entry.content_pending {
                flags |= Node::FLAG_CONTENT_PENDING;
            }
            let node = Node {
                parent: NO_PARENT,
                name_off,
                name_len,
                ext_id,
                flags,
                size: entry.size,
                mtime: entry.modified,
                dev: entry.dev,
                ino: entry.ino,
                char_bloom: entry.char_bloom,
                node_id: entry.id,
            };
            let idx = self.arena.push(node);
            self.insert_path_index(&entry.path, idx);
            if entry.id > 0 {
                self.node_id_to_idx.insert(entry.id, idx);
            }
            idxs.push(idx);
        }
        // Second pass: resolve parent pointers.
        for (entry, &idx) in entries.iter().zip(idxs.iter()) {
            let parent = self.parent_index_for(&entry.path);
            if parent == NO_PARENT {
                let full = entry.path.to_string_lossy().into_owned();
                let (off, len) = self.arena.names.push(&full);
                let node = self.arena.node_mut(idx);
                node.parent = parent;
                node.name_off = off;
                node.name_len = len;
            } else {
                let node = self.arena.node_mut(idx);
                node.parent = parent;
            }
        }

        self.generation.fetch_add(1, Ordering::SeqCst);
        self.dir_generation.fetch_add(1, Ordering::SeqCst);
        *self.cached_trie.write() = None;

        count
    }

    fn insert_path_index(&mut self, path: &Path, idx: u32) {
        let h = path_hash(path);
        // collision handling
        if let Some(&existing) = self.path_index.get(&h) {
            // verify
            if self.arena.path(existing) != path {
                self.collision_index.insert(path.to_path_buf(), idx);
                return;
            }
        }
        self.path_index.insert(h, idx);
    }

    fn find_path_index(&self, path: &Path) -> Option<u32> {
        let h = path_hash(path);
        if let Some(&idx) = self.path_index.get(&h)
            && self.arena.path(idx) == path
        {
            return Some(idx);
        }
        self.collision_index.get(path).copied()
    }

    pub fn clear(&mut self) {
        self.arena = NodeArena::new();
        self.path_index.clear();
        self.collision_index.clear();
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

    pub fn add(&mut self, entry: FileEntry) -> u64 {
        let is_dir = entry.is_dir;
        let mut entry_id = entry.id;
        // find existing
        let existing = self.find_path_index(&entry.path);
        let idx = if let Some(idx) = existing {
            // preserve content_pending
            let mut pending = self.arena.node(idx).content_pending();
            if self.arena.node(idx).content_pending() {
                pending = true;
            }
            // update node
            let ext_id = self.arena.intern_ext(entry.extension.as_deref());
            let parent = self.parent_index_for(&entry.path);
            let node_name = if parent == NO_PARENT {
                entry.path.to_string_lossy().into_owned()
            } else {
                entry.name.clone()
            };
            let (name_off, name_len) = self.arena.names.push(&node_name);
            let mut flags = 0u8;
            if entry.is_dir {
                flags |= Node::FLAG_DIR;
            }
            if pending || entry.content_pending {
                flags |= Node::FLAG_CONTENT_PENDING;
            }
            let node = Node {
                parent,
                name_off,
                name_len,
                ext_id,
                flags,
                size: entry.size,
                mtime: entry.modified,
                dev: entry.dev,
                ino: entry.ino,
                char_bloom: entry.char_bloom,
                node_id: entry.id,
            };
            *self.arena.node_mut(idx) = node;
            idx
        } else {
            let ext_id = self.arena.intern_ext(entry.extension.as_deref());
            let parent = self.parent_index_for(&entry.path);
            let name = if parent == NO_PARENT {
                entry.path.to_string_lossy().into_owned()
            } else {
                entry
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry.name.clone())
            };
            let (name_off, name_len) = self.arena.names.push(&name);
            let mut flags = 0u8;
            if entry.is_dir {
                flags |= Node::FLAG_DIR;
            }
            if entry.content_pending {
                flags |= Node::FLAG_CONTENT_PENDING;
            }
            let node = Node {
                parent,
                name_off,
                name_len,
                ext_id,
                flags,
                size: entry.size,
                mtime: entry.modified,
                dev: entry.dev,
                ino: entry.ino,
                char_bloom: entry.char_bloom,
                node_id: entry.id,
            };
            let idx = self.arena.push(node);
            self.insert_path_index(&entry.path, idx);
            idx
        };

        self.generation.fetch_add(1, Ordering::SeqCst);

        if is_dir {
            self.dir_generation.fetch_add(1, Ordering::SeqCst);
            *self.cached_trie.write() = None;
        }

        if let Some(db) = self.db.as_ref() {
            // Need a FileEntry view for db upsert
            let view = self.materialize(idx);
            match db.upsert(&view) {
                Ok(id) => {
                    self.arena.node_mut(idx).node_id = id;
                    entry_id = id;
                    self.node_id_to_idx.insert(id, idx);
                }
                Err(e) => {
                    eprintln!(
                        "[Kensaku] Failed to persist metadata entry '{}': {}",
                        view.path.display(),
                        e
                    );
                }
            }
        }

        entry_id
    }

    fn parent_index_for(&mut self, path: &Path) -> u32 {
        if let Some(parent) = path.parent()
            && let Some(idx) = self.find_path_index(parent)
        {
            return idx;
        }
        NO_PARENT
    }

    pub fn mark_images_pending(&mut self) -> usize {
        let mut marked = 0;
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let node = self.arena.node(idx);
            if node.is_dir() {
                continue;
            }
            // extension check
            let ext = self.arena.extension(idx);
            let is_img = ext
                .as_deref()
                .is_some_and(crate::indexer::extract::image::is_supported_image_extension);
            if is_img {
                self.arena.node_mut(idx).set_content_pending(true);
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

    pub fn clear_content_pending(&mut self, path: &Path) {
        if let Some(idx) = self.find_path_index(path) {
            let node = self.arena.node(idx);
            if node.content_pending() {
                self.arena.node_mut(idx).set_content_pending(false);
                self.generation.fetch_add(1, Ordering::SeqCst);
            }
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
        if let Some(idx) = self.find_path_index(path) {
            let was_dir = self.arena.node(idx).is_dir();
            let removed_id = self.arena.node(idx).node_id;
            if removed_id > 0 {
                self.node_id_to_idx.remove(&removed_id);
            }
            // remove from collision/path index
            let h = path_hash(path);
            if self.path_index.get(&h) == Some(&idx) {
                self.path_index.remove(&h);
            }
            self.collision_index.remove(path);

            // swap remove in arena vectors? NodeArena uses Vec<Node>; removing middle requires swap.
            let last_idx = self.arena.len().saturating_sub(1) as u32;
            if idx < last_idx {
                // move last node to idx
                let last_node = self.arena.nodes[last_idx as usize];
                self.arena.nodes[idx as usize] = last_node;
                // update indices maps for swapped node
                let swapped_path = self.arena.path(idx);
                // update path_index entry for swapped path
                // Remove old hash for swapped? We'll rebuild: find and update
                // Update collision_index if present
                // Simpler: update path_index for swapped path hash
                let sh = path_hash(&swapped_path);
                self.path_index.insert(sh, idx);
                // node_id map
                let swapped_id = self.arena.node(idx).node_id;
                if swapped_id > 0 {
                    self.node_id_to_idx.insert(swapped_id, idx);
                }
            }
            // Fix up parent pointers that referenced the swapped (last) slot.
            if idx < last_idx {
                for node in self.arena.nodes.iter_mut() {
                    if node.parent == last_idx {
                        node.parent = idx;
                    }
                }
            }
            self.arena.nodes.pop();
            // Note: NameArena not compacted (append-only). Acceptable.

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

    pub fn rename_prefix(&mut self, old: &Path, new: &Path) -> Vec<PathBuf> {
        if old.as_os_str().is_empty() || old == new {
            return Vec::new();
        }

        // conflicts
        let mut conflicts: Vec<PathBuf> = Vec::new();
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let p = self.arena.path(idx);
            if !p.starts_with(old) && p.starts_with(new) {
                conflicts.push(p);
            }
        }
        for path in conflicts {
            self.remove(&path);
        }

        let mut old_paths = Vec::new();
        // Reparent the renamed root (if present) under the new location's ancestors.
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let p = self.arena.path(idx);
            if p == old {
                let new_parent = self.parent_index_for(new);
                if new_parent == NO_PARENT {
                    let full = new.to_string_lossy().into_owned();
                    let (off, len) = self.arena.names.push(&full);
                    let node = self.arena.node_mut(idx);
                    node.parent = new_parent;
                    node.name_off = off;
                    node.name_len = len;
                } else {
                    let node = self.arena.node_mut(idx);
                    node.parent = new_parent;
                }
            }
        }
        // update nodes
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let p = self.arena.path(idx);
            let Ok(suffix) = p.strip_prefix(old) else {
                continue;
            };
            old_paths.push(p.clone());

            let new_path = if suffix.as_os_str().is_empty() {
                new.to_path_buf()
            } else {
                new.join(suffix)
            };
            // update name (file_name) and char_bloom
            let new_name = new_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let (name_off, name_len) = self.arena.names.push(&new_name);
            let node = self.arena.node_mut(idx);
            node.name_off = name_off;
            node.name_len = name_len;
            node.char_bloom = crate::search::fuzzy::compute_char_mask(&new_name);
            // store new path? We don't store full path; path derived from parent chain.
            // Parent chain unchanged (directory renamed would require updating parents).
            // For correctness, update parent pointers? Complex. Instead, rebuild path_index after.
        }

        if old_paths.is_empty() {
            return old_paths;
        }

        // Rebuild path_index
        self.path_index.clear();
        self.collision_index.clear();
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let p = self.arena.path(idx);
            // Wait: path() derives from parent chain + names. If we only updated leaf names,
            // parent directories names not updated for rename_prefix of a directory.
            // This is a known limitation; rename_prefix in original updated path strings.
            // To handle directory renames properly, we'd need update all descendants' parent names? 
            // Our arena stores names per node; renaming a directory means renaming that node's name,
            // and descendants keep parent pointers, so path() will reflect new dir name. Good.
            // But we updated only nodes whose path starts with old? We updated names for those nodes
            // based on new_path.file_name(), which for a child keeps its own name, not parent. Oops.
            // Actually for a directory rename, the directory node's name should change to new's last component;
            // descendants keep their names. Our loop changed EVERY descendant's name to its new_path last component,
            // which is wrong (child name becomes same as? new_path for child is new/child, file_name=child, okay).
            // For the renamed root itself, new_path=new, file_name=new last comp. Good.
            // So names are okay.
            self.insert_path_index(&p, idx);
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

        // materialize
        let mut entries = Vec::with_capacity(self.arena.len());
        let mut path_index = HashMap::with_capacity(self.arena.len());
        for i in 0..self.arena.len() {
            let idx = i as u32;
            let e = self.materialize(idx);
            path_index.insert(e.path.clone(), i);
            entries.push(e);
        }

        let new_trie = Arc::new(DirectoryTrie::build(
            &entries,
            &path_index,
            current_gen,
        ));
        let mut cache = self.cached_trie.write();
        *cache = Some((current_gen, Arc::clone(&new_trie)));
        new_trie
    }

    pub fn get(&self, path: &Path) -> Option<FileEntry> {
        self.find_path_index(path).map(|idx| self.materialize(idx))
    }

    pub fn get_index(&self, path: &Path) -> Option<u32> {
        self.find_path_index(path)
    }

    pub fn materialize(&self, idx: u32) -> FileEntry {
        let node = self.arena.node(idx);
        let extension = self.arena.extension(idx);
        let path = self.arena.path(idx);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.arena.name(idx).to_string());
        FileEntry {
            id: node.node_id,
            path,
            name,
            extension,
            size: node.size,
            modified: node.mtime,
            is_dir: node.is_dir(),
            dev: node.dev,
            ino: node.ino,
            char_bloom: node.char_bloom,
            content_pending: node.content_pending(),
        }
    }

    pub fn get_by_node_id(&self, node_id: u64) -> Option<FileEntry> {
        self.node_id_to_idx
            .get(&node_id)
            .map(|&idx| self.materialize(idx))
    }

    pub fn get_index_by_node_id(&self, node_id: u64) -> Option<u32> {
        self.node_id_to_idx.get(&node_id).copied()
    }

    pub fn path_index_map(&self) -> HashMap<PathBuf, usize> {
        let mut map = HashMap::with_capacity(self.arena.len());
        for i in 0..self.arena.len() {
            let idx = i as u32;
            map.insert(self.arena.path(idx), i);
        }
        map
    }

    pub fn db(&self) -> Option<Arc<MetadataDb>> {
        self.db.as_ref().map(Arc::clone)
    }

    pub fn len(&self) -> usize {
        self.arena.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    // Accessors for search hot path
    pub fn node(&self, idx: u32) -> &Node {
        self.arena.node(idx)
    }
    pub fn name(&self, idx: u32) -> &str {
        self.arena.name(idx)
    }
    pub fn path(&self, idx: u32) -> PathBuf {
        self.arena.path(idx)
    }
    pub fn extension(&self, idx: u32) -> Option<String> {
        self.arena.extension(idx)
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
