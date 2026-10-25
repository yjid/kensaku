pub mod office;
pub mod pdf;
pub mod text;

use std::fs::File;
use std::io::Read;
use std::path::Path;

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
pub fn extract_document(path: &Path, max_bytes: u64) -> Option<DocumentExtract> {
    let meta = std::fs::metadata(path).ok()?;
    let file_size = meta.len();

    // Content policy: metadata-only above 20 MB
    if file_size > 20_000_000 {
        return None;
    }

    let capped_bytes = (max_bytes as usize).min(1_000_000);
    let format = sniff_format(path);

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
