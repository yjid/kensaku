use crate::search::fuzzy::compute_char_mask;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub id: u64,
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
    /// 64-bit character presence bitmask for fast prefiltering.
    pub char_bloom: u64,
    /// True when the entry's content must be re-read even if its mtime/size did
    /// not change (used to re-run image OCR after the setting is toggled).
    pub content_pending: bool,
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

/// True when the file is a cloud-backed placeholder that would be hydrated
/// (downloaded) if its contents were read.
///
/// Only Windows exposes this state through metadata attributes. On Linux and
/// macOS, cloud providers (iCloud Drive, OneDrive FUSE mounts, …) present
/// regular files whose stat does not trigger a download, so there is nothing
/// portable to detect; returning `false` keeps indexing unchanged there.
#[cfg(windows)]
pub fn is_cloud_placeholder(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    // https://learn.microsoft.com/windows/win32/fileio/file-attribute-constants
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

    let attributes = meta.file_attributes();
    attributes & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0
}

#[cfg(not(windows))]
pub fn is_cloud_placeholder(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Convenience wrapper for callers that only have a path.
///
/// On non-Windows platforms this never touches the filesystem.
pub fn is_cloud_placeholder_path(path: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        std::fs::metadata(path)
            .map(|meta| is_cloud_placeholder(&meta))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

pub fn walk_directories(
    dirs: &[PathBuf],
    excluded_dirs: &[String],
) -> Vec<FileEntry> {
    let mut result = Vec::new();
    let excluded_set: HashSet<String> = excluded_dirs.iter().map(|s| s.to_lowercase()).collect();

    // Default-deny sensitive paths. These are skipped even if the user has not
    // added them to exclusions. Hidden entries (names starting with '.') are
    // already skipped above, which covers ~/.ssh, ~/.gnupg, ~/.aws, etc.
    const SENSITIVE_NAMES: &[&str] = &[
        ".ssh", ".gnupg", ".gpg", ".aws", ".azure", ".kube", ".docker",
        ".password-store", "1password", "bitwarden", "keepass", "keepassxc",
        ".env", ".env.local", ".env.production", ".env.development",
    ];
    let sensitive_set: HashSet<String> = SENSITIVE_NAMES.iter().map(|s| s.to_string()).collect();

    for root in dirs {
        if !root.exists() {
            continue;
        }

        // TODO: make hidden-file skipping configurable via `KensakuConfig::skip_hidden`.
        let walker = jwalk::WalkDir::new(root)
            .skip_hidden(true)
            .process_read_dir({
                let excluded_set = excluded_set.clone();
                let sensitive_set = sensitive_set.clone();
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
                            if sensitive_set.contains(&name) {
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

            let char_bloom = compute_char_mask(&name);
            let file_entry = FileEntry {
                id: 0,
                path,
                name,
                extension,
                size,
                modified,
                is_dir,
                dev,
                ino,
                char_bloom,
                content_pending: false,
            };

            result.push(file_entry);
        }
    }

    result
}
