pub mod content;
pub mod extract;
pub mod metadata;
pub mod metadata_db;
pub mod walker;
pub mod watcher;

use crate::config::KensakuConfig;
use crate::embed::{EmbeddingQueue, ModelRegistry, UnitStore, EMBEDDING_DIM};
use crate::indexer::content::{is_indexable_extension, is_indexable_file_name, read_text_content, ContentIndex};
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::walk_directories;
use crate::indexer::watcher::{start_file_watcher, WatcherHandle};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatus {
    pub is_indexing: bool,
    pub files_indexed: usize,
    pub content_indexed: usize,
    pub current_status: String,
    #[serde(default)]
    pub is_embedding: bool,
    #[serde(default)]
    pub units_indexed: usize,
    #[serde(default)]
    pub semantic_ready: bool,
}

pub struct IndexerManager {
    pub metadata: Arc<RwLock<MetadataStore>>,
    pub content_index: Arc<RwLock<Option<ContentIndex>>>,
    pub unit_store: Arc<RwLock<UnitStore>>,
    pub embedding_queue: Arc<Mutex<Option<EmbeddingQueue>>>,
    pub query_in_flight: Arc<AtomicBool>,
    pub config: Arc<RwLock<KensakuConfig>>,
    pub status: Arc<RwLock<IndexingStatus>>,
    is_running: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<WatcherHandle>>>,
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl IndexerManager {
    pub fn new(config: KensakuConfig) -> Self {
        let index_dir = KensakuConfig::index_dir();
        let content_index = match ContentIndex::new(&index_dir) {
            Ok(idx) => Some(idx),
            Err(e) => {
                eprintln!("[Kensaku] Failed to create Tantivy index: {}", e);
                None
            }
        };

        let metadata_store = Arc::new(RwLock::new(MetadataStore::new()));
        let unit_store = Arc::new(RwLock::new(UnitStore::new(EMBEDDING_DIM)));
        let query_in_flight = Arc::new(AtomicBool::new(false));

        // Load persisted units from database
        let mut units_count = 0;
        if let Some(db) = metadata_store.read().db() {
            match db.load_all_units() {
                Ok(units_with_vecs) => {
                    units_count = units_with_vecs.len();
                    let mut store = unit_store.write();
                    for (unit, vec) in units_with_vecs {
                        store.add_unit(unit, &vec);
                    }
                    if units_count > 0 {
                        println!("[Kensaku] Loaded {} semantic units from database", units_count);
                    }
                }
                Err(e) => {
                    eprintln!("[Kensaku] Failed to load semantic units: {}", e);
                }
            }
        }

        Self {
            metadata: metadata_store,
            content_index: Arc::new(RwLock::new(content_index)),
            unit_store,
            embedding_queue: Arc::new(Mutex::new(None)),
            query_in_flight,
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(IndexingStatus {
                is_indexing: false,
                files_indexed: 0,
                content_indexed: 0,
                current_status: "Idle".to_string(),
                is_embedding: false,
                units_indexed: units_count,
                semantic_ready: units_count > 0,
            })),
            is_running: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.write() = Some(handle);
    }

    fn emit_status(app_handle: &Arc<RwLock<Option<tauri::AppHandle>>>, status: &IndexingStatus) {
        if let Some(handle) = app_handle.read().as_ref() {
            use tauri::Emitter;
            let _ = handle.emit("indexing-status", status);
        }
    }

    pub fn start_indexing(&self) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            // Already running
            return;
        }

        // Stop any watcher left over from a previous indexing run so we never
        // end up with duplicate watchers on the same directories.
        let previous_watcher = self.watcher_handle.lock().take();
        if let Some(mut handle) = previous_watcher {
            handle.stop();
        }

        let metadata = Arc::clone(&self.metadata);
        let content_index = Arc::clone(&self.content_index);
        let unit_store = Arc::clone(&self.unit_store);
        let embedding_queue = Arc::clone(&self.embedding_queue);
        let query_in_flight = Arc::clone(&self.query_in_flight);
        let config_arc = Arc::clone(&self.config);
        let status = Arc::clone(&self.status);
        let is_running = Arc::clone(&self.is_running);
        let watcher_handle = Arc::clone(&self.watcher_handle);
        let app_handle = Arc::clone(&self.app_handle);

        thread::spawn(move || {
            let (watched_dirs, excluded_dirs, excluded_exts) = {
                let cfg = config_arc.read();
                (cfg.watched_dirs.clone(), cfg.excluded_dirs.clone(), cfg.excluded_extensions.clone())
            };

            {
                let mut st = status.write();
                st.is_indexing = true;
                st.current_status = "Scanning files...".to_string();
                Self::emit_status(&app_handle, &st);
            }

            println!("[Kensaku Indexer] Starting directory scan across {:?}", watched_dirs);

            let mut files_count = 0;
            let mut content_count = 0;
            let mut pending_commits = 0;
            let mut crawled_paths: HashSet<PathBuf> = HashSet::new();

            // Begin SQLite transaction for batched writes (100x faster than per-file commits)
            if let Some(db) = metadata.read().db() {
                let _ = db.begin_transaction();
            }

            // Phase 1: Collect all entries (jwalk is already parallel internally).
            let all_entries = walk_directories(&watched_dirs, &excluded_dirs);

            // Phase 2a: Filter excluded extensions, check incremental, add metadata.
            // Collect indexable files for parallel content extraction.
            let mut to_extract: Vec<(PathBuf, String, u64)> = Vec::new();

            for entry in all_entries {
                files_count += 1;
                crawled_paths.insert(entry.path.clone());

                // Skip files whose extension is in the excluded list.
                if !entry.is_dir
                    && let Some(ref ext) = entry.extension
                    && excluded_exts.iter().any(|e| e.eq_ignore_ascii_case(ext))
                {
                    continue;
                }

                // Incremental crawl: check + add under a single write lock.
                {
                    let mut store = metadata.write();
                    let unchanged = store
                        .get(&entry.path)
                        .map(|existing| {
                            existing.modified == entry.modified && existing.size == entry.size
                        })
                        .unwrap_or(false);
                    if unchanged {
                        continue;
                    }
                    store.add(entry.clone());
                }

                // Queue indexable files for parallel content extraction
                if !entry.is_dir
                    && (is_indexable_extension(entry.extension.as_deref()) || is_indexable_file_name(&entry.name))
                {
                    to_extract.push((entry.path.clone(), entry.name.clone(), entry.modified));
                }

                if files_count % 500 == 0 {
                    let mut st = status.write();
                    st.files_indexed = files_count;
                    st.content_indexed = content_count;
                    st.current_status = format!("Indexed {} items", files_count);
                    Self::emit_status(&app_handle, &st);
                }
            }

            // Phase 2b: Parallel content extraction with rayon
            let extracted: Vec<(PathBuf, String, String, u64)> = {
                use rayon::prelude::*;
                to_extract
                    .par_iter()
                    .filter_map(|(path, name, modified)| {
                        read_text_content(path, 1_048_576).map(|content| {
                            (path.clone(), name.clone(), content, *modified)
                        })
                    })
                    .collect()
            };

            // Phase 2c: Sequential Tantivy indexing (single writer)
            for (path, name, content, modified) in extracted {
                if let Some(c_index) = content_index.read().as_ref() {
                    let _ = c_index.add_or_update(&path, &name, &content, modified);
                    content_count += 1;
                    pending_commits += 1;
                }

                if pending_commits >= 1000 {
                    if let Some(c_index) = content_index.read().as_ref() {
                        let _ = c_index.commit();
                    }
                    pending_commits = 0;
                }
            }

            // Commit the SQLite transaction (batched writes)
            if let Some(db) = metadata.read().db() {
                let _ = db.commit_transaction();
            }

            // Drop metadata entries for files that no longer exist on disk.
            let stale_paths: Vec<PathBuf> = {
                let store = metadata.read();
                store
                    .get_entries()
                    .iter()
                    .filter(|entry| !crawled_paths.contains(&entry.path))
                    .map(|entry| entry.path.clone())
                    .collect()
            };

            if !stale_paths.is_empty() {
                let mut store = metadata.write();
                for path in &stale_paths {
                    store.remove(path);
                }
                println!(
                    "[Kensaku Indexer] Removed {} stale metadata entries",
                    stale_paths.len()
                );
            }

            // Purge content-index documents that no longer have a metadata
            // entry: files deleted while the app was closed, or trees that
            // belong to watched directories removed from the configuration.
            let valid_paths: HashSet<PathBuf> = {
                let store = metadata.read();
                store
                    .get_entries()
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect()
            };

            if let Some(c_index) = content_index.read().as_ref() {
                match c_index.purge_stale(&valid_paths) {
                    Ok(0) => {}
                    Ok(purged) => {
                        println!("[Kensaku Indexer] Purged {} stale content documents", purged);
                    }
                    Err(e) => {
                        eprintln!(
                            "[Kensaku Indexer] Failed to purge stale content documents: {}",
                            e
                        );
                    }
                }
            }

            // Final commit
            if let Some(c_index) = content_index.read().as_ref() {
                let _ = c_index.commit();
            }

            {
                let mut st = status.write();
                st.is_indexing = false;
                st.files_indexed = files_count;
                st.content_indexed = content_count;
                st.current_status = format!("Ready ({} files indexed)", files_count);
                Self::emit_status(&app_handle, &st);
            }

            println!("[Kensaku Indexer] Finished initial index: {} files ({} with full text)", files_count, content_count);

            // Start file watcher for live changes, replacing any previous one.
            let handle = start_file_watcher(watched_dirs, metadata.clone(), content_index, config_arc.clone());
            *watcher_handle.lock() = Some(handle);

            // Start background embedding worker if semantic search is enabled
            if config_arc.read().semantic_enabled {
                let mut eq_guard = embedding_queue.lock();
                if eq_guard.is_none() {
                    let model = ModelRegistry::default();
                    let mut q = EmbeddingQueue::new(
                        model,
                        Arc::clone(&unit_store),
                        Some(Arc::clone(&status)),
                        Some(Arc::clone(&app_handle)),
                    );
                    q.query_in_flight = Arc::clone(&query_in_flight);
                    if let Some(db) = metadata.read().db() {
                        q.start(db);
                    }
                    *eq_guard = Some(q);
                } else if let Some(ref mut q) = *eq_guard
                    && let Some(db) = metadata.read().db()
                {
                    q.start(db);
                }
            }

            is_running.store(false, Ordering::SeqCst);
        });
    }

    pub fn get_status(&self) -> IndexingStatus {
        self.status.read().clone()
    }
}
