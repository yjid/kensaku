use lopdf::Document;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PdfExtractResult {
    pub text: String,
    pub page_count: usize,
    pub is_encrypted: bool,
    pub is_scanned: bool,
    /// The PDF looked scanned but no OCR engine could run, so the text is a
    /// placeholder that should be replaced once one can.
    pub ocr_unavailable: bool,
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
                ocr_unavailable: false,
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

        // Fewer than 50 characters of real text across the whole document
        // means scanned pages (often with a stamped page number or a scanner
        // banner, which is why this is not "exactly empty").
        let is_scanned = page_count > 0 && full_text.trim().len() < 50;
        let mut ocr_unavailable = false;
        let text = if is_scanned && crate::indexer::extract::ocr_enabled() {
            let (ocr_text, unavailable) = extract_scanned_pdf_ocr(&doc, max_bytes);
            ocr_unavailable = unavailable;
            match ocr_text {
                Some(ocr_text) if full_text.trim().is_empty() => ocr_text,
                Some(ocr_text) => format!("{}\n\n{}", full_text.trim(), ocr_text),
                None if full_text.trim().is_empty() => "[Scanned PDF: Image only]".to_string(),
                None => full_text,
            }
        } else if is_scanned && full_text.trim().is_empty() {
            "[Scanned PDF: Image only]".to_string()
        } else {
            full_text
        };

        Some(PdfExtractResult {
            text,
            page_count,
            is_encrypted: false,
            is_scanned,
            ocr_unavailable,
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

/// Most pages of a scanned PDF that get OCR'd. PP-OCR reads a page in about
/// 0.15 s, so 20 pages fit comfortably in the extraction time budget; Indra
/// takes up to about 0.75 s on a dense page, so 10; Tesseract takes about 1 s
/// a page, so it stays at 6 to avoid timing out (a timed-out file is
/// retried, so too many pages would retry forever).
fn max_ocr_pages() -> usize {
    match crate::ocr::usable_engine() {
        crate::ocr::ACCURATE => 20,
        crate::ocr::INDRA => 10,
        _ => 6,
    }
}

/// OCRs the page images of a scanned PDF, in page order.
///
/// Returns the text (if any) and whether OCR was unavailable, so the caller
/// can retry the file once an engine can run. Undecodable images (JPEG 2000,
/// fax, JBIG2) are logged once per file rather than skipped silently.
fn extract_scanned_pdf_ocr(doc: &Document, max_bytes: usize) -> (Option<String>, bool) {
    use crate::ocr::OcrOutcome;

    let mut skipped: Vec<String> = Vec::new();
    let images = crate::indexer::extract::pdf_image::page_images(doc, max_ocr_pages(), |reason| {
        if skipped.len() < 3 {
            skipped.push(reason);
        }
    });
    if images.is_empty() && !skipped.is_empty() {
        eprintln!(
            "[Kensaku PDF] scanned PDF has no image Kensaku can decode: {}",
            skipped.join("; ")
        );
    }

    let mut collected = String::new();
    let mut unavailable = false;
    for image in &images {
        if collected.len() >= max_bytes {
            break;
        }
        match crate::indexer::extract::image::ocr_decoded_image(image) {
            OcrOutcome::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if !collected.is_empty() {
                        collected.push_str("\n\n");
                    }
                    collected.push_str(trimmed);
                }
            }
            OcrOutcome::NoText => {}
            OcrOutcome::Unavailable => {
                unavailable = true;
                break;
            }
        }
    }

    let text = (!collected.trim().is_empty()).then_some(collected);
    (text, unavailable)
}
