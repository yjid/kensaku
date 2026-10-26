pub mod arena;
pub mod content;
pub mod extract;
pub mod metadata;
pub mod metadata_db;
pub mod walker;
pub mod watcher;

use crate::config::KensakuConfig;
use crate::embed::units::chunk_text;
use crate::embed::{EMBEDDING_DIM, EmbeddingQueue, ModelRegistry, Unit, UnitStore};
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

/// One file scheduled for text extraction in the current crawl.
struct ExtractJob {
    node_id: u64,
    path: PathBuf,
    name: String,
    modified: u64,
    size: u64,
    is_image: bool,
}

/// A file that has been extracted, together with any semantic vectors.
///
/// Passed from the extraction workers to the single index-committing thread
/// through a bounded channel. The bound is what keeps peak memory flat: text is
/// dropped as soon as it has been written to the lexical and semantic indexes
/// instead of every document's text accumulating in a `Vec` until the end.
struct ExtractedWork {
    node_id: u64,
    path: PathBuf,
    name: String,
    modified: u64,
    content: Option<String>,
    units: Vec<(usize, usize, Vec<i8>)>,
    content_hash: Option<String>,
}

fn fnv_hash(data: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// True while a perceptual-hash backfill thread is running.
static PHASH_BACKFILL_RUNNING: AtomicBool = AtomicBool::new(false);

/// Computes perceptual hashes for images the crawl deliberately skipped.
///
/// Full-image decoding is the slowest and most memory-hungry part of image
/// handling, and the hash is only used by the `/dupes` view — it must never
/// gate OCR or the search index. This worker decodes one image at a time,
/// pauses during crawls and while a query is in flight, and lets hashes appear
/// progressively in duplicate grouping.
fn start_phash_backfill(
    metadata: Arc<RwLock<MetadataStore>>,
    crawl_running: Arc<AtomicBool>,
    query_in_flight: Arc<AtomicBool>,
) {
    if PHASH_BACKFILL_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        let thumbs_dir = KensakuConfig::config_dir().join("thumbs");
        struct ResetOnExit;
        impl Drop for ResetOnExit {
            fn drop(&mut self) {
                PHASH_BACKFILL_RUNNING.store(false, Ordering::SeqCst);
            }
        }
        let _reset = ResetOnExit;

        let Some(db) = metadata.read().db() else {
            return;
        };

        loop {
            // Never compete with a crawl or an interactive query.
            while crawl_running.load(Ordering::Relaxed) {
                thread::sleep(std::time::Duration::from_millis(500));
            }
            while query_in_flight.load(Ordering::Relaxed) {
                thread::sleep(std::time::Duration::from_millis(100));
            }

            let paths = match db.image_paths_missing_phash(16) {
                Ok(paths) => paths,
                Err(_) => return,
            };
            if paths.is_empty() {
                return;
            }

            for path in paths {
                if crawl_running.load(Ordering::Relaxed) {
                    break;
                }
                let hash = crate::indexer::extract::image::phash64_cached(&path, &thumbs_dir)
                    .unwrap_or(u64::MAX);
                let _ = db.set_image_phash(&path, hash);
                crate::util::memory::release_memory();
                // One decode at a time, with a small yield between images so
                // typing stays responsive.
                thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    });
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

        // Semantic state (the ~30 MB embedding table and the persisted unit
        // vectors) is only loaded when semantic search is actually enabled.
        // With it off the indexer stays a plain lexical index and keeps that
        // memory free.
        let semantic_enabled = config.semantic_enabled;

        // Discard semantic units produced by a different embedding model; the
        // embedding worker re-queues every node once it starts.
        if semantic_enabled && let Some(db) = metadata_store.read().db() {
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

        // Load persisted units from database. These are required for semantic
        // search even if the user toggles the tier on later in this session, so
        // they are always restored; only the embedding table is lazy.
        let mut units_count = 0;
        if let Some(db) = metadata_store.read().db() {
            let mut store = unit_store.write();
            match db.load_units_into(&mut store) {
                Ok(count) => {
                    units_count = count;
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

        // The crawl embeds newly extracted text inline, so a background
        // embedding worker from a previous run must not also pick up the same
        // nodes (it would re-read every file and duplicate units). It is
        // stopped inside the crawl thread so the IPC call returns immediately,
        // and restarted at the end for incremental work.

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

            // Stop a background embedding worker from a previous run before the
            // crawl starts writing. This runs here (not on the IPC thread) so a
            // long embedding batch cannot block the caller. The crawl embeds
            // newly extracted text inline; the worker is restarted afterwards
            // for incremental work.
            if let Some(mut queue) = embedding_queue.lock().take() {
                queue.stop();
            }

            println!(
                "[Kensaku Indexer] Starting directory scan across {:?}",
                watched_dirs
            );

            let mut files_count: usize = 0;
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
            let mut to_extract: Vec<ExtractJob> = Vec::new();

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

                // Incremental crawl: a cheap read-lock check first, so an
                // unchanged tree never contends on the write lock or touches
                // SQLite. A pending entry is re-processed even when mtime/size
                // match so an OCR toggle only re-scans images.
                let unchanged = {
                    let store = metadata.read();
                    store
                        .get(&entry.path)
                        .is_some_and(|existing| is_unchanged(&existing, &entry))
                };
                if unchanged {
                    continue;
                }

                let (node_id, was_pending) = {
                    let mut store = metadata.write();
                    let was_pending = store
                        .get(&entry.path)
                        .is_some_and(|existing| existing.content_pending);
                    let id = store.add(entry.clone());
                    (id, was_pending)
                };
                if was_pending {
                    pending_images.insert(entry.path.clone());
                }

                // Queue indexable files for parallel content extraction.
                // Cloud placeholders stay metadata-only so indexing does not
                // hydrate (download) them.
                if !entry.is_dir
                    && (is_indexable_extension(entry.extension.as_deref())
                        || is_indexable_file_name(&entry.name))
                    && !is_cloud_placeholder_path(&entry.path)
                {
                    to_extract.push(ExtractJob {
                        node_id,
                        path: entry.path.clone(),
                        name: entry.name.clone(),
                        modified: entry.modified,
                        size: entry.size,
                        is_image: entry.extension.as_deref().is_some_and(
                            crate::indexer::extract::image::is_supported_image_extension,
                        ),
                    });
                }

                if files_count.is_multiple_of(500) {
                    let mut st = status.write();
                    st.files_indexed = files_count;
                    st.content_indexed = content_count;
                    st.current_status = format!("Indexed {} items", files_count);
                    Self::emit_status(&app_handle, &st);
                }
            }

            // Index text and small files first. Images (OCR) are the slow tail,
            // so this makes the lexical index useful earlier in the crawl while
            // the same total amount of work is done.
            to_extract.sort_unstable_by_key(|job| (job.is_image, job.size));

            // Phase 2b: streaming extraction → lexical index → semantic index.
            //
            // Previously every extracted document was collected into a Vec and
            // indexed afterwards, and the semantic worker then re-read and
            // re-extracted every file (re-running OCR). The pipeline below
            // extracts each file exactly once and hands it to both indexes
            // through a bounded channel, so peak memory stays flat and images
            // are OCR'd once instead of twice.
            let extract_total = to_extract.len();
            {
                let mut st = status.write();
                st.phase = "extracting".to_string();
                st.files_indexed = files_count;
                st.content_indexed = content_count;
                st.total_files = total_entries;
                st.extract_total = extract_total;
                st.extract_done = 0;
                st.current_status =
                    format!("Extracting text from {} potential files...", extract_total);
                Self::emit_status(&app_handle, &st);
            }

            let semantic_enabled = config_arc.read().semantic_enabled;
            let model = semantic_enabled.then(|| ModelRegistry::global().clone());
            let embed_model_id = model.as_ref().map(|m| m.model_id);

            // Drop semantic units for every file about to be re-extracted. New
            // files have none, so a first scan does no work here. One batched
            // pass (instead of per-file vector compaction) keeps re-indexing a
            // large tree linear.
            if semantic_enabled && !to_extract.is_empty() {
                let ids: HashSet<u64> = to_extract
                    .iter()
                    .map(|job| job.node_id)
                    .filter(|id| *id > 0)
                    .collect();
                if !ids.is_empty() {
                    let id_vec: Vec<u64> = ids.iter().copied().collect();
                    if let Some(db) = metadata.read().db() {
                        let _ = db.delete_units_for_nodes(&id_vec);
                    }
                    unit_store.write().remove_units_for_nodes(&ids);
                }
            }

            let extract_done = AtomicUsize::new(0);
            let mut extracted_paths: HashSet<PathBuf> = HashSet::new();
            let mut units_added = 0usize;

            // Bounded handoff: at most 32 extracted documents (and their
            // vectors) are in flight at any moment.
            let (tx, rx) = std::sync::mpsc::sync_channel::<ExtractedWork>(32);
            let image_db = metadata.read().db();
            let db_handle = metadata.read().db();
            // The producer task runs on the rayon pool and must own its handles;
            // the consumer (this thread) keeps using the originals.
            let producer_status = Arc::clone(&status);
            let producer_app = Arc::clone(&app_handle);

            rayon::scope(|scope| {
                use rayon::prelude::*;

                scope.spawn(move |_| {
                    to_extract.par_iter().for_each_with(tx, |tx, job| {
                        // Image dimensions come from the file header, which is
                        // cheap. Perceptual hashing needs a full decode; doing
                        // it here would serialize the whole crawl behind a
                        // handful of decode slots and starve OCR. The hash is
                        // filled in later by the background backfill.
                        if job.is_image
                            && let Some((width, height)) =
                                crate::indexer::extract::image::dimensions(&job.path)
                            && let Some(db) = image_db.as_ref()
                        {
                            let _ = db.set_image_dimensions(&job.path, width, height);
                        }

                        let content = read_text_content(&job.path, 1_048_576);

                        // Embed here, in parallel with extraction and lexical
                        // indexing. The vectors travel with the text so the
                        // semantic tier never has to touch the file again.
                        let (units, content_hash) = match (model.as_ref(), content.as_deref()) {
                            (Some(model), Some(text)) if !text.trim().is_empty() => {
                                let chunks = chunk_text(text, job.size as usize);
                                let vectors: Vec<(usize, usize, Vec<i8>)> = chunks
                                    .into_par_iter()
                                    .map(|(start, end, chunk)| {
                                        (start, end, model.embed_to_i8(&chunk))
                                    })
                                    .collect();
                                (vectors, Some(format!("{:x}", fnv_hash(text))))
                            }
                            _ => (Vec::new(), None),
                        };

                        let done = extract_done.fetch_add(1, Ordering::Relaxed) + 1;
                        if done.is_multiple_of(25) || done == extract_total {
                            let mut st = producer_status.write();
                            st.extract_done = done;
                            st.current_status =
                                format!("Extracting text… {}/{} files", done, extract_total);
                            Self::emit_status(&producer_app, &st);
                        }

                        // Consumer may be gone only if the crawl is shutting
                        // down; dropping the work is safe in that case.
                        let _ = tx.send(ExtractedWork {
                            node_id: job.node_id,
                            path: job.path.clone(),
                            name: job.name.clone(),
                            modified: job.modified,
                            content,
                            units,
                            content_hash,
                        });
                    });
                });

                // Single writer for both indexes (Tantivy requires one).
                let index_guard = content_index.read();
                let c_index = index_guard.as_ref();
                for work in rx {
                    if let Some(ref content) = work.content
                        && let Some(c_index) = c_index
                    {
                        let _ =
                            c_index.add_or_update(&work.path, &work.name, content, work.modified);
                        content_count += 1;
                        pending_commits += 1;

                        // Publish live counts instead of waiting for a commit
                        // boundary, otherwise the UI reports 0 documents while
                        // extraction is still running.
                        if content_count.is_multiple_of(64) {
                            let mut st = status.write();
                            st.files_indexed = files_count;
                            st.content_indexed = content_count;
                            Self::emit_status(&app_handle, &st);
                        }
                    }

                    if pending_images.contains(&work.path) && work.content.is_some() {
                        extracted_paths.insert(work.path.clone());
                    }

                    if let Some(model_id) = embed_model_id
                        && let Some(db) = db_handle.as_ref()
                    {
                        for (start, end, vector) in &work.units {
                            if let Ok(id) =
                                db.insert_unit(work.node_id, *start, *end, model_id, vector)
                            {
                                let unit = Unit {
                                    id,
                                    node_id: work.node_id,
                                    byte_start: *start,
                                    byte_end: *end,
                                    model_id,
                                };
                                unit_store.write().add_unit(unit, vector);
                                units_added += 1;
                                if units_added.is_multiple_of(256) {
                                    let mut st = status.write();
                                    st.units_indexed = unit_store.read().len();
                                    st.semantic_ready = st.units_indexed > 0;
                                    Self::emit_status(&app_handle, &st);
                                }
                            }
                        }
                        let _ = db.update_node_indexed_gen(
                            work.node_id,
                            1,
                            work.content_hash.as_deref(),
                        );
                    }

                    if pending_commits >= 512
                        && let Some(c_index) = c_index
                    {
                        let _ = c_index.commit();
                        pending_commits = 0;
                        let mut st = status.write();
                        st.content_indexed = content_count;
                        Self::emit_status(&app_handle, &st);
                    }
                }
            });

            {
                let mut st = status.write();
                st.extract_done = extract_total;
                st.content_indexed = content_count;
                if semantic_enabled {
                    st.units_indexed = unit_store.read().len();
                    st.semantic_ready = st.units_indexed > 0;
                }
                st.phase = "committing".to_string();
                st.current_status = "Updating search index...".to_string();
                Self::emit_status(&app_handle, &st);
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
                (0..store.len())
                    .filter_map(|i| {
                        let e = store.materialize(i as u32);
                        if !crawled_paths.contains(&e.path)
                            && !belongs_to_unavailable_root(&e.path)
                        {
                            Some(e.path)
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            if !stale_paths.is_empty() {
                // Drop content documents for the same paths directly. The old
                // implementation scanned the entire content index on every
                // crawl, which cost seconds on large corpora; the crawl already
                // knows exactly what disappeared.
                if let Some(c_index) = content_index.read().as_ref() {
                    let _ = c_index.remove_many(&stale_paths);
                }

                let mut store = metadata.write();
                for path in &stale_paths {
                    store.remove(path);
                }
                println!(
                    "[Kensaku Indexer] Removed {} stale metadata entries",
                    stale_paths.len()
                );
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

            // Fill in perceptual hashes that the crawl skipped, one image at a
            // time. Safe to start here: the crawl transaction is committed and
            // the worker pauses if another crawl begins.
            start_phash_backfill(
                Arc::clone(&metadata),
                Arc::clone(&is_running),
                Arc::clone(&query_in_flight),
            );
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
