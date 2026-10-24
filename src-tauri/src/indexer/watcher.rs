use crate::config::KensakuConfig;
use crate::indexer::content::{is_indexable_extension, is_indexable_file_name, read_text_content, ContentIndex};
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::{file_identity, FileEntry};
use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

/// Handle to a running filesystem watcher thread.
///
/// Dropping the handle signals the thread to stop and waits for it to exit.
/// This keeps repeated `start_indexing` calls from leaving duplicate watchers
/// alive.
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl WatcherHandle {
    /// Signal the watcher to stop and wait for its thread to finish.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_file_watcher(
    watched_dirs: Vec<PathBuf>,
    metadata: Arc<RwLock<MetadataStore>>,
    content_index: Arc<RwLock<Option<ContentIndex>>>,
    config: Arc<RwLock<KensakuConfig>>,
) -> WatcherHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let thread = thread::spawn(move || {
        run_watcher(watched_dirs, metadata, content_index, config, stop_thread);
    });

    WatcherHandle {
        stop,
        thread: Some(thread),
    }
}

fn run_watcher(
    watched_dirs: Vec<PathBuf>,
    metadata: Arc<RwLock<MetadataStore>>,
    content_index: Arc<RwLock<Option<ContentIndex>>>,
    config: Arc<RwLock<KensakuConfig>>,
    stop: Arc<AtomicBool>,
) {
    let (tx, rx) = channel::<notify::Result<Event>>();

    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[Kensaku Watcher] Failed to initialize watcher: {}", e);
            return;
        }
    };

    for dir in &watched_dirs {
        if dir.exists() {
            if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                eprintln!("[Kensaku Watcher] Failed to watch {}: {}", dir.display(), e);
            } else {
                println!("[Kensaku Watcher] Watching {}", dir.display());
            }
        }
    }

    let mut pending_commits = 0;
    let mut last_commit = SystemTime::now();

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        // Read with timeout so we can periodically commit and honor stop.
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                let cfg = config.read().clone();
                pending_commits += handle_event(&event, &cfg, &metadata, &content_index);
            }
            Ok(Err(e)) => {
                eprintln!("[Kensaku Watcher] Error event: {:?}", e);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }

        // Periodic Tantivy commit if needed
        if pending_commits > 0
            && let Ok(elapsed) = last_commit.elapsed()
            && (elapsed >= Duration::from_secs(3) || pending_commits > 50)
        {
            if let Some(c_index) = content_index.read().as_ref() {
                let _ = c_index.commit();
            }
            pending_commits = 0;
            last_commit = SystemTime::now();
        }
    }

    // Flush anything still pending before the thread exits.
    if pending_commits > 0
        && let Some(c_index) = content_index.read().as_ref()
    {
        let _ = c_index.commit();
    }
}

fn is_path_excluded(path: &Path, cfg: &KensakuConfig) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|name| {
                // Hidden/dot entries are never indexed by default.
                // TODO: honor `cfg.skip_hidden` once it is wired through.
                cfg.skip_hidden && name.starts_with('.') || cfg.is_dir_excluded(name)
            })
            .unwrap_or(false)
    })
}

/// Applies a single watcher event, returning how many index mutations occurred.
fn handle_event(
    event: &Event,
    cfg: &KensakuConfig,
    metadata: &Arc<RwLock<MetadataStore>>,
    content_index: &Arc<RwLock<Option<ContentIndex>>>,
) -> usize {
    let mut changes = 0;

    match &event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            // `paths` is ordered (from, to) for this event kind.
            if event.paths.len() >= 2 {
                let old_path = &event.paths[0];
                let new_path = &event.paths[1];

                if !is_path_excluded(old_path, cfg) {
                    handle_path_remove(old_path, metadata, content_index);
                    changes += 1;
                }
                if !is_path_excluded(new_path, cfg) {
                    handle_file_upsert(new_path, metadata, content_index);
                    changes += 1;
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            for path in &event.paths {
                if !is_path_excluded(path, cfg) {
                    handle_path_remove(path, metadata, content_index);
                    changes += 1;
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            for path in &event.paths {
                if !is_path_excluded(path, cfg) {
                    handle_file_upsert(path, metadata, content_index);
                    changes += 1;
                }
            }
        }
        EventKind::Modify(ModifyKind::Name(_)) => {
            // Rename direction unknown: reconcile using the current filesystem state.
            for path in &event.paths {
                if is_path_excluded(path, cfg) {
                    continue;
                }
                if path.exists() {
                    handle_file_upsert(path, metadata, content_index);
                } else {
                    handle_path_remove(path, metadata, content_index);
                }
                changes += 1;
            }
        }
        EventKind::Remove(_) => {
            for path in &event.paths {
                if !is_path_excluded(path, cfg) {
                    handle_path_remove(path, metadata, content_index);
                    changes += 1;
                }
            }
        }
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in &event.paths {
                if !is_path_excluded(path, cfg) {
                    handle_file_upsert(path, metadata, content_index);
                    changes += 1;
                }
            }
        }
        _ => {}
    }

    changes
}

fn handle_file_upsert(
    path: &Path,
    metadata: &Arc<RwLock<MetadataStore>>,
    content_index: &Arc<RwLock<Option<ContentIndex>>>,
) {
    // Skip entries whose parent directory is already gone: creating them would
    // leave orphan rows behind after a directory removal.
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        return;
    }

    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(_) => return,
    };

    let is_dir = meta.is_dir();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let extension = if is_dir {
        None
    } else {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
    };

    let size = meta.len();
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (dev, ino) = file_identity(&meta);
    let char_bloom = crate::search::fuzzy::compute_char_mask(&name);

    let entry = FileEntry {
        id: 0,
        path: path.to_path_buf(),
        name: name.clone(),
        extension: extension.clone(),
        size,
        modified,
        is_dir,
        dev,
        ino,
        char_bloom,
    };

    metadata.write().add(entry);

    if !is_dir
        && (is_indexable_extension(extension.as_deref()) || is_indexable_file_name(&name))
        && let Some(content) = read_text_content(path, 100_000)
        && let Some(c_index) = content_index.read().as_ref()
    {
        let _ = c_index.add_or_update(path, &name, &content, modified);
    }
}

/// Removes a path from metadata and the content index.
///
/// When the path was a directory, every indexed descendant is removed as well,
/// since the filesystem no longer reports individual removals for all children.
fn handle_path_remove(
    path: &Path,
    metadata: &Arc<RwLock<MetadataStore>>,
    content_index: &Arc<RwLock<Option<ContentIndex>>>,
) {
    let was_dir = metadata
        .read()
        .get(path)
        .map(|entry| entry.is_dir)
        .unwrap_or(false);

    metadata.write().remove(path);
    if let Some(c_index) = content_index.read().as_ref() {
        let _ = c_index.remove(path);
    }

    if !was_dir {
        return;
    }

    let descendants: Vec<PathBuf> = {
        let store = metadata.read();
        store
            .get_entries()
            .iter()
            .filter(|entry| entry.path != path && entry.path.starts_with(path))
            .map(|entry| entry.path.clone())
            .collect()
    };

    if descendants.is_empty() {
        return;
    }

    {
        let mut store = metadata.write();
        for descendant in &descendants {
            store.remove(descendant);
        }
    }

    if let Some(c_index) = content_index.read().as_ref() {
        for descendant in &descendants {
            let _ = c_index.remove(descendant);
        }
    }
}
