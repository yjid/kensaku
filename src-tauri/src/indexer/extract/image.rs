use std::path::Path;
use std::process::Command;

/// Extracts text from an image via OCR.
/// On Linux: shells out to tesseract.
/// Returns None if OCR fails or produces no text.
pub fn extract_image_ocr(path: &Path) -> Option<String> {
    // Try tesseract (available on Linux and macOS with Homebrew)
    let output = Command::new("tesseract")
        .arg(path)
        .arg("stdout")
        .arg("--psm")
        .arg("3") // Fully automatic page segmentation
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Get image dimensions using ImageMagick identify.
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    let output = Command::new("identify")
        .arg("-format")
        .arg("%w %h")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 2 {
        let w = parts[0].parse().ok()?;
        let h = parts[1].parse().ok()?;
        Some((w, h))
    } else {
        None
    }
}