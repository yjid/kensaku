pub mod content;
pub mod metadata;
pub mod metadata_db;
pub mod walker;
pub mod watcher;

use crate::config::KensakuConfig;
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
}

pub struct IndexerManager {
    pub metadata: Arc<RwLock<MetadataStore>>,
    pub content_index: Arc<RwLock<Option<ContentIndex>>>,
    pub config: Arc<RwLock<KensakuConfig>>,
    pub status: Arc<RwLock<IndexingStatus>>,
    is_running: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<WatcherHandle>>>,
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

        Self {
            metadata: Arc::new(RwLock::new(MetadataStore::new())),
            content_index: Arc::new(RwLock::new(content_index)),
            config: Arc::new(RwLock::new(config)),
            status: Arc::new(RwLock::new(IndexingStatus {
                is_indexing: false,
                files_indexed: 0,
                content_indexed: 0,
                current_status: "Idle".to_string(),
            })),
            is_running: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
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
        let config_arc = Arc::clone(&self.config);
        let status = Arc::clone(&self.status);
        let is_running = Arc::clone(&self.is_running);
        let watcher_handle = Arc::clone(&self.watcher_handle);

        thread::spawn(move || {
            let (watched_dirs, excluded_dirs) = {
                let cfg = config_arc.read();
                (cfg.watched_dirs.clone(), cfg.excluded_dirs.clone())
            };

            {
                let mut st = status.write();
                st.is_indexing = true;
                st.current_status = "Scanning files...".to_string();
            }

            println!("[Kensaku Indexer] Starting directory scan across {:?}", watched_dirs);

            let mut files_count = 0;
            let mut content_count = 0;
            let mut pending_commits = 0;
            let mut crawled_paths: HashSet<PathBuf> = HashSet::new();

            walk_directories(&watched_dirs, &excluded_dirs, |entry| {
                files_count += 1;
                crawled_paths.insert(entry.path.clone());

                // Incremental crawl: skip entries whose size and mtime are unchanged.
                let unchanged = metadata
                    .read()
                    .get(&entry.path)
                    .map(|existing| {
                        existing.modified == entry.modified && existing.size == entry.size
                    })
                    .unwrap_or(false);

                if unchanged {
                    return true;
                }

                // Add to metadata
                metadata.write().add(entry.clone());

                // Content indexing for text files
                if !entry.is_dir && (is_indexable_extension(entry.extension.as_deref()) || is_indexable_file_name(&entry.name)) {
                    if let Some(content) = read_text_content(&entry.path, 100_000) {
                        if let Some(c_index) = content_index.read().as_ref() {
                            let _ = c_index.add_or_update(&entry.path, &entry.name, &content, entry.modified);
                            content_count += 1;
                            pending_commits += 1;
                        }
                    }
                }

                // Batch commits every 1000 content items
                if pending_commits >= 1000 {
                    if let Some(c_index) = content_index.read().as_ref() {
                        let _ = c_index.commit();
                    }
                    pending_commits = 0;
                }

                if files_count % 500 == 0 {
                    let mut st = status.write();
                    st.files_indexed = files_count;
                    st.content_indexed = content_count;
                    st.current_status = format!("Indexed {} items", files_count);
                }

                true
            });

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
            }

            println!("[Kensaku Indexer] Finished initial index: {} files ({} with full text)", files_count, content_count);

            // Start file watcher for live changes, replacing any previous one.
            let handle = start_file_watcher(watched_dirs, metadata, content_index, config_arc);
            *watcher_handle.lock() = Some(handle);

            is_running.store(false, Ordering::SeqCst);
        });
    }

    pub fn get_status(&self) -> IndexingStatus {
        self.status.read().clone()
    }
}
