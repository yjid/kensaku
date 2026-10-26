//! Headless access to a local Kensaku index.
//!
//! This module powers the `kensaku-cli` binary and the MCP server. It only
//! reads the index built by the GUI: there is no in-app AI, no LLM call and no
//! network access anywhere on this path.

pub mod mcp;

use crate::config::KensakuConfig;
use crate::embed::{ModelRegistry, UnitStore};
use crate::indexer::content::ContentIndex;
use crate::indexer::metadata::MetadataStore;
use crate::indexer::metadata_db::MetadataDb;
use crate::search::{SearchDispatcher, SearchResponse};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Error returned whenever the GUI has not built an index yet.
pub const NO_INDEX_MESSAGE: &str =
    "No Kensaku index found. Launch Kensaku once to build the index.";

/// A process-lifetime snapshot of the Kensaku data directory.
///
/// Opening is the expensive part (SQLite load + Tantivy reader + embedding
/// table); [`Self::search`] and [`Self::state`] are cheap afterwards. Long-lived
/// transports (MCP) should open once per session.
pub struct HeadlessIndex {
    pub config: KensakuConfig,
    pub metadata: MetadataStore,
    pub content: Option<ContentIndex>,
    pub units: UnitStore,
    pub model: &'static ModelRegistry,
    dir: PathBuf,
}

impl HeadlessIndex {
    /// Opens the user's real Kensaku data directory.
    pub fn open() -> Result<Self, String> {
        let dir = KensakuConfig::config_dir();
        if !dir.join("config.json").exists() && !dir.join("metadata.db").exists() {
            return Err(NO_INDEX_MESSAGE.to_string());
        }
        Self::open_at(&dir)
    }

    /// Opens an explicit data directory. Used by tests and reusable by callers
    /// that keep the index somewhere other than the default config dir.
    pub fn open_at(dir: &Path) -> Result<Self, String> {
        let config_path = dir.join("config.json");
        let db_path = dir.join("metadata.db");

        if !db_path.exists() {
            return Err(NO_INDEX_MESSAGE.to_string());
        }

        let config = if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)
                .map_err(|e| format!("Could not read config {}: {}", config_path.display(), e))?;
            KensakuConfig::load_from_str(&raw)
        } else {
            KensakuConfig::default()
        };

        // The headless surface must honor the same OCR setting as the GUI.
        crate::indexer::extract::set_ocr_enabled(config.ocr_enabled);

        // Read-write is fine: WAL lets the GUI writer and this connection
        // coexist. Nothing on the headless path writes.
        let db = MetadataDb::open_in_dir(dir)?;
        let metadata = MetadataStore::with_db(db);

        let model = ModelRegistry::global();
        let mut units = UnitStore::new(model.dim);
        if let Some(db) = metadata.db() {
            match db.load_all_units() {
                Ok(loaded) => {
                    for (unit, vector) in loaded {
                        units.add_unit(unit, &vector);
                    }
                }
                Err(e) => eprintln!("[Kensaku] Failed to load semantic units: {}", e),
            }
        }

        let index_dir = dir.join("index");
        let content = if index_dir.join("meta.json").exists() {
            Some(ContentIndex::open_read_only(&index_dir)?)
        } else {
            None
        };

        Ok(Self {
            config,
            metadata,
            content,
            units,
            model,
            dir: dir.to_path_buf(),
        })
    }

    /// Runs a query against the loaded index.
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResponse, String> {
        let aliases = self.config.alias_pairs();
        Ok(SearchDispatcher::search_response_with_aliases(
            query,
            &self.metadata,
            &self.content,
            Some(&self.units),
            self.config.semantic_enabled,
            limit,
            0,
            &aliases,
        ))
    }

    /// Computes snippets for `paths` in parallel on the extraction-free fast
    /// path (plain text is read directly, documents only hit the extraction
    /// cache). The result is aligned with `paths`.
    pub fn snippets(&self, paths: &[String], query: &str) -> Vec<Option<String>> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| crate::indexer::content::snippet_for_path(Path::new(path), query))
            .collect()
    }

    /// Compact index status shared by `kensaku-cli state` and the MCP
    /// `index_state` tool.
    pub fn state(&self) -> Result<Value, String> {
        if let Ok(value) = crate::daemon::request("index_state", json!({})) {
            return Ok(value);
        }
        self.state_local()
    }

    pub fn state_local(&self) -> Result<Value, String> {
        let mut files = 0u64;
        let mut dirs = 0u64;
        for i in 0..self.metadata.len() {
            if self.metadata.node(i as u32).is_dir() {
                dirs += 1;
            } else {
                files += 1;
            }
        }

        let content_docs = self.content.as_ref().map(|c| c.num_docs()).unwrap_or(0);
        let index_bytes = dir_size_bytes(&self.dir.join("index"))
            .saturating_add(prefixed_file_bytes(&self.dir, "metadata.db"));
        let last_reconcile = last_modified_secs(&[
            self.dir.join("metadata.db"),
            self.dir.join("index").join("meta.json"),
        ]);
        let roots: Vec<String> = self
            .config
            .watched_dirs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        // The units' model id is what the stored vectors were encoded with;
        // the active id is what a new query is embedded with. They can differ
        // until the GUI re-embeds after a model change.
        let units_model_id = match self.metadata.db() {
            Some(db) => db.units_model_id()?,
            None => None,
        };

        Ok(json!({
            "files": files,
            "dirs": dirs,
            "content_docs": content_docs,
            "semantic_units": self.units.len() as u64,
            "index_bytes": index_bytes,
            "model_id": units_model_id,
            "active_model_id": self.model.model_id,
            "model_name": self.model.name,
            "roots": roots,
            "last_reconcile": last_reconcile,
        }))
    }
}

/// Composes a Kensaku query from a base query plus optional agent filters.
///
/// Paths are quoted so a path containing spaces (or a `:` / `"`) survives the
/// query lexer; embedded quotes and backslashes are escaped.
pub fn compose_query(
    base: &str,
    path: Option<&str>,
    ext: Option<&str>,
    modified_after: Option<&str>,
    exhaustive: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let base = base.trim();
    if !base.is_empty() {
        parts.push(base.to_string());
    }

    if let Some(path) = path.map(str::trim).filter(|p| !p.is_empty()) {
        parts.push(format!("in:\"{}\"", escape_query_value(path)));
    }

    if let Some(ext) = ext.map(str::trim).filter(|e| !e.is_empty()) {
        let ext = ext.trim_start_matches('.');
        if !ext.is_empty() {
            parts.push(format!("ext:{}", ext));
        }
    }

    if let Some(modified) = modified_after.map(str::trim).filter(|m| !m.is_empty()) {
        parts.push(format!("modified:{}", modified));
    }

    if exhaustive {
        parts.push("mode:exhaustive".to_string());
    }

    parts.join(" ")
}

fn escape_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Truncates `text` to at most `max_chars` characters without splitting a
/// UTF-8 code point.
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

/// Unix timestamp (seconds) used for `as_of` / freshness fields.
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn dir_size_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total = total.saturating_add(dir_size_bytes(&entry.path()));
        } else if file_type.is_file() {
            total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }
    total
}

/// Sums files in `dir` whose name starts with `prefix` (e.g. `metadata.db`,
/// `metadata.db-wal`, `metadata.db-shm`).
fn prefixed_file_bytes(dir: &Path, prefix: &str) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(prefix) {
            continue;
        }
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
        }
    }
    total
}

fn last_modified_secs(paths: &[PathBuf]) -> Option<u64> {
    paths
        .iter()
        .filter_map(|path| {
            std::fs::metadata(path)
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        })
        .max()
}
