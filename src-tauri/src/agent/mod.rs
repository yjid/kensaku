//! Headless access to a local Kensaku index.
//!
//! This module powers the `kensaku-cli` binary and the MCP server. It only
//! reads the index built by the GUI: there is no in-app AI, no LLM call and no
//! network access anywhere on this path.

pub mod connect;
pub mod http;
pub mod mcp;
pub mod symbols;

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
    pub visual: crate::embed::visual::VisualStore,
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
        crate::ocr::set_engine(&config.ocr_engine);
        crate::indexer::extract::set_external_extractors(config.extractors.clone());

        // Read-write is fine: WAL lets the GUI writer and this connection
        // coexist. Nothing on the headless path writes.
        let db = MetadataDb::open_in_dir(dir)?;
        let metadata = MetadataStore::with_db(db);

        let model = ModelRegistry::global();
        let mut units = UnitStore::new(model.dim);
        let mut visual = crate::embed::visual::VisualStore::default();
        if let Some(db) = metadata.db() {
            match db.load_all_units() {
                Ok(loaded) => {
                    for (unit, vector) in loaded {
                        units.add_unit(unit, &vector);
                    }
                }
                Err(e) => eprintln!("[Kensaku] Failed to load semantic units: {}", e),
            }
            let _ = db.load_visual_units_into(&mut visual);
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
            visual,
            model,
            dir: dir.to_path_buf(),
        })
    }

    /// Runs a query against the loaded index.
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResponse, String> {
        self.search_page(query, limit, 0)
    }

    /// Runs a query with a stable result offset (MCP pagination).
    pub fn search_page(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, String> {
        if let Ok(value) = crate::daemon::request(
            "search",
            serde_json::json!({
                "query": query,
                "limit": limit,
                "offset": offset,
                "snippets": true
            }),
        ) && let Ok(response) = serde_json::from_value::<SearchResponse>(value)
        {
            return Ok(response);
        }
        self.search_local_page_with_snippets(query, limit, offset, true)
    }

    pub fn search_local(&self, query: &str, limit: usize) -> Result<SearchResponse, String> {
        self.search_local_page_with_snippets(query, limit, 0, true)
    }

    /// No-snippet local path used by MCP, which batches its own extraction.
    pub fn search_local_page(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, String> {
        self.search_local_page_with_snippets(query, limit, offset, false)
    }

    pub fn search_local_page_with_snippets(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
        with_snippets: bool,
    ) -> Result<SearchResponse, String> {
        let aliases = self.config.alias_pairs();
        Ok(SearchDispatcher::search_response_fast_sorted_with_snippets(
            query,
            &self.metadata,
            &self.content,
            Some(&self.units),
            self.config.semantic_enabled,
            limit,
            0,
            crate::search::SortOrder::Best,
            &aliases,
            offset,
            with_snippets,
        ))
    }

    /// Computes snippets for `paths` in parallel on the extraction-free fast
    /// path (plain text is read directly, documents only hit the extraction
    /// cache). The result is aligned with `paths`.
    pub fn snippets(&self, paths: &[String], query: &str) -> Vec<Option<String>> {
        snippets_for_paths(paths, query)
    }

    /// Looks up visually similar images in the local visual store.
    pub fn find_similar_images_local(
        &self,
        path: &str,
        limit: usize,
    ) -> Result<Vec<crate::embed::visual::SimilarImageResult>, String> {
        let p = Path::new(path);
        let (source_node_id, query_vec) = if let Some(entry) = self.metadata.get(p) {
            let vec_opt = self.visual.get_vector(entry.id).map(|v| v.to_vec());
            (Some(entry.id), vec_opt)
        } else {
            (None, None)
        };

        let query_vec = match query_vec {
            Some(v) => v,
            None => {
                let thumbs_dir = self.dir.join("thumbs");
                let thumb = crate::indexer::extract::image::thumbnail_path(p, &thumbs_dir);
                let encoder = crate::embed::visual::MockVisualEncoder;
                use crate::embed::visual::VisualEncoder;
                if thumb.is_file()
                    && let Some(v) = encoder.embed_image_from_thumbnail(&thumb)
                {
                    v
                } else if p.is_file() {
                    let img = image::open(p)
                        .map_err(|e| format!("Failed to open image {}: {}", p.display(), e))?;
                    encoder.embed_image(&img)
                } else {
                    return Err(format!("Image not found or not readable: {}", p.display()));
                }
            }
        };

        let floor = 0.30f32;
        let hits = self
            .visual
            .find_similar(&query_vec, source_node_id, floor, limit);

        let mut results = Vec::new();
        for (node_id, similarity) in hits {
            if let Some(entry) = self.metadata.get_by_node_id(node_id) {
                let is_image = entry
                    .path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(crate::indexer::extract::image::is_supported_image_extension);
                if is_image {
                    results.push(crate::embed::visual::SimilarImageResult {
                        path: entry.path.to_string_lossy().to_string(),
                        filename: entry.name,
                        similarity,
                        file_size: entry.size,
                        modified: entry.modified,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Health snapshot without a daemon: the local stores were opened at
    /// startup, so this is cheap.
    pub fn health_local(&self) -> Value {
        let state = match self.state_local() {
            Ok(state) => state,
            Err(error) => {
                return json!({
                    "ok": false,
                    "daemon": false,
                    "db": false,
                    "index": false,
                    "model": false,
                    "error": error,
                });
            }
        };
        json!({
            "ok": true,
            "daemon": false,
            "protocol_version": null,
            "watcher_ok": false,
            "db": self.dir.join("metadata.db").exists(),
            "index": self.content.is_some(),
            "model": state["active_model_id"].as_u64().is_some(),
            "state": state,
        })
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

        let model_dir = self.dir.join("models");
        let model_real = model_dir.join("model.safetensors").is_file()
            && model_dir.join("tokenizer.json").is_file();

        Ok(json!({
            "files": files,
            "dirs": dirs,
            "content_docs": content_docs,
            "semantic_units": self.units.len() as u64,
            "index_bytes": index_bytes,
            "model_id": units_model_id,
            "active_model_id": self.model.model_id,
            "model_name": self.model.name,
            "model_real": model_real,
            "roots": roots,
            "last_reconcile": last_reconcile,
        }))
    }
}

/// Computes extraction-free snippets for a page of results.
pub fn snippets_for_paths(paths: &[String], query: &str) -> Vec<Option<String>> {
    use rayon::prelude::*;
    paths
        .par_iter()
        .map(|path| crate::indexer::content::snippet_for_path(Path::new(path), query))
        .collect()
}

/// Backend selected for one MCP session.
///
/// `Daemon` never opens SQLite, Tantivy or the embedding table in this
/// process; tool calls are forwarded to the single writer. `Local` is the
/// read-only fallback used when no daemon is running. `None` still answers
/// `initialize` and returns actionable errors from tools.
pub enum AgentSession<'a> {
    Daemon,
    Local(&'a HeadlessIndex),
    None,
}

impl AgentSession<'_> {
    pub fn is_daemon(&self) -> bool {
        matches!(self, Self::Daemon)
    }

    /// Configured agent-scope roots, fetched once per call. Empty means the
    /// whole index is visible to agents.
    pub fn scoped_roots(&self) -> Vec<String> {
        match self {
            Self::Daemon => crate::daemon::request("get_config", json!({}))
                .ok()
                .and_then(|value| serde_json::from_value::<KensakuConfig>(value).ok())
                .map(|config| config.agent_scoped_roots)
                .unwrap_or_default(),
            Self::Local(index) => index.config.agent_scoped_roots.clone(),
            Self::None => Vec::new(),
        }
    }

    pub fn search_page(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, String> {
        match self {
            Self::Daemon => {
                let value = crate::daemon::request(
                    "search",
                    json!({
                        "query": query,
                        "limit": limit,
                        "offset": offset,
                        "snippets": false
                    }),
                )?;
                serde_json::from_value(value).map_err(|e| e.to_string())
            }
            Self::Local(index) => index.search_local_page(query, limit, offset),
            Self::None => Err(NO_INDEX_MESSAGE.to_string()),
        }
    }

    /// Keyset-paginated exhaustive search: matches strictly after
    /// `start_after` (a path), for pagination past the 10k exhaustive cap.
    pub fn search_page_after(
        &self,
        query: &str,
        limit: usize,
        start_after: &str,
    ) -> Result<SearchResponse, String> {
        match self {
            Self::Daemon => {
                let value = crate::daemon::request(
                    "search",
                    json!({
                        "query": query,
                        "limit": limit,
                        "offset": 0,
                        "snippets": false,
                        "start_after": start_after,
                        "mode": "exhaustive",
                    }),
                )?;
                serde_json::from_value(value).map_err(|e| e.to_string())
            }
            Self::Local(index) => {
                // Callers pass a composed query that already carries
                // `mode:exhaustive`; the keyset filter does the rest.
                Ok(SearchDispatcher::search_response_exhaustive_after(
                    query.trim(),
                    &index.metadata,
                    &index.content,
                    Some(&index.units),
                    index.config.semantic_enabled,
                    limit,
                    0,
                    &index.config.alias_pairs(),
                    start_after,
                    false,
                ))
            }
            Self::None => Err(NO_INDEX_MESSAGE.to_string()),
        }
    }

    pub fn state(&self) -> Result<Value, String> {
        match self {
            Self::Daemon => crate::daemon::request("index_state", json!({})),
            Self::Local(index) => index.state_local(),
            Self::None => Err(NO_INDEX_MESSAGE.to_string()),
        }
    }

    pub fn health(&self) -> Value {
        match self {
            Self::Daemon => crate::daemon::request("health", json!({})).unwrap_or_else(|error| {
                json!({
                    "ok": false,
                    "daemon": true,
                    "db": false,
                    "index": false,
                    "model": false,
                    "error": error,
                })
            }),
            Self::Local(index) => index.health_local(),
            Self::None => json!({
                "ok": false,
                "daemon": false,
                "db": false,
                "index": false,
                "model": false,
                "error": NO_INDEX_MESSAGE,
            }),
        }
    }

    pub fn snippets(&self, paths: &[String], query: &str) -> Vec<Option<String>> {
        snippets_for_paths(paths, query)
    }

    /// Looks up visually similar images.
    pub fn find_similar_images(
        &self,
        path: &str,
        limit: usize,
    ) -> Result<Vec<crate::embed::visual::SimilarImageResult>, String> {
        match self {
            Self::Daemon => {
                let value = crate::daemon::request(
                    "find_similar_images",
                    json!({
                        "path": path,
                        "limit": limit,
                    }),
                )?;
                serde_json::from_value(value).map_err(|e| e.to_string())
            }
            Self::Local(index) => index.find_similar_images_local(path, limit),
            Self::None => Err(NO_INDEX_MESSAGE.to_string()),
        }
    }

    /// `reindex` is the only write tool. It requires both the config opt-in
    /// and a running daemon, so read-only fallback can never write.
    pub fn reindex(&self) -> Result<Value, String> {
        match self {
            Self::Daemon => crate::daemon::request("reindex", json!({})),
            Self::Local(_) => Err(
                "reindex requires the Kensaku daemon; start it before enabling this tool"
                    .to_string(),
            ),
            Self::None => Err(NO_INDEX_MESSAGE.to_string()),
        }
    }
}

/// Applies the configured agent-scope roots to an optional requested path.
///
/// - No roots configured: pass the request through unchanged.
/// - Roots configured: a requested path must live under one of them (else
///   the request is rejected); an absent path defaults to the union of the
///   roots.
///
/// Returns the scope paths to append as `in:"..."` tokens, or an error for
/// out-of-scope requests.
pub fn agent_scope_paths(
    roots: &[String],
    requested: Option<&str>,
) -> Result<Vec<String>, String> {
    if roots.is_empty() {
        return Ok(requested
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default());
    }

    let requested = requested.map(str::trim).filter(|path| !path.is_empty());
    if let Some(path) = requested {
        let normalized = normalize_scope_path(path);
        for root in roots {
            let root_norm = normalize_scope_path(root);
            if normalized == root_norm
                || normalized.starts_with(&format!("{}/", root_norm.trim_end_matches('/')))
            {
                return Ok(vec![path.to_string()]);
            }
        }
        return Err(format!(
            "path is outside the configured agent scope: {}",
            path
        ));
    }
    Ok(roots.to_vec())
}

/// Expands a leading `~` so scope comparisons work for `~/Projects` style
/// roots; everything else is compared as given.
fn normalize_scope_path(path: &str) -> String {
    let path = path.trim();
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) =
            std::env::var_os("HOME").or_else(|| dirs::home_dir().map(|p| p.into_os_string()))
    {
        let mut expanded = home.to_string_lossy().into_owned();
        if !expanded.ends_with('/') && !rest.starts_with('/') {
            expanded.push('/');
        }
        expanded.push_str(rest);
        return expanded.trim_end_matches('/').to_string();
    }
    path.trim_end_matches('/').to_string()
}

/// True when `path` is within the configured agent-scope roots (or no roots
/// are configured). Used to gate `read_snippet`-style single-path reads.
pub fn agent_path_allowed(roots: &[String], path: &str) -> bool {
    if roots.is_empty() {
        return true;
    }
    let normalized = normalize_scope_path(path);
    roots.iter().any(|root| {
        let root_norm = normalize_scope_path(root);
        normalized == root_norm
            || normalized.starts_with(&format!("{}/", root_norm.trim_end_matches('/')))
    })
}

/// Composes a Kensaku query from a base query plus one or more scope paths.
///
/// Like [`compose_query`], but every scope path becomes an `in:"..."` token
/// (multiple include scopes are OR'd by the parser — the agent-scope union).
pub fn compose_query_scoped(
    base: &str,
    scope_paths: &[String],
    ext: Option<&str>,
    modified_after: Option<&str>,
    exhaustive: bool,
) -> String {
    let (first, rest) = match scope_paths.split_first() {
        Some((first, rest)) => (Some(first.as_str()), rest),
        None => (None, &[][..]),
    };
    let mut composed = compose_query(base, first, ext, modified_after, exhaustive);
    for path in rest {
        if !composed.is_empty() {
            composed.push(' ');
        }
        composed.push_str(&format!("in:\"{}\"", escape_query_value(path)));
    }
    composed
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

/// Random-enough token for a loopback-only local socket or HTTP endpoint.
///
/// Not cryptographic: the threat model is other local processes, and the
/// token file is 0600. Same scheme the daemon uses for its socket token.
pub fn new_connection_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", std::process::id(), nanos)
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
