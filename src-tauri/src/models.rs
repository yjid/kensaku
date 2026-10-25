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
    url: &'static str,
    bytes_expected: u64,
}

/// Static manifest of the files required by the semantic model.
const ARTIFACTS: [Artifact; 3] = [
    Artifact {
        name: "model.safetensors",
        url: "https://huggingface.co/minishlab/potion-base-8M/resolve/main/model.safetensors",
        bytes_expected: 30_236_760,
    },
    Artifact {
        name: "tokenizer.json",
        url: "https://huggingface.co/minishlab/potion-base-8M/resolve/main/tokenizer.json",
        bytes_expected: 683_666,
    },
    Artifact {
        name: "config.json",
        url: "https://huggingface.co/minishlab/potion-base-8M/resolve/main/config.json",
        bytes_expected: 202,
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
            url: artifact.url.to_string(),
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

    for artifact in ARTIFACTS.iter() {
        if dir.join(artifact.name).is_file() {
            continue;
        }
        download_one(&dir, artifact)?;
    }

    Ok(())
}

fn download_one(dir: &Path, artifact: &Artifact) -> Result<(), String> {
    let final_path = dir.join(artifact.name);
    let part_path = dir.join(format!("{}.part", artifact.name));

    // A stale partial file from a previous attempt is never resumable.
    let _ = std::fs::remove_file(&part_path);

    set_progress(Some(ModelProgress {
        file: artifact.name.to_string(),
        downloaded: 0,
        total: artifact.bytes_expected,
    }));

    let result = write_artifact(artifact, &part_path);

    if result.is_err() {
        let _ = std::fs::remove_file(&part_path);
    }

    result?;

    std::fs::rename(&part_path, &final_path)
        .map_err(|e| format!("Failed to finalize {}: {}", final_path.display(), e))?;

    Ok(())
}

fn write_artifact(artifact: &Artifact, part_path: &Path) -> Result<(), String> {
    let response = ureq::get(artifact.url)
        .timeout(Duration::from_secs(600))
        .call()
        .map_err(|e| format!("Download failed for {}: {}", artifact.name, e))?;

    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(artifact.bytes_expected);

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(part_path)
        .map_err(|e| format!("Failed to create {}: {}", part_path.display(), e))?;

    let mut buffer = vec![0u8; CHUNK_BYTES];
    let mut downloaded = 0u64;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Download interrupted for {}: {}", artifact.name, e))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])
            .map_err(|e| format!("Failed to write {}: {}", part_path.display(), e))?;

        downloaded = downloaded.saturating_add(read as u64);
        set_progress(Some(ModelProgress {
            file: artifact.name.to_string(),
            downloaded,
            total,
        }));
    }

    file.flush()
        .map_err(|e| format!("Failed to flush {}: {}", part_path.display(), e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync {}: {}", part_path.display(), e))?;

    if total > 0 && downloaded == 0 {
        return Err(format!("Downloaded 0 bytes for {}", artifact.name));
    }

    Ok(())
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
