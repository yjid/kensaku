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
            extract_scanned_pdf_ocr(&doc, max_bytes)
                .unwrap_or_else(|| "[Scanned PDF: Image only]".to_string())
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
            eprintln!(
                "[Kensaku PDF] Panic caught while parsing PDF {}",
                path.display()
            );
            None
        }
    }
}

/// Extracts embedded image streams from a PDF and attempts OCR when pages contain no text.
fn extract_scanned_pdf_ocr(doc: &Document, max_bytes: usize) -> Option<String> {
    let mut collected = String::new();
    let mut image_count = 0usize;

    for object in doc.objects.values() {
        if collected.len() >= max_bytes || image_count >= 5 {
            break;
        }
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        let Ok(subtype) = stream.dict.get(b"Subtype").and_then(|o| o.as_name()) else {
            continue;
        };
        if subtype != b"Image" {
            continue;
        }
        let Ok(data) = stream.decompressed_content() else {
            continue;
        };
        let Ok(img) = image::load_from_memory(&data) else {
            continue;
        };
        image_count += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = std::env::temp_dir().join(format!(
            "kensaku-pdf-ocr-{}-{}.png",
            std::process::id(),
            nanos
        ));
        if img.save(&tmp_path).is_ok() {
            if let Some(ocr_text) = crate::indexer::extract::image::extract_image_ocr(&tmp_path) {
                let trimmed = ocr_text.trim();
                if !trimmed.is_empty() {
                    if !collected.is_empty() {
                        collected.push_str("\n\n");
                    }
                    collected.push_str(trimmed);
                }
            }
            let _ = std::fs::remove_file(&tmp_path);
        }
    }

    if collected.trim().is_empty() {
        None
    } else {
        Some(collected)
    }
}
