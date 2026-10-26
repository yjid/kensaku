pub mod content;
pub mod extract;
pub mod metadata;
pub mod metadata_db;
pub mod walker;
pub mod watcher;

use crate::config::KensakuConfig;
use crate::embed::{EMBEDDING_DIM, EmbeddingQueue, ModelRegistry, UnitStore};
use crate::indexer::content::{
    ContentIndex, is_indexable_extension, is_indexable_file_name, read_text_content,
};
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::{FileEntry, is_cloud_placeholder_path, walk_directories};
use crate::indexer::watcher::{WatcherHandle, start_file_watcher};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
    /// Coarse pipeline phase: "idle", "walking", "extracting", "embedding", "committing".
    #[serde(default = "default_phase")]
    pub phase: String,
    /// Total entries discovered by the current crawl (0 until the walk ends).
    #[serde(default)]
    pub total_files: usize,
    /// Files whose text extraction has finished in the current crawl.
    #[serde(default)]
    pub extract_done: usize,
    /// Files queued for text extraction in the current crawl.
    #[serde(default)]
    pub extract_total: usize,
}

fn default_phase() -> String {
    "idle".to_string()
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn now_unix_secs() -> u64 {
    now_unix_ms() / 1000
}

/// True when an existing entry is unchanged and has no content re-read pending.
///
/// A pending entry (set by an OCR toggle) is always re-processed even when its
/// mtime and size still match, which is how the image re-scan stays targeted.
fn is_unchanged(existing: &FileEntry, entry: &FileEntry) -> bool {
    existing.modified == entry.modified && existing.size == entry.size && !existing.content_pending
}

pub struct IndexerManager {
    pub metadata: Arc<RwLock<MetadataStore>>,
    pub content_index: Arc<RwLock<Option<ContentIndex>>>,
    pub unit_store: Arc<RwLock<UnitStore>>,
    pub embedding_queue: Arc<Mutex<Option<EmbeddingQueue>>>,
    pub query_in_flight: Arc<AtomicBool>,
    pub config: Arc<RwLock<KensakuConfig>>,
    pub status: Arc<RwLock<IndexingStatus>>,
    /// Unix timestamp (ms) when the current/last crawl started, 0 if none.
    pub crawl_start_unix_ms: Arc<AtomicU64>,
    /// Unix timestamp (secs) of the last successful reconcile, 0 if never.
    pub last_reconcile_unix_secs: Arc<AtomicU64>,
    /// True while a live filesystem watcher is running.
    pub watcher_ok: Arc<AtomicBool>,
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

        // Discard semantic units produced by a different embedding model; the
        // embedding worker re-queues every node once it starts.
        if let Some(db) = metadata_store.read().db() {
            let expected_model_id = ModelRegistry::global().model_id;
            match db.units_model_id() {
                Ok(Some(existing_model_id)) if existing_model_id != expected_model_id => {
                    match db.reset_semantic_index() {
                        Ok(()) => {
                            unit_store.write().clear();
                            log::info!(
                                "[Kensaku] Semantic model changed (units id {} -> {}); reset semantic index",
                                existing_model_id,
                                expected_model_id
                            );
                        }
                        Err(e) => {
                            eprintln!("[Kensaku] Failed to reset semantic index: {}", e);
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[Kensaku] Failed to read semantic model id: {}", e);
                }
            }
        }

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
                        println!(
                            "[Kensaku] Loaded {} semantic units from database",
                            units_count
                        );
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
                phase: "idle".to_string(),
                total_files: 0,
                extract_done: 0,
                extract_total: 0,
            })),
            crawl_start_unix_ms: Arc::new(AtomicU64::new(0)),
            last_reconcile_unix_secs: Arc::new(AtomicU64::new(0)),
            watcher_ok: Arc::new(AtomicBool::new(false)),
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
        self.watcher_ok.store(false, Ordering::SeqCst);

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
        let crawl_start_unix_ms = Arc::clone(&self.crawl_start_unix_ms);
        let last_reconcile_unix_secs = Arc::clone(&self.last_reconcile_unix_secs);
        let watcher_ok = Arc::clone(&self.watcher_ok);

        thread::spawn(move || {
            let (watched_dirs, excluded_dirs, excluded_exts) = {
                let cfg = config_arc.read();
                (
                    cfg.watched_dirs.clone(),
                    cfg.excluded_dirs.clone(),
                    cfg.excluded_extensions.clone(),
                )
            };

            crawl_start_unix_ms.store(now_unix_ms(), Ordering::Relaxed);
            watcher_ok.store(false, Ordering::Relaxed);

            {
                let mut st = status.write();
                st.is_indexing = true;
                st.phase = "walking".to_string();
                st.current_status = "Scanning files...".to_string();
                Self::emit_status(&app_handle, &st);
            }

            println!(
                "[Kensaku Indexer] Starting directory scan across {:?}",
                watched_dirs
            );

            let mut files_count = 0;
            let mut content_count = 0;
            let mut pending_commits = 0;
            let mut crawled_paths: HashSet<PathBuf> = HashSet::new();
            // Images flagged by an OCR-toggle re-scan that were processed by
            // this crawl. Their pending flag is cleared once extraction ran.
            let mut pending_images: HashSet<PathBuf> = HashSet::new();

            // Begin SQLite transaction for batched writes (100x faster than per-file commits)
            if let Some(db) = metadata.read().db() {
                let _ = db.begin_transaction();
            }

            // Phase 1: Collect all entries (jwalk is already parallel internally).
            let all_entries = walk_directories(&watched_dirs, &excluded_dirs);
            let total_entries = all_entries.len();
            {
                let mut st = status.write();
                st.total_files = total_entries;
                st.phase = "walking".to_string();
                st.current_status = format!("Indexing metadata for {} entries...", total_entries);
                Self::emit_status(&app_handle, &st);
            }

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

                // Incremental crawl: check + add under a single write lock. A
                // pending entry is re-processed even when mtime/size match so
                // an OCR toggle only re-scans images.
                {
                    let mut store = metadata.write();
                    let (unchanged, was_pending) = match store.get(&entry.path) {
                        Some(existing) => {
                            (is_unchanged(existing, &entry), existing.content_pending)
                        }
                        None => (false, false),
                    };
                    if unchanged {
                        continue;
                    }
                    store.add(entry.clone());
                    if was_pending {
                        pending_images.insert(entry.path.clone());
                    }
                }

                // Queue indexable files for parallel content extraction.
                // Cloud placeholders stay metadata-only so indexing does not
                // hydrate (download) them.
                if !entry.is_dir
                    && (is_indexable_extension(entry.extension.as_deref())
                        || is_indexable_file_name(&entry.name))
                    && !is_cloud_placeholder_path(&entry.path)
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
            let extract_total = to_extract.len();
            {
                let mut st = status.write();
                st.phase = "extracting".to_string();
                st.files_indexed = files_count;
                st.content_indexed = content_count;
                st.total_files = total_entries;
                st.extract_total = extract_total;
                st.extract_done = 0;
                st.current_status = format!("Extracting text from {} potential files...", extract_total);
                Self::emit_status(&app_handle, &st);
            }

            let extract_done = AtomicUsize::new(0);
            let extracted: Vec<(PathBuf, String, String, u64)> = {
                use rayon::prelude::*;
                // Image metadata is best-effort: a corrupt or unsupported image
                // must not prevent text extraction (OCR) from being attempted.
                let image_db = metadata.read().db();
                to_extract
                    .par_iter()
                    .filter_map(|(path, name, modified)| {
                        let image_extension =
                            path.extension().and_then(|ext| ext.to_str()).filter(|ext| {
                                crate::indexer::extract::image::is_supported_image_extension(ext)
                            });
                        if image_extension.is_some()
                            && let (Some((width, height)), Some(phash)) = (
                                crate::indexer::extract::image::dimensions(path),
                                crate::indexer::extract::image::phash64(path),
                            )
                            && let Some(db) = image_db.as_ref()
                        {
                            let _ = db.set_image_meta(path, width, height, phash);
                        }

                        let content =
                            read_text_content(path, 1_048_576)
                                .map(|content| (path.clone(), name.clone(), content, *modified));

                        let done = extract_done.fetch_add(1, Ordering::Relaxed) + 1;
                        if done % 25 == 0 || done == extract_total {
                            let mut st = status.write();
                            st.extract_done = done;
                            st.current_status =
                                format!("Extracting text… {}/{} files", done, extract_total);
                            Self::emit_status(&app_handle, &st);
                        }

                        content
                    })
                    .collect()
            };
            {
                let mut st = status.write();
                st.extract_done = extract_total;
                Self::emit_status(&app_handle, &st);
            }

            // Phase 2c: Sequential Tantivy indexing (single writer)
            {
                let mut st = status.write();
                st.phase = "committing".to_string();
                st.files_indexed = files_count;
                st.content_indexed = content_count;
                st.current_status = "Updating search index...".to_string();
                Self::emit_status(&app_handle, &st);
            }

            let mut extracted_paths: HashSet<PathBuf> = HashSet::new();
            for (path, name, content, modified) in extracted {
                if let Some(c_index) = content_index.read().as_ref() {
                    let _ = c_index.add_or_update(&path, &name, &content, modified);
                    content_count += 1;
                    pending_commits += 1;
                }

                if pending_images.contains(&path) {
                    extracted_paths.insert(path);
                }

                if pending_commits >= 1000 {
                    if let Some(c_index) = content_index.read().as_ref() {
                        let _ = c_index.commit();
                    }
                    pending_commits = 0;
                }
            }

            // A pending image re-scanned by this crawl is done. When extraction
            // produced nothing (OCR disabled, unreadable, or no text), drop any
            // stale OCR document so search stops returning it. Paths skipped by
            // exclusions never enter `pending_images`, so their flag is kept.
            for path in &pending_images {
                if !extracted_paths.contains(path)
                    && let Some(c_index) = content_index.read().as_ref()
                {
                    let _ = c_index.remove(path);
                }
                metadata.write().clear_content_pending(path);
            }

            // Commit the SQLite transaction (batched writes)
            if let Some(db) = metadata.read().db() {
                let _ = db.commit_transaction();
                // Keep the WAL from growing across crawls. PASSIVE never
                // blocks readers/writers, so this is safe on the crawl thread.
                db.checkpoint_passive();
            }

            // A configured root that cannot be read (unplugged external drive,
            // unmounted network share) is not "everything was deleted": keep
            // its metadata and documents so reconnecting the drive does not
            // require a full rebuild.
            let unavailable_roots: Vec<PathBuf> = watched_dirs
                .iter()
                .filter(|root| std::fs::read_dir(root).is_err())
                .cloned()
                .collect();
            let belongs_to_unavailable_root = |path: &std::path::Path| {
                unavailable_roots.iter().any(|root| path.starts_with(root))
            };

            if !unavailable_roots.is_empty() {
                let hint = format!(
                    "{} roots unavailable, index preserved",
                    unavailable_roots.len()
                );
                println!("[Kensaku Indexer] {}: {:?}", hint, unavailable_roots);
                let mut st = status.write();
                st.current_status = hint;
                Self::emit_status(&app_handle, &st);
            }

            // Drop metadata entries for files that no longer exist on disk.
            let stale_paths: Vec<PathBuf> = {
                let store = metadata.read();
                store
                    .get_entries()
                    .iter()
                    .filter(|entry| {
                        !crawled_paths.contains(&entry.path)
                            && !belongs_to_unavailable_root(&entry.path)
                    })
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
                match c_index.purge_stale(&valid_paths, &unavailable_roots) {
                    Ok(0) => {}
                    Ok(purged) => {
                        println!(
                            "[Kensaku Indexer] Purged {} stale content documents",
                            purged
                        );
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

            last_reconcile_unix_secs.store(now_unix_secs(), Ordering::Relaxed);

            // First-scan completion gate: the first full crawl marks setup done.
            {
                let mut cfg = config_arc.write();
                if cfg.is_setup_complete && !cfg.first_scan_done {
                    cfg.first_scan_done = true;
                    let _ = cfg.save();
                }
            }

            {
                let mut st = status.write();
                st.is_indexing = false;
                st.phase = "idle".to_string();
                st.files_indexed = files_count;
                st.content_indexed = content_count;
                st.current_status = format!("Ready ({} files indexed)", files_count);
                Self::emit_status(&app_handle, &st);
            }

            println!(
                "[Kensaku Indexer] Finished initial index: {} files ({} with full text)",
                files_count, content_count
            );

            // Start file watcher for live changes, replacing any previous one.
            let handle = start_file_watcher(
                watched_dirs,
                metadata.clone(),
                content_index,
                config_arc.clone(),
                Arc::clone(&watcher_ok),
            );
            *watcher_handle.lock() = Some(handle);

            // Start background embedding worker if semantic search is enabled
            if config_arc.read().semantic_enabled {
                let mut eq_guard = embedding_queue.lock();
                if eq_guard.is_none() {
                    let model = ModelRegistry::global().clone();
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

    /// True while a live filesystem watcher is running.
    pub fn watcher_ok(&self) -> bool {
        self.watcher_ok.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(modified: u64, size: u64, content_pending: bool) -> FileEntry {
        FileEntry {
            id: 0,
            path: PathBuf::from("/tmp/kensaku/example.bin"),
            name: "example.bin".to_string(),
            extension: Some("bin".to_string()),
            size,
            modified,
            is_dir: false,
            dev: 0,
            ino: 0,
            char_bloom: 0,
            content_pending,
        }
    }

    #[test]
    fn change_detection_honors_pending_content() {
        let existing = entry(100, 10, false);
        assert!(is_unchanged(&existing, &entry(100, 10, false)));
        assert!(!is_unchanged(&existing, &entry(101, 10, false)));
        assert!(!is_unchanged(&existing, &entry(100, 11, false)));

        let pending = entry(100, 10, true);
        assert!(
            !is_unchanged(&pending, &entry(100, 10, false)),
            "a pending entry must be re-processed even when mtime/size match"
        );
    }
}
