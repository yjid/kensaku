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

        let walker = jwalk::WalkDir::new(root)
            .skip_hidden(false)
            .process_read_dir({
                let excluded_set = excluded_set.clone();
                move |_depth, _path, _read_dir_state, children| {
                    children.retain(|dir_entry_result| {
                        if let Ok(entry) = dir_entry_result {
                            let name = entry.file_name.to_string_lossy().to_lowercase();
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

            let (size, modified) = match entry.metadata() {
                Ok(meta) => {
                    let sz = meta.len();
                    let mod_time = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    (sz, mod_time)
                }
                Err(_) => (0, 0),
            };

            let file_entry = FileEntry {
                path,
                name,
                extension,
                size,
                modified,
                is_dir,
            };

            if !on_entry(file_entry) {
                return;
            }
        }
    }
}
