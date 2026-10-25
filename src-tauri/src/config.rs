use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KensakuConfig {
    pub watched_dirs: Vec<PathBuf>,
    pub excluded_dirs: Vec<String>,
    pub excluded_extensions: Vec<String>,
    pub shortcut: String,
    pub max_results: usize,
    pub is_setup_complete: bool,
    /// Skip files and directories whose name starts with `.` (hidden entries).
    /// Enabled by default; users can opt into indexing hidden files.
    #[serde(default = "default_true")]
    pub skip_hidden: bool,
    #[serde(default = "default_true")]
    pub semantic_enabled: bool,
    #[serde(default = "default_semantic_model")]
    pub semantic_model: String,
    #[serde(default = "default_true")]
    pub ocr_enabled: bool,
    /// Persisted UI theme ("paper" or "void").
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Config schema version, used for one-time migrations.
    #[serde(default)]
    pub config_version: u32,
}

fn default_true() -> bool {
    true
}

fn default_semantic_model() -> String {
    "static".to_string()
}

fn default_theme() -> String {
    "paper".to_string()
}

impl Default for KensakuConfig {
    fn default() -> Self {
        let mut watched = Vec::new();
        if let Some(home) = dirs::home_dir() {
            watched.push(home);
        }

        let default_excluded_dirs: Vec<String> = vec![
            // Version control
            ".git", ".svn", ".hg", ".bzr",
            // Package managers / dependencies
            "node_modules", "vendor", "Pods", ".bundle",
            // Build outputs
            "target", "build", "dist", "out", ".output", ".next", ".nuxt", ".svelte-kit", ".turbo",
            ".vercel", ".netlify",
            // Caches
            ".cache", ".npm", ".yarn", ".pnpm", ".cargo", ".rustup", ".gradle", ".m2", "__pycache__",
            ".pytest_cache", ".mypy_cache", ".tox", ".nox",
            // Virtual environments
            "venv", ".venv", "env", ".env",
            // IDE / editor
            ".vscode", ".idea", ".vs", ".eclipse",
            // OS junk
            ".DS_Store", "Thumbs.db", "desktop.ini",
            // Misc
            ".Trash", ".Trash-*", "lost+found",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // Documents (PDF/Office/EPUB) and images are indexed via text extraction
        // and OCR, so they are deliberately NOT excluded by default.
        let default_excluded_extensions: Vec<String> = vec![
            // Raw/editor-only image formats we cannot read text from
            "psd", "ai", "sketch", "fig",
            // Audio/Video
            "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac", "ogg", "webm",
            // Archives
            "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "tgz",
            // Binaries
            "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", "class", "pyc", "pyo",
            // Database
            "db", "sqlite", "sqlite3",
            // Lock files
            "lock", "lockb",
            // Minified/generated
            "min.js", "min.css", "map",
            // Font
            "ttf", "otf", "woff", "woff2", "eot",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        Self {
            watched_dirs: watched,
            excluded_dirs: default_excluded_dirs,
            excluded_extensions: default_excluded_extensions,
            shortcut: "Alt+Space".to_string(),
            max_results: 50,
            is_setup_complete: false,
            skip_hidden: true,
            semantic_enabled: true,
            semantic_model: "static".to_string(),
            ocr_enabled: true,
            theme: default_theme(),
            config_version: 1,
        }
    }
}

impl KensakuConfig {
    pub fn config_dir() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("kensaku")
        } else {
            PathBuf::from(".kensaku")
        }
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn index_dir() -> PathBuf {
        Self::config_dir().join("index")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists()
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(mut cfg) = serde_json::from_str::<Self>(&content)
        {
            if cfg.config_version < 1 {
                // v1: document/image formats became indexable (text extraction + OCR).
                // Strip the legacy defaults so existing installs start indexing them.
                const LEGACY_EXCLUDED: &[&str] = &[
                    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
                    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "tiff", "tif",
                ];
                cfg.excluded_extensions.retain(|ext| {
                    !LEGACY_EXCLUDED.iter().any(|legacy| legacy.eq_ignore_ascii_case(ext))
                });
                cfg.config_version = 1;
                let _ = cfg.save();
            }
            return cfg;
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn is_setup_done() -> bool {
        let cfg = Self::load();
        cfg.is_setup_complete
    }

    pub fn is_dir_excluded(&self, dir_name: &str) -> bool {
        self.excluded_dirs.iter().any(|ex| ex.eq_ignore_ascii_case(dir_name))
    }

    pub fn is_extension_excluded(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.trim_start_matches('.').to_ascii_lowercase();
            return self.excluded_extensions.iter().any(|e| {
                e.trim_start_matches('.').eq_ignore_ascii_case(&ext)
            });
        }
        false
    }
}
