use crate::embed::model::ModelRegistry;
use crate::embed::units::{chunk_text, Unit, UnitStore};
use crate::indexer::content::{is_indexable_extension, is_indexable_file_name, read_text_content};
use crate::indexer::metadata_db::MetadataDb;
use crate::indexer::IndexingStatus;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct EmbeddingQueue {
    pub model: ModelRegistry,
    pub unit_store: Arc<RwLock<UnitStore>>,
    pub is_running: Arc<AtomicBool>,
    pub query_in_flight: Arc<AtomicBool>,
    pub status: Option<Arc<RwLock<IndexingStatus>>>,
    pub app_handle: Option<Arc<RwLock<Option<tauri::AppHandle>>>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl EmbeddingQueue {
    pub fn new(
        model: ModelRegistry,
        unit_store: Arc<RwLock<UnitStore>>,
        status: Option<Arc<RwLock<IndexingStatus>>>,
        app_handle: Option<Arc<RwLock<Option<tauri::AppHandle>>>>,
    ) -> Self {
        Self {
            model,
            unit_store,
            is_running: Arc::new(AtomicBool::new(false)),
            query_in_flight: Arc::new(AtomicBool::new(false)),
            status,
            app_handle,
            worker_handle: None,
        }
    }

    fn emit_status(
        app_handle: &Option<Arc<RwLock<Option<tauri::AppHandle>>>>,
        status: &Option<Arc<RwLock<IndexingStatus>>>,
    ) {
        if let (Some(app_arc), Some(st_arc)) = (app_handle, status) {
            let st = st_arc.read().clone();
            if let Some(handle) = app_arc.read().as_ref() {
                use tauri::Emitter;
                let _ = handle.emit("indexing-status", &st);
            }
        }
    }

    /// Starts the background embedding worker.
    /// Uses a bounded 2-thread Rayon pool and respects `query_in_flight` preemption.
    pub fn start(&mut self, db: Arc<MetadataDb>) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let is_running = Arc::clone(&self.is_running);
        let query_in_flight = Arc::clone(&self.query_in_flight);
        let unit_store = Arc::clone(&self.unit_store);
        let status = self.status.clone();
        let app_handle = self.app_handle.clone();
        let model = self.model.clone();

        let handle = thread::Builder::new()
            .name("kensaku-embed-worker".to_string())
            .spawn(move || {
                // Rayon thread pool bounded to max 2 threads per ARCHITECTURE §5.3 / §10
                let embed_pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(2)
                    .thread_name(|i| format!("kensaku-embed-{}", i))
                    .build()
                    .ok();

                let mut hash_cache: HashMap<String, Vec<(usize, usize, Vec<i8>)>> = HashMap::new();
                let mut was_embedding = false;

                while is_running.load(Ordering::SeqCst) {
                    // Preempt if query in flight
                    if query_in_flight.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }

                    // TODO: Check battery state (e.g. AC power) and skip if on battery power

                    // Fetch unindexed nodes (indexed_gen = 0), priority: recent first, small files first
                    let pending_nodes = match db.get_nodes_for_embedding(16) {
                        Ok(nodes) => nodes,
                        Err(e) => {
                            eprintln!("[Kensaku Embedder] Failed to get nodes for embedding: {}", e);
                            thread::sleep(Duration::from_millis(1000));
                            continue;
                        }
                    };

                    if pending_nodes.is_empty() {
                        if was_embedding {
                            was_embedding = false;
                            if let Some(st_arc) = &status {
                                let mut st = st_arc.write();
                                st.is_embedding = false;
                                st.units_indexed = unit_store.read().len();
                                st.semantic_ready = st.units_indexed > 0;
                            }
                            Self::emit_status(&app_handle, &status);
                        }
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }

                    if !was_embedding {
                        was_embedding = true;
                        if let Some(st_arc) = &status {
                            let mut st = st_arc.write();
                            st.is_embedding = true;
                        }
                        Self::emit_status(&app_handle, &status);
                    }

                    for (node_id, path, size, _modified, _existing_hash) in pending_nodes {
                        if !is_running.load(Ordering::SeqCst) {
                            break;
                        }
                        // Check query preemption before every file
                        if query_in_flight.load(Ordering::SeqCst) {
                            thread::sleep(Duration::from_millis(50));
                            break;
                        }

                        let filename = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let ext = path.extension().and_then(|e| e.to_str());

                        if !is_indexable_extension(ext) && !is_indexable_file_name(&filename) {
                            let _ = db.update_node_indexed_gen(node_id, 1, None);
                            continue;
                        }

                        let content = match read_text_content(&path, 500_000) {
                            Some(c) => c,
                            None => {
                                let _ = db.update_node_indexed_gen(node_id, 1, None);
                                continue;
                            }
                        };

                        let content_hash = format!("{:x}", fnv_hash(&content));

                        // Deduplicate embedding if identical content hash already computed
                        let chunks_with_vectors = if let Some(cached) = hash_cache.get(&content_hash) {
                            cached.clone()
                        } else {
                            let chunks = chunk_text(&content, size as usize);
                            let model_ref = &model;
                            let computed: Vec<(usize, usize, Vec<i8>)> = if let Some(ref pool) = embed_pool {
                                pool.install(|| {
                                    chunks
                                        .into_par_iter()
                                        .map(|(start, end, chunk_str)| {
                                            let vec = model_ref.embed_to_i8(&chunk_str);
                                            (start, end, vec)
                                        })
                                        .collect()
                                })
                            } else {
                                chunks
                                    .into_iter()
                                    .map(|(start, end, chunk_str)| {
                                        let vec = model_ref.embed_to_i8(&chunk_str);
                                        (start, end, vec)
                                    })
                                    .collect()
                            };

                            if hash_cache.len() < 5000 {
                                hash_cache.insert(content_hash.clone(), computed.clone());
                            }
                            computed
                        };

                        let mut units_to_insert = Vec::new();
                        for (start, end, vec) in chunks_with_vectors {
                            let unit = Unit {
                                id: 0, // Assigned by SQLite autoincrement
                                node_id,
                                byte_start: start,
                                byte_end: end,
                                model_id: model.model_id,
                            };
                            units_to_insert.push((unit, vec));
                        }

                        // Store in SQLite and UnitStore
                        for (unit, vec) in units_to_insert {
                            if let Ok(inserted_id) = db.insert_unit(
                                node_id,
                                unit.byte_start,
                                unit.byte_end,
                                unit.model_id,
                                &vec,
                            ) {
                                let mut stored_unit = unit;
                                stored_unit.id = inserted_id;
                                unit_store.write().add_unit(stored_unit, &vec);
                            }
                        }

                        let _ = db.update_node_indexed_gen(node_id, 1, Some(&content_hash));
                    }

                    // Update indexing status progress
                    if let Some(st_arc) = &status {
                        let mut st = st_arc.write();
                        st.units_indexed = unit_store.read().len();
                        st.semantic_ready = st.units_indexed > 0;
                    }
                    Self::emit_status(&app_handle, &status);
                }
            })
            .ok();

        self.worker_handle = handle;
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

fn fnv_hash(data: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
