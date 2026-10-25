use lopdf::Document;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PdfExtractResult {
    pub text: String,
    pub page_count: usize,
    pub is_encrypted: bool,
    pub is_scanned: bool,
}

pub fn extract_pdf(path: &Path, max_bytes: usize) -> Option<PdfExtractResult> {
    let path_buf = path.to_path_buf();
    let res = std::panic::catch_unwind(move || {
        let doc = match Document::load(&path_buf) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[Kensaku PDF] Failed to load {}: {}", path_buf.display(), e);
                return None;
            }
        };

        if doc.is_encrypted() {
            return Some(PdfExtractResult {
                text: "[Encrypted PDF: Password Protected]".to_string(),
                page_count: 0,
                is_encrypted: true,
                is_scanned: false,
            });
        }

        let pages = doc.get_pages();
        let page_count = pages.len();
        let mut full_text = String::new();

        for page_num in pages.keys() {
            if full_text.len() >= max_bytes {
                full_text.push_str("\n... [content truncated at limit]");
                break;
            }
            if let Ok(page_text) = doc.extract_text(&[*page_num]) {
                let trimmed = page_text.trim();
                if !trimmed.is_empty() {
                    if !full_text.is_empty() {
                        full_text.push_str(&format!("\n\n--- Page {} ---\n", page_num));
                    }
                    full_text.push_str(trimmed);
                }
            }
        }

        let is_scanned = page_count > 0 && full_text.trim().len() < 50;
        let text = if is_scanned && full_text.trim().is_empty() {
            "[Scanned PDF: Image only]".to_string()
        } else {
            full_text
        };

        Some(PdfExtractResult {
            text,
            page_count,
            is_encrypted: false,
            is_scanned,
        })
    });

    match res {
        Ok(opt) => opt,
        Err(_) => {
            eprintln!("[Kensaku PDF] Panic caught while parsing PDF {}", path.display());
            None
        }
    }
}
