use parking_lot::RwLock;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::collector::{DocSetCollector, TopDocs};
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

pub struct ContentIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    pub path_field: Field,
    pub filename_field: Field,
    pub content_field: Field,
    pub modified_field: Field,
}

#[derive(Debug, Clone)]
pub struct ContentMatch {
    pub path: PathBuf,
    pub filename: String,
    pub snippet: Option<String>,
    pub score: f32,
    pub modified: u64,
}

impl ContentIndex {
    pub fn new(index_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(index_dir).map_err(|e| e.to_string())?;

        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let filename_field = schema_builder.add_text_field("filename", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let modified_field = schema_builder.add_u64_field("modified", STORED | FAST);
        let schema = schema_builder.build();

        let index = if index_dir.join("meta.json").exists() {
            Index::open_in_dir(index_dir).map_err(|e| e.to_string())?
        } else {
            Index::create_in_dir(index_dir, schema.clone()).map_err(|e| e.to_string())?
        };

        // 50MB writer buffer
        let writer = index
            .writer(50_000_000)
            .map_err(|e| e.to_string())?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
            path_field,
            filename_field,
            content_field,
            modified_field,
        })
    }

    pub fn add_or_update(&self, path: &Path, filename: &str, content: &str, modified: u64) -> Result<(), String> {
        let path_str = path.to_string_lossy().to_string();
        let term = Term::from_field_text(self.path_field, &path_str);
        
        let writer = self.writer.write();
        writer.delete_term(term);

        let doc = doc!(
            self.path_field => path_str,
            self.filename_field => filename.to_string(),
            self.content_field => content.to_string(),
            self.modified_field => modified,
        );
        writer.add_document(doc).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<(), String> {
        let path_str = path.to_string_lossy().to_string();
        let term = Term::from_field_text(self.path_field, &path_str);
        let writer = self.writer.write();
        writer.delete_term(term);
        Ok(())
    }

    /// Deletes every indexed document whose path is not present in
    /// `valid_paths`.
    ///
    /// This is used after a full crawl to remove documents that belong to
    /// files deleted while the application was not running (or to watched
    /// directories removed from the configuration). Returns the number of
    /// documents purged.
    pub fn purge_stale(&self, valid_paths: &HashSet<PathBuf>) -> Result<usize, String> {
        // Documents store `to_string_lossy()` paths, so compare in the same
        // representation to avoid purging valid non-UTF-8 paths.
        let valid_lossy: HashSet<String> = valid_paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();

        let searcher = self.reader.searcher();
        let doc_addresses = searcher
            .search(&AllQuery, &DocSetCollector)
            .map_err(|e| format!("Failed to scan content index: {}", e))?;

        let mut stale_paths: Vec<String> = Vec::new();
        for doc_address in doc_addresses {
            let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) else {
                continue;
            };
            let Some(path_str) = doc.get_first(self.path_field).and_then(|f| f.as_str()) else {
                continue;
            };
            if !valid_lossy.contains(path_str) {
                stale_paths.push(path_str.to_string());
            }
        }

        if stale_paths.is_empty() {
            return Ok(0);
        }

        {
            let writer = self.writer.write();
            for path_str in &stale_paths {
                let term = Term::from_field_text(self.path_field, path_str);
                writer.delete_term(term);
            }
        }

        self.commit()?;
        Ok(stale_paths.len())
    }

    pub fn commit(&self) -> Result<(), String> {
        let mut writer = self.writer.write();
        writer.commit().map_err(|e| e.to_string())?;
        self.reader.reload().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Vec<ContentMatch> {
        if query_str.trim().is_empty() {
            return Vec::new();
        }

        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field, self.filename_field]);
        
        // Escape query if syntax error
        let parsed_query = match query_parser.parse_query(query_str) {
            Ok(q) => q,
            Err(_) => {
                let sanitized = query_str.replace([':', '^', '[', ']', '{', '}', '~', '*', '?', '\\', '(', ')', '"', '-', '+', '!'], " ");
                match query_parser.parse_query(&sanitized) {
                    Ok(q) => q,
                    Err(_) => return Vec::new(),
                }
            }
        };

        let collector = TopDocs::with_limit(limit).order_by_score();
        let top_docs = match searcher.search(&*parsed_query, &collector) {
            Ok(docs) => docs,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();

        for (score, doc_address) in top_docs {
            if let Ok(retrieved_doc) = searcher.doc::<TantivyDocument>(doc_address) {
                let path_str = retrieved_doc
                    .get_first(self.path_field)
                    .and_then(|f| f.as_str())
                    .unwrap_or("")
                    .to_string();

                let filename = retrieved_doc
                    .get_first(self.filename_field)
                    .and_then(|f| f.as_str())
                    .unwrap_or("")
                    .to_string();

                let modified = retrieved_doc
                    .get_first(self.modified_field)
                    .and_then(|f| f.as_u64())
                    .unwrap_or(0);

                if !path_str.is_empty() {
                    let path = PathBuf::from(&path_str);
                    let snippet = Self::extract_snippet_from_file(&path, query_str);

                    results.push(ContentMatch {
                        path,
                        filename,
                        snippet,
                        score,
                        modified,
                    });
                }
            }
        }

        results
    }

    fn extract_snippet_from_file(path: &Path, query: &str) -> Option<String> {
        let content = read_text_content(path, 50_000)?;
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();
        if terms.is_empty() {
            return None;
        }

        let content_lower = content.to_lowercase();
        let mut first_match_idx = None;

        for term in &terms {
            if let Some(pos) = content_lower.find(term) {
                first_match_idx = match first_match_idx {
                    Some(cur) => Some(std::cmp::min(cur, pos)),
                    None => Some(pos),
                };
            }
        }

        let match_idx = first_match_idx?;
        let start = match_idx.saturating_sub(40);
        let end = std::cmp::min(content.len(), match_idx + 80);

        let mut safe_start = start;
        while !content.is_char_boundary(safe_start) && safe_start > 0 {
            safe_start -= 1;
        }

        let mut safe_end = end;
        while !content.is_char_boundary(safe_end) && safe_end < content.len() {
            safe_end += 1;
        }

        let slice = &content[safe_start..safe_end];
        let cleaned = slice.replace('\n', " ").replace('\r', " ").trim().to_string();

        let prefix = if safe_start > 0 { "..." } else { "" };
        let suffix = if safe_end < content.len() { "..." } else { "" };

        Some(format!("{}{}{}", prefix, cleaned, suffix))
    }
}

pub fn is_indexable_extension(ext_opt: Option<&str>) -> bool {
    let ext = match ext_opt {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    static TEXT_EXTENSIONS: &[&str] = &[
        "txt", "md", "markdown", "rs", "ts", "tsx", "js", "jsx", "py", "go",
        "json", "toml", "yaml", "yml", "xml", "html", "htm", "css", "scss",
        "c", "cpp", "cc", "cxx", "h", "hpp", "java", "kt", "sh", "bash",
        "zsh", "fish", "env", "cfg", "ini", "conf", "log", "csv", "sql",
        "rb", "php", "swift", "r", "lua", "vim", "vue", "svelte", "astro",
        "dockerfile", "diff", "patch", "tex", "graphql", "proto",
    ];

    TEXT_EXTENSIONS.contains(&ext.as_str())
}

pub fn is_indexable_file_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "dockerfile" | "makefile" | "justfile" | "cargo.toml" | "package.json" | ".gitignore" | ".env" | ".editorconfig"
    )
}

pub fn read_text_content(path: &Path, max_bytes: u64) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut handle = file.take(max_bytes);
    let mut buffer = Vec::new();
    handle.read_to_end(&mut buffer).ok()?;

    if buffer.iter().take(1024).any(|&b| b == 0) {
        return None;
    }

    String::from_utf8(buffer).ok()
}
