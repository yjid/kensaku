//! Local semantic model artifact manager.
//!
//! Kensaku ships without the Potion embedding weights. This module tracks the
//! three files needed by the semantic layer, downloads the missing ones from
//! HuggingFace in a single background thread, and exposes progress to the UI.

use crate::config::KensakuConfig;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Size of the read buffer, and therefore the progress update interval.
const CHUNK_BYTES: usize = 64 * 1024;

struct Artifact {
    name: &'static str,
    urls: &'static [&'static str],
    bytes_expected: u64,
    sha256: Option<&'static str>,
}

/// Static manifest of the files required by the semantic model.
const ARTIFACTS: [Artifact; 3] = [
    Artifact {
        name: "model.safetensors",
        urls: &[
            "https://huggingface.co/minishlab/potion-base-8M/resolve/main/model.safetensors",
            "https://hf-mirror.com/minishlab/potion-base-8M/resolve/main/model.safetensors",
        ],
        bytes_expected: 30_236_760,
        sha256: Some("f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2"),
    },
    Artifact {
        name: "tokenizer.json",
        urls: &[
            "https://huggingface.co/minishlab/potion-base-8M/resolve/main/tokenizer.json",
            "https://hf-mirror.com/minishlab/potion-base-8M/resolve/main/tokenizer.json",
        ],
        bytes_expected: 683_666,
        sha256: Some("e67e803f624fb4d67dea1c730d06e1067e1b14d830e2c2202569e3ef0f70bb50"),
    },
    Artifact {
        name: "config.json",
        urls: &[
            "https://huggingface.co/minishlab/potion-base-8M/resolve/main/config.json",
            "https://hf-mirror.com/minishlab/potion-base-8M/resolve/main/config.json",
        ],
        bytes_expected: 202,
        sha256: Some("2a6ac0e9aaa356a68a5688070db78fc3a464fefe85d2f06a1905ce3718687553"),
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct ModelArtifact {
    pub name: String,
    pub url: String,
    pub bytes_expected: u64,
    pub bytes_on_disk: u64,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelProgress {
    pub file: String,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelsStatus {
    pub dir: String,
    pub ready: bool,
    pub downloading: bool,
    pub progress: Option<ModelProgress>,
    pub error: Option<String>,
    pub artifacts: Vec<ModelArtifact>,
    pub total_bytes: u64,
    pub total_present_bytes: u64,
}

/// Process-global guard so only one download thread can run at a time.
static DOWNLOADING: AtomicBool = AtomicBool::new(false);
static PROGRESS: OnceLock<Mutex<Option<ModelProgress>>> = OnceLock::new();
static LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn progress_slot() -> &'static Mutex<Option<ModelProgress>> {
    PROGRESS.get_or_init(|| Mutex::new(None))
}

fn error_slot() -> &'static Mutex<Option<String>> {
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

fn set_progress(progress: Option<ModelProgress>) {
    let mut guard = match progress_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = progress;
}

fn set_error(error: Option<String>) {
    let mut guard = match error_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = error;
}

fn current_progress() -> Option<ModelProgress> {
    match progress_slot().lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn current_error() -> Option<String> {
    match error_slot().lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

pub fn models_dir() -> PathBuf {
    KensakuConfig::config_dir().join("models")
}

/// Creates the models directory (0700 on unix) if needed.
fn ensure_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create models directory {}: {}", dir.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Failed to set permissions on {}: {}", dir.display(), e))?;
    }
    Ok(())
}

/// Current artifact inventory, derived entirely from disk.
pub fn status() -> ModelsStatus {
    let dir = models_dir();
    let mut artifacts = Vec::with_capacity(ARTIFACTS.len());
    let mut total_bytes = 0u64;
    let mut total_present_bytes = 0u64;

    for artifact in ARTIFACTS.iter() {
        let path = dir.join(artifact.name);
        let metadata = std::fs::metadata(&path).ok();
        let is_file = metadata.as_ref().map(|m| m.is_file()).unwrap_or(false);
        let bytes_on_disk = if is_file {
            metadata.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        total_bytes = total_bytes.saturating_add(artifact.bytes_expected);
        total_present_bytes = total_present_bytes.saturating_add(bytes_on_disk);

        artifacts.push(ModelArtifact {
            name: artifact.name.to_string(),
            url: artifact.urls.first().copied().unwrap_or("").to_string(),
            bytes_expected: artifact.bytes_expected,
            bytes_on_disk,
            present: is_file,
        });
    }

    let ready = artifacts.iter().all(|artifact| artifact.present);

    ModelsStatus {
        dir: dir.to_string_lossy().to_string(),
        ready,
        downloading: DOWNLOADING.load(Ordering::Relaxed),
        progress: current_progress(),
        error: current_error(),
        artifacts,
        total_bytes,
        total_present_bytes,
    }
}

/// Starts a background download of every missing artifact.
///
/// Returns immediately. Calling this while a download is already running is a
/// no-op.
pub fn download_all() {
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Err(e) = ensure_dir(&models_dir()) {
        set_error(Some(e));
        DOWNLOADING.store(false, Ordering::SeqCst);
        return;
    }

    set_error(None);
    set_progress(None);

    let spawned = std::thread::Builder::new()
        .name("kensaku-model-download".to_string())
        .spawn(run_download);

    if let Err(e) = spawned {
        set_error(Some(format!(
            "Failed to start model download thread: {}",
            e
        )));
        DOWNLOADING.store(false, Ordering::SeqCst);
    }
}

fn run_download() {
    match download_missing() {
        Ok(()) => {
            set_progress(None);
            set_error(None);
        }
        Err(e) => {
            set_progress(None);
            set_error(Some(e));
        }
    }
    DOWNLOADING.store(false, Ordering::SeqCst);
}

fn download_missing() -> Result<(), String> {
    let dir = models_dir();
    ensure_dir(&dir)?;

    let missing_bytes: u64 = ARTIFACTS
        .iter()
        .filter(|artifact| !dir.join(artifact.name).is_file())
        .map(|artifact| artifact.bytes_expected)
        .sum();
    if missing_bytes > 0
        && let Some(free) = free_bytes(&dir)
        && free < missing_bytes.saturating_add(64 * 1024 * 1024)
    {
        return Err(format!(
            "Not enough disk space for model files: need {} MB, have {} MB",
            missing_bytes / (1024 * 1024),
            free / (1024 * 1024)
        ));
    }

    // Always verify an existing artifact. A truncated or corrupt final file is
    // removed and downloaded again instead of being trusted by name alone.
    for artifact in ARTIFACTS.iter() {
        download_one(&dir, artifact)?;
    }

    Ok(())
}

#[cfg(unix)]
fn free_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
        Some(stat.f_bavail as u64 * stat.f_frsize as u64)
    } else {
        None
    }
}

#[cfg(not(unix))]
fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

fn download_one(dir: &Path, artifact: &Artifact) -> Result<(), String> {
    let final_path = dir.join(artifact.name);
    if final_path.is_file() && sha256_matches(&final_path, artifact.sha256)? {
        return Ok(());
    }
    if final_path.is_file() {
        let _ = std::fs::remove_file(&final_path);
    }

    let part_path = dir.join(format!("{}.part", artifact.name));
    let mut downloaded = part_path
        .metadata()
        .map(|meta| meta.len())
        .unwrap_or(0);
    if downloaded > artifact.bytes_expected && artifact.bytes_expected > 0 {
        let _ = std::fs::remove_file(&part_path);
        downloaded = 0;
    }

    set_progress(Some(ModelProgress {
        file: artifact.name.to_string(),
        downloaded,
        total: artifact.bytes_expected,
    }));

    let mut last_error = None;
    for url in artifact.urls {
        match download_url(url, artifact, &part_path, &mut downloaded) {
            Ok(()) => {
                if sha256_matches(&part_path, artifact.sha256)? {
                    std::fs::rename(&part_path, &final_path).map_err(|e| {
                        format!("Failed to finalize {}: {}", final_path.display(), e)
                    })?;
                    set_progress(Some(ModelProgress {
                        file: artifact.name.to_string(),
                        downloaded: artifact.bytes_expected,
                        total: artifact.bytes_expected,
                    }));
                    return Ok(());
                }
                let _ = std::fs::remove_file(&part_path);
                downloaded = 0;
                last_error = Some(format!("Checksum mismatch for {}", artifact.name));
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| format!("Download failed for {}", artifact.name)))
}

fn download_url(
    url: &str,
    artifact: &Artifact,
    part_path: &Path,
    downloaded: &mut u64,
) -> Result<(), String> {
    let mut request = ureq::get(url).timeout(Duration::from_secs(600));
    if *downloaded > 0 {
        request = request.set("Range", &format!("bytes={}-", *downloaded));
    }
    let response = request
        .call()
        .map_err(|e| format!("Download failed for {}: {}", artifact.name, e))?;

    let status = response.status();
    let (append, total) = match status {
        206 => {
            let total = response
                .header("Content-Range")
                .and_then(parse_content_range_total)
                .or_else(|| {
                    response
                        .header("Content-Length")
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(|len| len.saturating_add(*downloaded))
                })
                .unwrap_or(artifact.bytes_expected);
            (true, total)
        }
        200 => {
            let _ = std::fs::remove_file(part_path);
            *downloaded = 0;
            let total = response
                .header("Content-Length")
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(artifact.bytes_expected);
            (false, total)
        }
        other => {
            return Err(format!(
                "Unexpected HTTP status {} for {}",
                other, artifact.name
            ))
        }
    };

    let mut reader = response.into_reader();
    let mut file = if append {
        std::fs::OpenOptions::new()
            .append(true)
            .open(part_path)
            .map_err(|e| format!("Failed to reopen {}: {}", part_path.display(), e))?
    } else {
        std::fs::File::create(part_path)
            .map_err(|e| format!("Failed to create {}: {}", part_path.display(), e))?
    };

    let mut buffer = vec![0u8; CHUNK_BYTES];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Download interrupted for {}: {}", artifact.name, e))?;
        if read == 0 {
            break;
        }
        file
            .write_all(&buffer[..read])
            .map_err(|e| format!("Failed to write {}: {}", part_path.display(), e))?;
        *downloaded = downloaded.saturating_add(read as u64);
        set_progress(Some(ModelProgress {
            file: artifact.name.to_string(),
            downloaded: *downloaded,
            total,
        }));
    }
    if total > 0 && *downloaded < total {
        return Err(format!(
            "Download interrupted for {}: expected {} bytes, got {}",
            artifact.name, total, downloaded
        ));
    }
    file
        .flush()
        .map_err(|e| format!("Failed to flush {}: {}", part_path.display(), e))?;
    Ok(())
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.parse::<u64>().ok()
}

fn sha256_matches(path: &Path, expected: Option<&str>) -> Result<bool, String> {
    let Some(expected) = expected else {
        return Ok(true);
    };
    let actual = sha256_file(path)?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{:x}", digest))
}

/// Removes every model artifact and resets progress/error state.
pub fn delete_all() -> Result<(), String> {
    if DOWNLOADING.load(Ordering::SeqCst) {
        return Err("A model download is in progress. Try again once it finishes.".to_string());
    }

    let dir = models_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("Failed to delete {}: {}", dir.display(), e))?;
    }
    ensure_dir(&dir)?;

    set_progress(None);
    set_error(None);
    Ok(())
}

#[tauri::command]
pub async fn get_models_status() -> Result<ModelsStatus, String> {
    Ok(status())
}

#[tauri::command]
pub async fn download_models() -> Result<(), String> {
    download_all();
    Ok(())
}

#[tauri::command]
pub async fn delete_models() -> Result<(), String> {
    delete_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_detects_corruption() {
        let dir = std::env::temp_dir().join("kensaku-model-test-corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("artifact.bin");
        std::fs::write(&path, b"hello").unwrap();
        let digest = sha256_file(&path).unwrap();
        assert!(sha256_matches(&path, Some(&digest)).unwrap());
        assert!(!sha256_matches(&path, Some("0000000000000000000000000000000000000000000000000000000000000000")).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_matches_skips_missing_checksum() {
        let dir = std::env::temp_dir().join("kensaku-model-test-skip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("artifact.bin");
        std::fs::write(&path, b"hello").unwrap();
        assert!(sha256_matches(&path, None).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}


#[cfg(test)]
mod download_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_server<F>(handler: F) -> String
    where
        F: Fn(std::net::TcpStream) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                handler(stream);
            }
        });
        format!("http://{}", addr)
    }

    #[test]
    fn download_resumes_from_partial_file() {
        let full = b"hello world";
        let url = spawn_server(move |mut stream| {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let request = String::from_utf8_lossy(&buf);
            let start = request
                .lines()
                .find_map(|line| line.strip_prefix("Range: bytes="))
                .and_then(|value| value.trim_end_matches('-').parse::<usize>().ok())
                .unwrap_or(0);
            let body = &full[start..];
            let response = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                start,
                full.len() - 1,
                full.len(),
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(body);
        });

        let dir = std::env::temp_dir().join("kensaku-download-resume");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("artifact.bin.part");
        std::fs::write(&part, b"hello ").unwrap();

        let url_static: &'static str = Box::leak(url.into_boxed_str());
        let artifact = Artifact {
            name: "artifact.bin",
            urls: Box::leak(vec![url_static].into_boxed_slice()),
            bytes_expected: full.len() as u64,
            sha256: None,
        };
        download_one(&dir, &artifact).unwrap();
        assert_eq!(std::fs::read(dir.join("artifact.bin")).unwrap(), full);
        assert!(!part.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_falls_back_to_mirror() {
        let full = b"payload";
        let bad = spawn_server(|mut stream| {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        });
        let good = spawn_server(move |mut stream| {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                full.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(full);
        });

        let dir = std::env::temp_dir().join("kensaku-download-mirror");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bad_static: &'static str = Box::leak(bad.into_boxed_str());
        let good_static: &'static str = Box::leak(good.into_boxed_str());
        let artifact = Artifact {
            name: "artifact.bin",
            urls: Box::leak(vec![bad_static, good_static].into_boxed_slice()),
            bytes_expected: full.len() as u64,
            sha256: None,
        };
        download_one(&dir, &artifact).unwrap();
        assert_eq!(std::fs::read(dir.join("artifact.bin")).unwrap(), full);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn interrupted_download_keeps_partial_file() {
        let full = b"hello world";
        let url = spawn_server(move |mut stream| {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                full.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&full[..5]);
        });

        let dir = std::env::temp_dir().join("kensaku-download-interrupted");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let url_static: &'static str = Box::leak(url.into_boxed_str());
        let artifact = Artifact {
            name: "artifact.bin",
            urls: Box::leak(vec![url_static].into_boxed_slice()),
            bytes_expected: full.len() as u64,
            sha256: None,
        };
        let result = download_one(&dir, &artifact);
        assert!(result.is_err());
        assert!(dir.join("artifact.bin.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
