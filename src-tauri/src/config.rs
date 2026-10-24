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
}

impl Default for KensakuConfig {
    fn default() -> Self {
        let mut watched = Vec::new();
        if let Some(home) = dirs::home_dir() {
            watched.push(home);
        }

        let default_excluded_dirs = vec![
            "node_modules".to_string(),
            ".git".to_string(),
            "target".to_string(),
            "__pycache__".to_string(),
            ".cache".to_string(),
            ".npm".to_string(),
            ".cargo".to_string(),
            ".rustup".to_string(),
            "dist".to_string(),
            "build".to_string(),
            ".next".to_string(),
            ".nuxt".to_string(),
            ".output".to_string(),
            ".venv".to_string(),
            "venv".to_string(),
            "env".to_string(),
            ".gradle".to_string(),
            ".m2".to_string(),
            ".idea".to_string(),
            ".vscode".to_string(),
            "vendor".to_string(),
            "bower_components".to_string(),
            ".Trash".to_string(),
            "Library".to_string(),
        ];

        let default_excluded_extensions = vec![
            ".o".to_string(),
            ".so".to_string(),
            ".dll".to_string(),
            ".exe".to_string(),
            ".bin".to_string(),
            ".dat".to_string(),
            ".db".to_string(),
            ".sqlite".to_string(),
            ".class".to_string(),
            ".pyc".to_string(),
            ".wasm".to_string(),
            ".jpg".to_string(),
            ".jpeg".to_string(),
            ".png".to_string(),
            ".gif".to_string(),
            ".bmp".to_string(),
            ".mp3".to_string(),
            ".mp4".to_string(),
            ".avi".to_string(),
            ".mov".to_string(),
            ".zip".to_string(),
            ".tar".to_string(),
            ".gz".to_string(),
            ".rar".to_string(),
            ".7z".to_string(),
            ".iso".to_string(),
            ".dmg".to_string(),
            ".pdf".to_string(),
        ];

        Self {
            watched_dirs: watched,
            excluded_dirs: default_excluded_dirs,
            excluded_extensions: default_excluded_extensions,
            shortcut: "Alt+Space".to_string(),
            max_results: 50,
            is_setup_complete: false,
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
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                    return cfg;
                }
            }
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
            let dot_ext = format!(".{}", ext.to_ascii_lowercase());
            return self.excluded_extensions.iter().any(|e| e.to_ascii_lowercase() == dot_ext);
        }
        false
    }
}
