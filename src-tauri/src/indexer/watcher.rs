use crate::config::KensakuConfig;
use crate::indexer::content::{is_indexable_extension, is_indexable_file_name, read_text_content, ContentIndex};
use crate::indexer::metadata::MetadataStore;
use crate::indexer::walker::FileEntry;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

pub fn start_file_watcher(
    watched_dirs: Vec<PathBuf>,
    metadata: Arc<RwLock<MetadataStore>>,
    content_index: Arc<RwLock<Option<ContentIndex>>>,
    config: Arc<RwLock<KensakuConfig>>,
) {
    thread::spawn(move || {
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
            // Read with timeout so we can periodically commit
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => {
                    let cfg = config.read().clone();
                    for path in event.paths {
                        // Check if path is excluded
                        let is_excluded = path.components().any(|c| {
                            if let Some(s) = c.as_os_str().to_str() {
                                cfg.is_dir_excluded(s)
                            } else {
                                false
                            }
                        });

                        if is_excluded {
                            continue;
                        }

                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                handle_file_upsert(&path, &metadata, &content_index);
                                pending_commits += 1;
                            }
                            EventKind::Remove(_) => {
                                handle_file_remove(&path, &metadata, &content_index);
                                pending_commits += 1;
                            }
                            _ => {}
                        }
                    }
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
            if pending_commits > 0 {
                if let Ok(elapsed) = last_commit.elapsed() {
                    if elapsed >= Duration::from_secs(3) || pending_commits > 50 {
                        if let Some(c_index) = content_index.read().as_ref() {
                            let _ = c_index.commit();
                        }
                        pending_commits = 0;
                        last_commit = SystemTime::now();
                    }
                }
            }
        }
    });
}

fn handle_file_upsert(
    path: &Path,
    metadata: &Arc<RwLock<MetadataStore>>,
    content_index: &Arc<RwLock<Option<ContentIndex>>>,
) {
    if !path.exists() {
        return;
    }

    let is_dir = path.is_dir();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let extension = if is_dir {
        None
    } else {
        path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_lowercase())
    };

    let (size, modified) = match std::fs::metadata(path) {
        Ok(m) => {
            let sz = m.len();
            let mod_time = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (sz, mod_time)
        }
        Err(_) => (0, 0),
    };

    let entry = FileEntry {
        path: path.to_path_buf(),
        name: name.clone(),
        extension: extension.clone(),
        size,
        modified,
        is_dir,
    };

    metadata.write().add(entry);

    if !is_dir && (is_indexable_extension(extension.as_deref()) || is_indexable_file_name(&name)) {
        if let Some(content) = read_text_content(path, 100_000) {
            if let Some(c_index) = content_index.read().as_ref() {
                let _ = c_index.add_or_update(path, &name, &content, modified);
            }
        }
    }
}

fn handle_file_remove(
    path: &Path,
    metadata: &Arc<RwLock<MetadataStore>>,
    content_index: &Arc<RwLock<Option<ContentIndex>>>,
) {
    metadata.write().remove(path);
    if let Some(c_index) = content_index.read().as_ref() {
        let _ = c_index.remove(path);
    }
}
