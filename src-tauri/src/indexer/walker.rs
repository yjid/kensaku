use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified: u64,
    pub is_dir: bool,
    /// Filesystem device id (`st_dev`). `0` means unknown.
    pub dev: u64,
    /// Filesystem inode number (`st_ino`). `0` means unknown.
    pub ino: u64,
}

/// Returns `(device_id, inode)` for a filesystem metadata entry.
///
/// These values form the stable identity of a file across path-based renames
/// and replacements. On non-Unix platforms they are unavailable and default
/// to `0`, which callers treat as "unknown".
pub fn file_identity(meta: &std::fs::Metadata) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        (0, 0)
    }
}

pub fn walk_directories<F>(
    dirs: &[PathBuf],
    excluded_dirs: &[String],
    mut on_entry: F,
) where
    F: FnMut(FileEntry) -> bool,
{
    let excluded_set: HashSet<String> = excluded_dirs.iter().map(|s| s.to_lowercase()).collect();

    for root in dirs {
        if !root.exists() {
            continue;
        }

        // TODO: make hidden-file skipping configurable via `KensakuConfig::skip_hidden`.
        let walker = jwalk::WalkDir::new(root)
            .skip_hidden(true)
            .process_read_dir({
                let excluded_set = excluded_set.clone();
                move |_depth, _path, _read_dir_state, children| {
                    children.retain(|dir_entry_result| {
                        if let Ok(entry) = dir_entry_result {
                            let name = entry.file_name.to_string_lossy().to_lowercase();
                            // Belt-and-suspenders: never descend into hidden entries
                            // even if `skip_hidden` is unsupported on this platform.
                            if name.starts_with('.') {
                                return false;
                            }
                            if excluded_set.contains(&name) {
                                return false;
                            }
                        }
                        true
                    });
                }
            });

        for entry_res in walker {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let is_dir = entry.file_type().is_dir();
            let name = entry.file_name.to_string_lossy().to_string();

            let extension = if is_dir {
                None
            } else {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_lowercase())
            };

            let (size, modified, dev, ino) = match entry.metadata() {
                Ok(meta) => {
                    let sz = meta.len();
                    let mod_time = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let (dev, ino) = file_identity(&meta);
                    (sz, mod_time, dev, ino)
                }
                Err(_) => (0, 0, 0, 0),
            };

            let file_entry = FileEntry {
                path,
                name,
                extension,
                size,
                modified,
                is_dir,
                dev,
                ino,
            };

            if !on_entry(file_entry) {
                return;
            }
        }
    }
}
