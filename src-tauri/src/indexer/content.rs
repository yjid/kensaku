use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::collector::{DocSetCollector, TopDocs};
use tantivy::query::{AllQuery, BooleanQuery, Occur, Query, QueryParser, RegexQuery};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

pub struct SnippetCache {
    capacity: usize,
    map: HashMap<PathBuf, (String, String)>, // (original, lowercase)
    order: VecDeque<PathBuf>,
}

impl SnippetCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get_or_read(&mut self, path: &Path) -> Option<(&str, &str)> {
        if self.map.contains_key(path) {
            if let Some(pos) = self.order.iter().position(|p| p == path) {
                self.order.remove(pos);
            }
            self.order.push_back(path.to_path_buf());
            return self
                .map
                .get(path)
                .map(|(orig, lower)| (orig.as_str(), lower.as_str()));
        }

        let content = read_text_content(path, 64_000)?;
        let lower = content.to_lowercase();
        if self.map.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }
        self.order.push_back(path.to_path_buf());
        self.map.insert(path.to_path_buf(), (content, lower));
        self.map
            .get(path)
            .map(|(orig, lower)| (orig.as_str(), lower.as_str()))
    }
}

pub struct ContentIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    pub path_field: Field,
    pub filename_field: Field,
    pub content_field: Field,
    pub modified_field: Field,
    snippet_cache: Mutex<SnippetCache>,
}

#[derive(Debug, Clone)]
pub struct ContentMatch {
    pub path: PathBuf,
    pub filename: String,
    pub snippet: Option<String>,
    pub score: f32,
    pub modified: u64,
}

/// Which text field a BM25 group should be scoped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Search both `filename` and `content`.
    Any,
    /// Search only `filename`.
    Name,
    /// Search only `content`.
    Content,
}

/// Result of a grouped BM25 search that may also include regex clauses.
#[derive(Debug, Clone, Default)]
pub struct ContentSearchOutcome {
    pub matches: Vec<ContentMatch>,
    /// Actionable messages for ignored/invalid regex patterns.
    pub hints: Vec<String>,
}

/// Upper bound on user-supplied regex patterns to avoid pathological cases.
pub const MAX_REGEX_PATTERN_LEN: usize = 1024;

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
        let writer = index.writer(50_000_000).map_err(|e| e.to_string())?;

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
            snippet_cache: Mutex::new(SnippetCache::new(200)),
        })
    }

    pub fn add_or_update(
        &self,
        path: &Path,
        filename: &str,
        content: &str,
        modified: u64,
    ) -> Result<(), String> {
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
    /// `valid_paths`, except documents under `preserved_roots`.
    ///
    /// This is used after a full crawl to remove documents that belong to
    /// files deleted while the application was not running (or to watched
    /// directories removed from the configuration). `preserved_roots` are
    /// configured roots that were unavailable during the crawl (unplugged
    /// drive, unmounted share); their documents are kept so a transient outage
    /// cannot wipe the index. Returns the number of documents purged.
    pub fn purge_stale(
        &self,
        valid_paths: &HashSet<PathBuf>,
        preserved_roots: &[PathBuf],
    ) -> Result<usize, String> {
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
            if valid_lossy.contains(path_str) {
                continue;
            }
            let path = Path::new(path_str);
            if preserved_roots.iter().any(|root| path.starts_with(root)) {
                continue;
            }
            stale_paths.push(path_str.to_string());
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

    pub fn search(
        &self,
        query_str: &str,
        limit: usize,
        path_prefix: Option<&str>,
    ) -> Vec<ContentMatch> {
        self.search_groups(&[(FieldKind::Any, query_str)], limit, path_prefix)
    }

    /// Field-aware BM25 search. Each group is ANDed with the others; within a
    /// group multi-word input narrows (conjunction by default).
    pub fn search_groups(
        &self,
        groups: &[(FieldKind, &str)],
        limit: usize,
        path_prefix: Option<&str>,
    ) -> Vec<ContentMatch> {
        self.search_groups_with_regex(groups, &[], limit, path_prefix)
            .matches
    }

    /// Like [`Self::search_groups`], plus optional `re:` patterns.
    ///
    /// Regex clauses are `Should` clauses and are only added when a pattern was
    /// explicitly supplied by the caller (the query parser only sets the regex
    /// flag for `re:` tokens). Plain term groups and regex clauses are combined
    /// so that at least one of them must match.
    pub fn search_groups_with_regex(
        &self,
        groups: &[(FieldKind, &str)],
        regex_patterns: &[&str],
        limit: usize,
        path_prefix: Option<&str>,
    ) -> ContentSearchOutcome {
        let mut hints = Vec::new();

        let mut plain_clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for &(kind, text) in groups {
            let sanitized = sanitize_query_text(text);
            if sanitized.trim().is_empty() {
                continue;
            }
            let fields = match kind {
                FieldKind::Any => vec![self.filename_field, self.content_field],
                FieldKind::Name => vec![self.filename_field],
                FieldKind::Content => vec![self.content_field],
            };
            let mut parser = QueryParser::for_index(&self.index, fields);
            parser.set_conjunction_by_default();
            // `sanitize_query_text` removes every parser metacharacter, so this
            // should always succeed; skip the group if it somehow does not.
            if let Ok(parsed) = parser.parse_query(&sanitized) {
                plain_clauses.push((Occur::Must, parsed));
            }
        }

        let mut regex_clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for &pattern in regex_patterns {
            if pattern.is_empty() {
                continue;
            }
            if pattern.chars().count() > MAX_REGEX_PATTERN_LEN {
                hints.push(format!(
                    "Regex pattern too long (max {} characters); ignored",
                    MAX_REGEX_PATTERN_LEN
                ));
                continue;
            }

            let mut field_queries: Vec<RegexQuery> = Vec::new();
            let mut failed = false;
            for field in [self.content_field, self.filename_field] {
                match RegexQuery::from_pattern(pattern, field) {
                    Ok(query) => field_queries.push(query),
                    Err(err) => {
                        hints.push(format!("Invalid regex '{}': {}", pattern, err));
                        failed = true;
                        break;
                    }
                }
            }
            if !failed {
                for query in field_queries {
                    regex_clauses.push((Occur::Should, Box::new(query)));
                }
            }
        }

        let plain_query: Option<Box<dyn Query>> = if plain_clauses.is_empty() {
            None
        } else {
            Some(Box::new(BooleanQuery::new(plain_clauses)))
        };
        let regex_query: Option<Box<dyn Query>> = if regex_clauses.is_empty() {
            None
        } else {
            Some(Box::new(BooleanQuery::new(regex_clauses)))
        };

        let query: Option<Box<dyn Query>> = match (plain_query, regex_query) {
            (None, None) => None,
            (Some(plain), None) => Some(plain),
            (None, Some(regex)) => Some(regex),
            (Some(plain), Some(regex)) => Some(Box::new(BooleanQuery::new(vec![
                (Occur::Should, plain),
                (Occur::Should, regex),
            ]))),
        };

        let matches = match query {
            Some(q) => self.run_query(q.as_ref(), limit, path_prefix),
            None => Vec::new(),
        };

        ContentSearchOutcome { matches, hints }
    }

    fn run_query(
        &self,
        query: &dyn Query,
        limit: usize,
        path_prefix: Option<&str>,
    ) -> Vec<ContentMatch> {
        if limit == 0 {
            return Vec::new();
        }

        let searcher = self.reader.searcher();
        let collector = TopDocs::with_limit(limit).order_by_score();
        let top_docs = match searcher.search(query, &collector) {
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

                // Apply path prefix filter (pushed from scope)
                if let Some(prefix) = path_prefix
                    && !path_str.starts_with(prefix)
                {
                    continue;
                }

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
                    results.push(ContentMatch {
                        path,
                        filename,
                        snippet: None,
                        score,
                        modified,
                    });
                }
            }
        }

        results
    }

    pub fn get_snippet(&self, path: &Path, query: &str) -> Option<String> {
        let mut cache = self.snippet_cache.lock();
        let (content, content_lower) = cache.get_or_read(path)?;
        Self::extract_snippet_from_text_cached(content, content_lower, query)
    }

    pub fn extract_snippet_from_text_cached(
        content: &str,
        content_lower: &str,
        query: &str,
    ) -> Option<String> {
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();
        if terms.is_empty() {
            return None;
        }

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
        let cleaned = slice.replace(['\n', '\r'], " ").trim().to_string();

        let prefix = if safe_start > 0 { "..." } else { "" };
        let suffix = if safe_end < content.len() { "..." } else { "" };

        Some(format!("{}{}{}", prefix, cleaned, suffix))
    }

    pub fn extract_snippet_from_text(content: &str, query: &str) -> Option<String> {
        Self::extract_snippet_from_text_cached(content, &content.to_lowercase(), query)
    }
}

pub fn is_indexable_extension(ext_opt: Option<&str>) -> bool {
    let ext = match ext_opt {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    static TEXT_EXTENSIONS: &[&str] = &[
        // Plain text and code
        "txt",
        "md",
        "markdown",
        "rs",
        "ts",
        "tsx",
        "js",
        "jsx",
        "py",
        "go",
        "json",
        "toml",
        "yaml",
        "yml",
        "xml",
        "html",
        "htm",
        "css",
        "scss",
        "c",
        "cpp",
        "cc",
        "cxx",
        "h",
        "hpp",
        "java",
        "kt",
        "sh",
        "bash",
        "zsh",
        "fish",
        "env",
        "cfg",
        "ini",
        "conf",
        "log",
        "csv",
        "sql",
        "rb",
        "php",
        "swift",
        "r",
        "lua",
        "vim",
        "vue",
        "svelte",
        "astro",
        "dockerfile",
        "diff",
        "patch",
        "tex",
        "graphql",
        "proto",
        // Documents & Office (PR 5)
        "pdf",
        "doc",
        "docx",
        "ppt",
        "pptx",
        "xls",
        "xlsx",
        "epub",
        "odt",
        "ods",
        "odp",
        "rtf",
        "pages",
        "key",
        "numbers",
        "png",
        "jpg",
        "jpeg",
        "gif",
        "bmp",
        "webp",
        "tiff",
        "tif",
    ];
    TEXT_EXTENSIONS.contains(&ext.as_str())
}

pub fn is_indexable_file_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "dockerfile"
            | "makefile"
            | "justfile"
            | "cargo.toml"
            | "package.json"
            | ".gitignore"
            | ".env"
            | ".editorconfig"
    )
}

pub fn read_text_content(path: &Path, max_bytes: u64) -> Option<String> {
    crate::indexer::extract::extract_document(path, max_bytes).map(|doc| doc.text)
}

/// Defuses QueryParser metacharacters in user-supplied text.
///
/// Replacing them with spaces preserves the surrounding words while ensuring
/// raw `:`, quotes, `-`, boolean operators, etc. can never abort parsing.
fn sanitize_query_text(text: &str) -> String {
    text.replace(
        [
            ':', '^', '[', ']', '{', '}', '~', '*', '?', '\\', '(', ')', '"', '-', '+', '!', '&',
            '|', '/',
        ],
        " ",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempIndex {
        index: Option<ContentIndex>,
        dir: PathBuf,
    }

    impl Drop for TempIndex {
        fn drop(&mut self) {
            self.index.take();
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn temp_index(tag: &str) -> TempIndex {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "kensaku-content-test-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ));
        let index = ContentIndex::new(&dir).expect("create content index");
        TempIndex {
            index: Some(index),
            dir,
        }
    }

    fn add(index: &ContentIndex, path: &str, filename: &str, content: &str) {
        index
            .add_or_update(Path::new(path), filename, content, 0)
            .expect("add document");
    }

    #[test]
    fn field_scoped_groups_only_hit_their_field() {
        let temp = temp_index("groups");
        let index = temp.index.as_ref().unwrap();
        add(
            index,
            "/tmp/kensaku/a/alpha.rs",
            "alpha.rs",
            "unique content zebra",
        );
        add(
            index,
            "/tmp/kensaku/a/beta.rs",
            "beta.rs",
            "unique content alpha",
        );
        index.commit().unwrap();

        let by_name = index.search_groups(&[(FieldKind::Name, "alpha")], 10, None);
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].filename, "alpha.rs");

        let by_content = index.search_groups(&[(FieldKind::Content, "alpha")], 10, None);
        assert_eq!(by_content.len(), 1);
        assert_eq!(by_content[0].filename, "beta.rs");

        let any = index.search_groups(&[(FieldKind::Any, "alpha")], 10, None);
        assert_eq!(any.len(), 2);
    }

    #[test]
    fn regex_clauses_match_both_fields_and_invalid_patterns_hint() {
        let temp = temp_index("regex");
        let index = temp.index.as_ref().unwrap();
        add(
            index,
            "/tmp/kensaku/r/one.rs",
            "one.rs",
            "the zebra lives here",
        );
        add(index, "/tmp/kensaku/r/two.rs", "two.rs", "nothing to see");
        index.commit().unwrap();

        let by_content = index.search_groups_with_regex(&[], &[".*zebra.*"], 10, None);
        assert!(by_content.hints.is_empty());
        assert_eq!(by_content.matches.len(), 1);
        assert_eq!(by_content.matches[0].filename, "one.rs");

        let by_filename = index.search_groups_with_regex(&[], &[".*two.*"], 10, None);
        assert_eq!(by_filename.matches.len(), 1);
        assert_eq!(by_filename.matches[0].filename, "two.rs");

        let invalid = index.search_groups_with_regex(&[], &["[unterminated"], 10, None);
        assert!(invalid.matches.is_empty());
        assert_eq!(invalid.hints.len(), 1);
        assert!(invalid.hints[0].contains("Invalid regex"));

        let too_long = "a".repeat(MAX_REGEX_PATTERN_LEN + 1);
        let capped = index.search_groups_with_regex(&[], &[too_long.as_str()], 10, None);
        assert!(capped.matches.is_empty());
        assert!(capped.hints.iter().any(|h| h.contains("too long")));
    }
}
