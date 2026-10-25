pub mod image;
pub mod office;
pub mod pdf;
pub mod text;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DocumentExtract {
    pub text: String,
    pub page_count: Option<usize>,
    pub is_encrypted: bool,
    pub is_scanned: bool,
    pub format_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Pptx,
    Xlsx,
    Epub,
    Odt,
    Ods,
    Odp,
    Rtf,
    PlainText,
    Image,
    Unknown,
}

/// Sniffs the format using magic bytes first, falling back to file extension.
pub fn sniff_format(path: &Path) -> DocumentFormat {
    let mut header = [0u8; 16];
    if let Ok(mut f) = File::open(path)
        && let Ok(n) = f.read(&mut header)
        && n >= 4
    {
        // PDF magic: %PDF-
        if header.starts_with(b"%PDF-") {
            return DocumentFormat::Pdf;
        }
        // RTF magic: {\rtf
        if header.starts_with(b"{\\rtf") {
            return DocumentFormat::Rtf;
        }
        // Image magic bytes
        if header.starts_with(b"\x89PNG") || header.starts_with(b"\xff\xd8\xff")
            || header.starts_with(b"GIF8") || header.starts_with(b"RIFF")
            || header.starts_with(b"BM")
        {
            return DocumentFormat::Image;
        }
        // ZIP magic: PK\x03\x04
        if header.starts_with(b"PK\x03\x04") {
            // Differentiate by extension
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            return match ext.as_str() {
                "docx" => DocumentFormat::Docx,
                "pptx" => DocumentFormat::Pptx,
                "xlsx" => DocumentFormat::Xlsx,
                "epub" => DocumentFormat::Epub,
                "odt" => DocumentFormat::Odt,
                "ods" => DocumentFormat::Ods,
                "odp" => DocumentFormat::Odp,
                _ => DocumentFormat::Docx, // Default zip-office
            };
        }
    }

    // Fallback to extension
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "pdf" => DocumentFormat::Pdf,
        "docx" => DocumentFormat::Docx,
        "pptx" => DocumentFormat::Pptx,
        "xlsx" => DocumentFormat::Xlsx,
        "epub" => DocumentFormat::Epub,
        "odt" => DocumentFormat::Odt,
        "ods" => DocumentFormat::Ods,
        "odp" => DocumentFormat::Odp,
        "rtf" => DocumentFormat::Rtf,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "heic" | "heif" => DocumentFormat::Image,
        "txt" | "md" | "markdown" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "json"
        | "toml" | "yaml" | "yml" | "xml" | "html" | "htm" | "css" | "c" | "cpp" | "h"
        | "java" | "sh" | "log" | "csv" | "sql" => DocumentFormat::PlainText,
        _ => DocumentFormat::Unknown,
    }
}

/// Extracts text content and metadata from documents according to ARCHITECTURE §5.2 content policy:
/// - Max file size: 20MB (files above 20MB are metadata-only).
/// - Extracted content capped at max_bytes (default 1MB).
/// - Panic-isolated with std::panic::catch_unwind.
/// Global LRU cache for extraction results, keyed by (path, size, mtime).
static EXTRACTION_CACHE: std::sync::OnceLock<Mutex<ExtractionCache>> = std::sync::OnceLock::new();

struct ExtractionCache {
    capacity: usize,
    map: HashMap<(std::path::PathBuf, u64, u64), DocumentExtract>,
    order: VecDeque<(std::path::PathBuf, u64, u64)>,
}

impl ExtractionCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &(std::path::PathBuf, u64, u64)) -> Option<DocumentExtract> {
        if let Some(val) = self.map.get(key) {
            // Move to back (most recently used)
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
            }
            self.order.push_back(key.clone());
            return Some(val.clone());
        }
        None
    }

    fn insert(&mut self, key: (std::path::PathBuf, u64, u64), val: DocumentExtract) {
        if self.map.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.map.insert(key, val);
    }
}

fn get_extraction_cache() -> &'static Mutex<ExtractionCache> {
    EXTRACTION_CACHE.get_or_init(|| Mutex::new(ExtractionCache::new(64)))
}

/// Upper bound for a single document extraction. A malformed PDF (or a stalled
/// network/FUSE filesystem) must not block indexing forever.
const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(8);

/// Dedicated pool for extraction work.
///
/// A hung extraction permanently loses one worker, but the pool can never grow
/// past four threads, so a pile of pathological documents cannot spawn an
/// unbounded number of detached threads.
static EXTRACTION_POOL: std::sync::OnceLock<Option<rayon::ThreadPool>> = std::sync::OnceLock::new();

fn extraction_pool() -> Option<&'static rayon::ThreadPool> {
    EXTRACTION_POOL
        .get_or_init(|| match rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .thread_name(|index| format!("kensaku-extract-{}", index))
            .build()
        {
            Ok(pool) => Some(pool),
            Err(e) => {
                eprintln!(
                    "[Kensaku] Could not create extraction thread pool ({}); extraction runs inline without a timeout",
                    e
                );
                None
            }
        })
        .as_ref()
}

/// Paths that already produced a timeout warning, so a permanently stuck
/// document is reported once instead of on every crawl.
static TIMEOUT_WARNED: std::sync::OnceLock<Mutex<HashSet<PathBuf>>> = std::sync::OnceLock::new();

fn warn_extraction_timeout(path: &Path) {
    let warned = TIMEOUT_WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut warned = warned.lock().unwrap();
    // Keep the bookkeeping bounded on systems with many broken documents.
    if warned.len() >= 4096 {
        warned.clear();
    }
    if warned.insert(path.to_path_buf()) {
        eprintln!(
            "[Kensaku] Text extraction timed out after {}s; leaving pending: {}",
            EXTRACTION_TIMEOUT.as_secs(),
            path.display()
        );
    }
}

pub fn extract_document(path: &Path, max_bytes: u64) -> Option<DocumentExtract> {
    let meta = std::fs::metadata(path).ok()?;
    let file_size = meta.len();
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();

    // Check cache first
    let cache_key = (path.to_path_buf(), file_size, mtime);
    {
        let mut cache = get_extraction_cache().lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            return Some(cached);
        }
    }

    // Content policy: metadata-only above 20 MB
    if file_size > 20_000_000 {
        return None;
    }

    let capped_bytes = (max_bytes as usize).min(1_000_000);
    let format = sniff_format(path);

    // Run the extraction on the dedicated pool and time it out. On timeout the
    // file stays pending (or skipped) and is retried on a later crawl; nothing
    // is cached, so a transient stall is not sticky.
    let result = match extraction_pool() {
        Some(pool) => {
            let (tx, rx) = std::sync::mpsc::channel();
            let worker_path = path.to_path_buf();
            pool.spawn(move || {
                // Catch panics here: a panic escaping a `rayon::spawn` task
                // reaches rayon's panic handler (which aborts by default), and
                // a malformed document must never take the app down.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    extract_document_inner(&worker_path, capped_bytes, format, file_size)
                }))
                .unwrap_or_else(|_| {
                    eprintln!(
                        "[Kensaku] Text extraction panicked; leaving pending: {}",
                        worker_path.display()
                    );
                    None
                });
                // Ignore send errors: the caller may have timed out already.
                let _ = tx.send(result);
            });

            match rx.recv_timeout(EXTRACTION_TIMEOUT) {
                Ok(result) => result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    warn_extraction_timeout(path);
                    None
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!(
                        "[Kensaku] Text extraction worker exited without a result; leaving pending: {}",
                        path.display()
                    );
                    None
                }
            }
        }
        // Pool creation failed (rare); fall back to inline extraction.
        None => extract_document_inner(path, capped_bytes, format, file_size),
    };

    // Cache the result
    if let Some(ref res) = result {
        let mut cache = get_extraction_cache().lock().unwrap();
        cache.insert(cache_key, res.clone());
    }

    result
}

fn extract_document_inner(path: &Path, capped_bytes: usize, format: DocumentFormat, file_size: u64) -> Option<DocumentExtract> {
    match format {
        DocumentFormat::Pdf => {
            let res = pdf::extract_pdf(path, capped_bytes)?;
            Some(DocumentExtract {
                text: res.text,
                page_count: Some(res.page_count),
                is_encrypted: res.is_encrypted,
                is_scanned: res.is_scanned,
                format_name: "PDF".to_string(),
            })
        }
        DocumentFormat::Docx => {
            let text = office::extract_docx(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "Word Document".to_string(),
            })
        }
        DocumentFormat::Pptx => {
            let text = office::extract_pptx(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "Presentation".to_string(),
            })
        }
        DocumentFormat::Xlsx => {
            let text = office::extract_xlsx(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "Spreadsheet".to_string(),
            })
        }
        DocumentFormat::Epub => {
            let text = office::extract_epub(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "EPUB eBook".to_string(),
            })
        }
        DocumentFormat::Odt | DocumentFormat::Ods | DocumentFormat::Odp => {
            let text = office::extract_odt(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "OpenDocument".to_string(),
            })
        }
        DocumentFormat::Rtf => {
            let text = text::extract_rtf(path, capped_bytes)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: false,
                format_name: "Rich Text".to_string(),
            })
        }
        DocumentFormat::Image => {
            // OCR via tesseract (Linux) or platform APIs
            let text = image::extract_image_ocr(path)?;
            Some(DocumentExtract {
                text,
                page_count: None,
                is_encrypted: false,
                is_scanned: true,
                format_name: "Image (OCR)".to_string(),
            })
        }
        DocumentFormat::PlainText | DocumentFormat::Unknown => {
            // Head/tail policy for plain text > 1MB:
            if file_size > 1_000_000 {
                // Read head 512KB + tail 512KB
                let mut f = File::open(path).ok()?;
                let mut head = vec![0u8; 512_000];
                let n_head = f.read(&mut head).unwrap_or(0);
                head.truncate(n_head);

                use std::io::Seek;
                let _ = f.seek(std::io::SeekFrom::End(-512_000));
                let mut tail = vec![0u8; 512_000];
                let n_tail = f.read(&mut tail).unwrap_or(0);
                tail.truncate(n_tail);

                let head_str = String::from_utf8_lossy(&head);
                let tail_str = String::from_utf8_lossy(&tail);
                let combined = format!("{}\n\n... [content omitted] ...\n\n{}", head_str, tail_str);

                Some(DocumentExtract {
                    text: combined,
                    page_count: None,
                    is_encrypted: false,
                    is_scanned: false,
                    format_name: "Text".to_string(),
                })
            } else {
                let text = text::extract_text(path, capped_bytes)?;
                Some(DocumentExtract {
                    text,
                    page_count: None,
                    is_encrypted: false,
                    is_scanned: false,
                    format_name: "Text".to_string(),
                })
            }
        }
    }
}
