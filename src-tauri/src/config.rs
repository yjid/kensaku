use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    /// Word the user types.
    pub term: String,
    /// Other spellings that should match the same thing.
    pub expansions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub name: String,
    pub query: String,
    #[serde(default)]
    pub sort: Option<String>,
}

/// Opt-in external text extractor. Commands are executed directly (never
/// through a shell), with a cleared environment and a hard timeout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalExtractor {
    /// Shown in extraction metadata and warnings.
    pub name: String,
    /// Lowercase extensions without the dot, e.g. `["stl", "obj"]`.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Optional MIME selectors, e.g. `["model/*"]`. Matched alongside
    /// `extensions` when either matches. `mime_types` is accepted as an alias.
    #[serde(default, alias = "mime_types")]
    pub mime: Vec<String>,
    /// Absolute path to an executable file. PATH lookup is never used.
    pub command: String,
    /// Individual arguments. `{path}` is replaced with the file path; if no
    /// argument contains it, the path is appended as the last argument.
    #[serde(default)]
    pub args: Vec<String>,
    /// Wall-clock limit; clamped to 100..=7000 ms by the runner.
    #[serde(default = "default_extractor_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum captured stdout; clamped to 1 KiB..=4 MiB.
    #[serde(default = "default_extractor_output_bytes")]
    pub max_output_bytes: u64,
    /// External commands never run unless this is explicitly true.
    #[serde(default)]
    pub enabled: bool,
}

pub fn default_extractor_timeout_ms() -> u64 {
    5_000
}

pub fn default_extractor_output_bytes() -> u64 {
    1_000_000
}

pub fn default_aliases() -> Vec<Alias> {
    [
        ("resume", &["cv", "curriculum vitae"][..]),
        ("invoice", &["receipt", "bill"][..]),
        ("receipt", &["invoice", "bill"][..]),
        ("screenshot", &["screen shot", "capture"][..]),
        ("contract", &["agreement"][..]),
        ("photo", &["image", "picture"][..]),
    ]
    .into_iter()
    .map(|(term, expansions)| Alias {
        term: term.to_string(),
        expansions: expansions.iter().map(|s| s.to_string()).collect(),
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CrawlMode {
    Full,
    #[default]
    Balanced,
    Eco,
}

impl CrawlMode {
    /// True when indexing should run at background priority.
    pub fn is_background(self) -> bool {
        !matches!(self, Self::Full)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Balanced => "balanced",
            Self::Eco => "eco",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KensakuConfig {
    pub watched_dirs: Vec<PathBuf>,
    pub excluded_dirs: Vec<String>,
    pub excluded_extensions: Vec<String>,
    pub shortcut: String,
    pub max_results: usize,
    pub is_setup_complete: bool,
    /// True once the *first* full crawl finished. `is_setup_complete` in the
    /// command layer is the AND of this and `is_setup_complete` (configured).
    #[serde(default)]
    pub first_scan_done: bool,
    /// Skip files and directories whose name starts with `.` (hidden entries).
    /// Enabled by default; users can opt into indexing hidden files.
    #[serde(default = "default_true")]
    pub skip_hidden: bool,
    #[serde(default = "default_true")]
    pub semantic_enabled: bool,
    #[serde(default = "default_semantic_model")]
    pub semantic_model: String,
    /// Keep the static embedding table in f32 for one release. The default is
    /// the compact i8 table; this switch exists for quality comparisons and
    /// rollback without rebuilding.
    #[serde(default)]
    pub semantic_f32_table: bool,
    /// Background indexing policy. `balanced` uses idle priority; `eco` also
    /// paces work on battery. `full` stays at normal priority.
    #[serde(default)]
    pub crawl_mode: CrawlMode,
    /// Pause background crawling without stopping the watcher. Renames and
    /// deletes still apply while paused.
    #[serde(default)]
    pub pause_indexing: bool,
    /// Allow MCP agents to call the opt-in `reindex` write tool. Disabled by
    /// default; read-only tools are always available.
    #[serde(default)]
    pub mcp_allow_reindex: bool,
    #[serde(default = "default_true")]
    pub ocr_enabled: bool,
    /// OCR backend: `tesseract` (fast default) or `accurate` (optional
    /// PP-OCRv6 pack). Falls back to Tesseract when the pack is absent.
    #[serde(default = "default_ocr_engine")]
    pub ocr_engine: String,
    /// Opt-in external extractors keyed by extension or MIME type. Empty by
    /// default: no external command ever runs without explicit configuration.
    #[serde(default)]
    pub extractors: Vec<ExternalExtractor>,
    /// Beta visual-search model pack. This only gates the Beta settings tab and
    /// model download; it is intentionally not wired into search yet.
    #[serde(default)]
    pub visual_search_beta_enabled: bool,
    /// Local LLM provider: `off` (default), `ollama`, `lmstudio`, or `openai`
    /// (any OpenAI-compatible endpoint; requires `llm_allow_cloud`).
    #[serde(default)]
    pub llm_provider: String,
    /// LLM base URL override. Empty uses the provider's loopback default.
    #[serde(default)]
    pub llm_base_url: String,
    /// Model name used by the query compiler and `/ask`.
    #[serde(default)]
    pub llm_model: String,
    /// Allow non-loopback (cloud) LLM endpoints. Off by default; `/ask` also
    /// asks for consent per use when the endpoint is not loopback.
    #[serde(default)]
    pub llm_allow_cloud: bool,
    /// Refine natural-language queries with the configured local LLM after
    /// the deterministic intent pass. Off by default.
    #[serde(default)]
    pub llm_compile_queries: bool,
    /// Agent-surface scope: when non-empty, MCP/HTTP/CLI results and snippet
    /// reads are restricted to these roots (read-only scoping per root).
    #[serde(default)]
    pub agent_scoped_roots: Vec<String>,
    /// Include files matched by `.gitignore` / `.cursorignore` in the index.
    /// Default false: ignore files are honored so agents see the same tree
    /// developers do ("ignore parity").
    #[serde(default)]
    pub include_ignored: bool,
    /// Persisted UI theme ("paper" or "void").
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Personal vocabulary: `term` matches any of `expansions`.
    #[serde(default = "default_aliases")]
    pub aliases: Vec<Alias>,
    /// Saved queries shown on the home screen.
    #[serde(default)]
    pub saved_searches: Vec<SavedSearch>,
    /// Pinned file paths shown on the home screen.
    #[serde(default)]
    pub pins: Vec<String>,
    /// Show generated thumbnails on result cards. On by default; off shows
    /// the type icon only (no thumbnail generation at all).
    #[serde(default = "default_true")]
    pub show_thumbnails: bool,
    /// Result layout: "grid" (cards, default) or "list" (one row per file).
    #[serde(default = "default_results_view")]
    pub results_view: String,
    /// Config schema version, used for one-time migrations.
    #[serde(default)]
    pub config_version: u32,
}

fn default_true() -> bool {
    true
}

fn default_ocr_engine() -> String {
    "tesseract".to_string()
}

fn default_semantic_model() -> String {
    "static".to_string()
}

fn default_results_view() -> String {
    "grid".to_string()
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
            ".git",
            ".svn",
            ".hg",
            ".bzr",
            // Package managers / dependencies
            "node_modules",
            "vendor",
            "Pods",
            ".bundle",
            // Build outputs
            "target",
            "build",
            "dist",
            "out",
            ".output",
            ".next",
            ".nuxt",
            ".svelte-kit",
            ".turbo",
            ".vercel",
            ".netlify",
            // Caches
            ".cache",
            ".npm",
            ".yarn",
            ".pnpm",
            ".cargo",
            ".rustup",
            ".gradle",
            ".m2",
            "__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            ".tox",
            ".nox",
            // Virtual environments
            "venv",
            ".venv",
            "env",
            ".env",
            // IDE / editor
            ".vscode",
            ".idea",
            ".vs",
            ".eclipse",
            // OS junk
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            // Misc
            ".Trash",
            ".Trash-*",
            "lost+found",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // Documents (PDF/Office/EPUB) and images are indexed via text extraction
        // and OCR, so they are deliberately NOT excluded by default.
        let default_excluded_extensions: Vec<String> = vec![
            // Raw/editor-only image formats we cannot read text from
            "psd", "ai", "sketch", "fig", // Audio/Video
            "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac", "ogg", "webm", // Archives
            "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "tgz", // Binaries
            "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", "class", "pyc", "pyo",
            // Secrets / credentials
            "pem", "key", "pfx", "p12", "crt", "cer", // Database
            "db", "sqlite", "sqlite3", // Lock files
            "lock", "lockb", // Minified/generated
            "min.js", "min.css", "map", // Font
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
            first_scan_done: false,
            skip_hidden: true,
            semantic_enabled: true,
            semantic_model: "static".to_string(),
            semantic_f32_table: false,
            crawl_mode: CrawlMode::Full,
            pause_indexing: false,
            mcp_allow_reindex: false,
            visual_search_beta_enabled: false,
            llm_provider: String::new(),
            llm_base_url: String::new(),
            llm_model: String::new(),
            llm_allow_cloud: false,
            llm_compile_queries: false,
            agent_scoped_roots: Vec::new(),
            include_ignored: false,
            extractors: Vec::new(),
            ocr_enabled: true,
            ocr_engine: default_ocr_engine(),
            theme: default_theme(),
            aliases: default_aliases(),
            saved_searches: Vec::new(),
            pins: Vec::new(),
            show_thumbnails: true,
            results_view: default_results_view(),
            config_version: 4,
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

    /// Parses `content` and applies one-time migrations.
    ///
    /// Returns the config plus whether anything changed, so callers that loaded
    /// from disk can persist the migrated form while [`Self::load_from_str`]
    /// stays side-effect free.
    fn parse_and_migrate(content: &str) -> Result<(Self, bool), serde_json::Error> {
        let mut cfg: Self = serde_json::from_str(content)?;
        let mut migrated = false;

        if cfg.config_version < 1 {
            // v1: document/image formats became indexable (text extraction + OCR).
            // Strip the legacy defaults so existing installs start indexing them.
            const LEGACY_EXCLUDED: &[&str] = &[
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg", "jpeg", "gif",
                "bmp", "ico", "webp", "tiff", "tif",
            ];
            cfg.excluded_extensions.retain(|ext| {
                !LEGACY_EXCLUDED
                    .iter()
                    .any(|legacy| legacy.eq_ignore_ascii_case(ext))
            });
            cfg.config_version = 1;
            migrated = true;
        }

        if cfg.config_version < 2 {
            // v2: early UI configs stored exclusions with a leading dot
            // (".jpg"), so the v1 bare-name strip missed them and documents /
            // images were never indexed. Normalize before comparing.
            const NOW_INDEXABLE: &[&str] = &[
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "epub", "odt", "ods", "odp",
                "rtf", "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif",
            ];
            cfg.excluded_extensions.retain(|ext| {
                let normalized = ext.trim_start_matches('.').to_ascii_lowercase();
                !NOW_INDEXABLE.contains(&normalized.as_str())
            });
            cfg.config_version = 2;
            migrated = true;
        }

        if cfg.config_version < 3 {
            // v3: first-scan completion gate. Existing installs that already
            // completed setup are treated as having finished their first scan.
            if cfg.is_setup_complete {
                cfg.first_scan_done = true;
            }
            cfg.config_version = 3;
            migrated = true;
        }

        if cfg.config_version < 4 {
            // v4: default-deny common secret/credential file extensions.
            const SENSITIVE_EXTS: &[&str] = &["pem", "key", "pfx", "p12", "crt", "cer"];
            for ext in SENSITIVE_EXTS {
                if !cfg
                    .excluded_extensions
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(ext))
                {
                    cfg.excluded_extensions.push((*ext).to_string());
                }
            }
            cfg.config_version = 4;
            migrated = true;
        }

        Ok((cfg, migrated))
    }

    /// Disk-free equivalent of [`Self::load`], used by tests to exercise the
    /// migration path without touching the user's config directory.
    pub fn load_from_str(content: &str) -> Self {
        match Self::parse_and_migrate(content) {
            Ok((cfg, _migrated)) => cfg,
            Err(_) => Self::default(),
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!(
                    "[Kensaku] Could not read config {}: {}; using defaults",
                    path.display(),
                    e
                );
                return Self::default();
            }
        };

        match Self::parse_and_migrate(&content) {
            Ok((cfg, migrated)) => {
                if migrated {
                    let _ = cfg.save();
                }
                cfg
            }
            Err(e) => {
                // Preserve the unreadable file instead of silently discarding
                // the user's settings: the next `save()` will overwrite it.
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = Self::config_dir().join(format!("config.json.corrupt-{}", stamp));
                match fs::copy(&path, &backup) {
                    Ok(_) => eprintln!(
                        "[Kensaku] Config is corrupt ({}); backup saved to {}; using defaults",
                        e,
                        backup.display()
                    ),
                    Err(copy_err) => eprintln!(
                        "[Kensaku] Config is corrupt ({}); failed to back up {}: {}; using defaults",
                        e,
                        path.display(),
                        copy_err
                    ),
                }
                Self::default()
            }
        }
    }

    /// Atomically persists the config: write a temp file, fsync it, rename it
    /// over the real config, then (best effort on unix) fsync the directory so
    /// the rename itself is durable.
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::config_dir())
    }

    /// Writes `<dir>/config.json` atomically. Extracted from [`Self::save`] so
    /// tests can target a temporary directory instead of the real one.
    pub fn save_to(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join("config.json");
        let tmp_path = dir.join("config.json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;

        {
            let mut file = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
            file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }

        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            // `File::sync_all` on the directory flushes the rename entry. Some
            // filesystems reject fsync on directories, hence best effort.
            if let Ok(dir_file) = fs::File::open(dir) {
                let _ = dir_file.sync_all();
            }
        }

        Ok(())
    }

    pub fn is_setup_done() -> bool {
        let cfg = Self::load();
        cfg.is_setup_complete && cfg.first_scan_done
    }

    /// Alias table for the intent compiler: (term, expansions).
    pub fn alias_pairs(&self) -> Vec<(String, Vec<String>)> {
        self.aliases
            .iter()
            .filter(|a| !a.term.trim().is_empty() && !a.expansions.is_empty())
            .map(|a| (a.term.trim().to_lowercase(), a.expansions.clone()))
            .collect()
    }

    pub fn is_dir_excluded(&self, dir_name: &str) -> bool {
        self.excluded_dirs
            .iter()
            .any(|ex| ex.eq_ignore_ascii_case(dir_name))
    }

    pub fn is_extension_excluded(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.trim_start_matches('.').to_ascii_lowercase();
            return self
                .excluded_extensions
                .iter()
                .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kensaku-config-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_config_is_v2_and_paper() {
        let cfg = KensakuConfig::default();
        assert_eq!(cfg.config_version, 4);
        assert!(cfg.skip_hidden);
        assert!(cfg.ocr_enabled);
        assert_eq!(cfg.theme, "paper");
    }

    #[test]
    fn semantic_f32_table_flag_defaults_off_and_round_trips() {
        let cfg = KensakuConfig::default();
        assert!(!cfg.semantic_f32_table);
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed = KensakuConfig::load_from_str(&json);
        assert!(!parsed.semantic_f32_table);

        let with_flag = json.replace(
            "\"semantic_f32_table\":false",
            "\"semantic_f32_table\":true",
        );
        assert!(KensakuConfig::load_from_str(&with_flag).semantic_f32_table);
    }

    #[test]
    fn crawl_mode_and_pause_parse_from_config() {
        let cfg = KensakuConfig::default();
        assert_eq!(cfg.crawl_mode, CrawlMode::Full);
        assert!(!cfg.pause_indexing);

        let raw = r#"{
            "watched_dirs": [],
            "excluded_dirs": [],
            "excluded_extensions": [],
            "shortcut": "Alt+Space",
            "max_results": 50,
            "is_setup_complete": false,
            "crawl_mode": "eco",
            "pause_indexing": true
        }"#;
        let parsed = KensakuConfig::load_from_str(raw);
        assert_eq!(parsed.crawl_mode, CrawlMode::Eco);
        assert!(parsed.pause_indexing);
    }

    #[test]
    fn legacy_config_is_migrated() {
        let legacy = r#"{
            "watched_dirs": [],
            "excluded_dirs": [],
            "excluded_extensions": ["pdf", "PNG", "doc", "rs", "png", "zip"],
            "shortcut": "Alt+Space",
            "max_results": 50,
            "is_setup_complete": false
        }"#;

        let cfg = KensakuConfig::load_from_str(legacy);

        assert_eq!(cfg.config_version, 4);
        // Legacy document/image extensions are stripped (case-insensitively)...
        assert!(
            !cfg.excluded_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case("pdf"))
        );
        assert!(
            !cfg.excluded_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case("png"))
        );
        assert!(
            !cfg.excluded_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case("doc"))
        );
        // ...while user-defined exclusions survive.
        assert!(cfg.excluded_extensions.iter().any(|e| e == "rs"));
        assert!(cfg.excluded_extensions.iter().any(|e| e == "zip"));
        // Missing defaults are filled in by serde.
        assert!(cfg.skip_hidden);
        assert!(cfg.ocr_enabled);
        assert_eq!(cfg.theme, "paper");
    }

    #[test]
    fn already_migrated_config_is_unchanged() {
        let current = r#"{
            "watched_dirs": [],
            "excluded_dirs": [],
            "excluded_extensions": ["rs", "node_modules"],
            "shortcut": "Alt+Space",
            "max_results": 50,
            "is_setup_complete": false,
            "config_version": 2
        }"#;

        let cfg = KensakuConfig::load_from_str(current);
        assert_eq!(cfg.config_version, 4);
        assert_eq!(
            cfg.excluded_extensions,
            vec![
                "rs".to_string(),
                "node_modules".to_string(),
                "pem".to_string(),
                "key".to_string(),
                "pfx".to_string(),
                "p12".to_string(),
                "crt".to_string(),
                "cer".to_string(),
            ]
        );
    }

    #[test]
    fn v2_migration_strips_dotted_document_and_image_extensions() {
        let legacy = r#"{
            "watched_dirs": [],
            "excluded_dirs": [],
            "excluded_extensions": [".jpg", ".PNG", ".pdf", ".o", "zip"],
            "shortcut": "Alt+Space",
            "max_results": 50,
            "is_setup_complete": false,
            "config_version": 1
        }"#;

        let cfg = KensakuConfig::load_from_str(legacy);
        assert_eq!(cfg.config_version, 4);
        assert_eq!(
            cfg.excluded_extensions,
            vec![
                ".o".to_string(),
                "zip".to_string(),
                "pem".to_string(),
                "key".to_string(),
                "pfx".to_string(),
                "p12".to_string(),
                "crt".to_string(),
                "cer".to_string(),
            ]
        );
    }

    #[test]
    fn external_extractors_default_to_off_with_safe_limits() {
        let default = KensakuConfig::default();
        assert!(default.extractors.is_empty(), "no plugins ship by default");

        let extractor: ExternalExtractor =
            serde_json::from_str(r#"{"name":"sample","extensions":[".STL"],"command":"/bin/cat"}"#)
                .expect("minimal extractor should parse");
        assert!(!extractor.enabled, "plugins require explicit opt-in");
        assert_eq!(extractor.timeout_ms, 5_000);
        assert_eq!(extractor.max_output_bytes, 1_000_000);
        assert!(extractor.mime.is_empty());
        assert!(extractor.args.is_empty());

        let enabled: ExternalExtractor = serde_json::from_str(
            r#"{"name":"sample","extensions":["stl"],"mime":["model/*"],"command":"/bin/cat","enabled":true}"#,
        )
        .expect("explicitly enabled extractor should parse");
        assert!(enabled.enabled);
        assert_eq!(enabled.mime, vec!["model/*".to_string()]);
    }

    #[test]
    fn corrupt_config_falls_back_to_defaults() {
        let cfg = KensakuConfig::load_from_str("{ not json");
        assert_eq!(cfg.config_version, 4);
        assert_eq!(cfg.theme, "paper");
    }

    #[test]
    fn save_to_writes_atomically() {
        let dir = temp_dir("save");

        let cfg = KensakuConfig::default();
        cfg.save_to(&dir).expect("save should succeed");

        let path = dir.join("config.json");
        assert!(path.is_file(), "config.json should exist");
        assert!(
            !dir.join("config.json.tmp").exists(),
            "temp file should be renamed away"
        );

        let saved: KensakuConfig =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.config_version, 4);
        assert_eq!(saved.theme, "paper");

        // A second save replaces the existing file atomically.
        let updated = KensakuConfig {
            max_results: 7,
            ..cfg
        };
        updated.save_to(&dir).expect("second save should succeed");
        let saved: KensakuConfig =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.max_results, 7);
        assert!(!dir.join("config.json.tmp").exists());

        fs::remove_dir_all(&dir).ok();
    }
}
