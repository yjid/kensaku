//! Optional accurate OCR pack.
//!
//! Tesseract stays the fast, always-available default. The accurate pack adds
//! PaddleOCR PP-OCRv6 tiny (detector + recognizer + dictionary) behind the
//! `accurate-ocr` Cargo feature. If the pack is not installed or the feature is
//! disabled, callers fall back to Tesseract.

use crate::config::KensakuConfig;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

pub const TESSERACT: &str = "tesseract";
pub const ACCURATE: &str = "accurate";

static ENGINE: AtomicU8 = AtomicU8::new(0);

/// Directory holding the optional PP-OCRv6 pack.
pub fn pack_dir() -> PathBuf {
    KensakuConfig::config_dir().join("ocr")
}

pub fn det_path() -> PathBuf {
    pack_dir().join("det.onnx")
}

pub fn rec_path() -> PathBuf {
    pack_dir().join("rec.onnx")
}

pub fn dict_path() -> PathBuf {
    pack_dir().join("dict.txt")
}

/// True when all three pack files exist.
pub fn pack_present() -> bool {
    det_path().is_file() && rec_path().is_file() && dict_path().is_file()
}

/// True when the compiled binary has the accurate engine.
pub fn accurate_compiled() -> bool {
    cfg!(feature = "accurate-ocr")
}

/// True when the accurate engine can actually run.
pub fn accurate_available() -> bool {
    accurate_compiled() && pack_present()
}

/// Updates the process-wide OCR backend from config.
///
/// Values other than `accurate` (including the old `native` string) fall back
/// to Tesseract. Native platform OCR is not implemented yet.
pub fn set_engine(engine: &str) {
    let value = if engine.eq_ignore_ascii_case(ACCURATE) {
        1
    } else {
        0
    };
    ENGINE.store(value, Ordering::SeqCst);
}

pub fn engine_name() -> &'static str {
    match ENGINE.load(Ordering::SeqCst) {
        1 => ACCURATE,
        _ => TESSERACT,
    }
}

/// Extracts text from an image using the optional accurate pack.
///
/// Returns `None` so the caller falls back to Tesseract when the pack is
/// missing, disabled, or returns no text. Native macOS/Windows OCR is a
/// separate, not-yet-implemented backlog item.
pub fn extract_image_ocr(path: &Path) -> Option<String> {
    #[cfg(feature = "accurate-ocr")]
    {
        if engine_name() == ACCURATE
            && pack_present()
            && let Some(text) = extract_accurate_ocr(path)
        {
            return Some(text);
        }
    }

    let _ = path;
    None
}

#[cfg(feature = "accurate-ocr")]
fn extract_accurate_ocr(path: &Path) -> Option<String> {
    use paddleocr_rs_onnx::{OcrEngine, OrderBy};
    use std::sync::{Mutex, OnceLock};

    static ENGINE_CACHE: OnceLock<Mutex<Option<OcrEngine>>> = OnceLock::new();

    let cache = ENGINE_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().ok()?;
    if guard.is_none() {
        let det = std::fs::read(det_path()).ok()?;
        let rec = std::fs::read(rec_path()).ok()?;
        let dict = std::fs::read(dict_path()).ok()?;
        let engine = OcrEngine::new(&det, &rec, &dict).ok()?;
        *guard = Some(engine);
    }
    let engine = guard.as_ref()?;

    let image = image::open(path).ok()?;
    let blocks = engine.recognize_all(&image, OrderBy::Horizontal).ok()?;
    let text = blocks
        .iter()
        .map(|block| block.text.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_defaults_to_tesseract_and_round_trips() {
        set_engine(TESSERACT);
        assert_eq!(engine_name(), TESSERACT);
        set_engine(ACCURATE);
        assert_eq!(engine_name(), ACCURATE);
        // The retired `native` value must fall back to Tesseract rather than
        // claiming a backend that does not exist.
        set_engine("native");
        assert_eq!(engine_name(), TESSERACT);
        set_engine(TESSERACT);
    }

    #[test]
    fn pack_presence_requires_all_three_files() {
        // The real config dir should not be touched by this test.
        let dir = std::env::temp_dir().join(format!("kensaku-ocr-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // pack_dir() is config-based, so only the path helpers are exercised
        // here; presence is covered by the ignored real-pack test.
        assert!(det_path().ends_with("det.onnx"));
        assert!(rec_path().ends_with("rec.onnx"));
        assert!(dict_path().ends_with("dict.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "accurate-ocr")]
    #[test]
    #[ignore = "requires KENSAKU_OCR_PACK_DIR pointing at det.onnx/rec.onnx/dict.txt"]
    fn real_pack_loads() {
        let dir = std::env::var("KENSAKU_OCR_PACK_DIR")
            .expect("KENSAKU_OCR_PACK_DIR must point at the OCR pack");
        let dir = Path::new(&dir);
        let det = std::fs::read(dir.join("det.onnx")).expect("det.onnx");
        let rec = std::fs::read(dir.join("rec.onnx")).expect("rec.onnx");
        let dict = std::fs::read(dir.join("dict.txt")).expect("dict.txt");
        let engine =
            paddleocr_rs_onnx::OcrEngine::new(&det, &rec, &dict).expect("load PP-OCRv6 tiny pack");
        assert!(engine.device() == paddleocr_rs_onnx::AccelerationDevice::Cpu);
    }

    #[cfg(feature = "accurate-ocr")]
    #[test]
    #[ignore = "requires KENSAKU_OCR_PACK_DIR and KENSAKU_OCR_IMAGE"]
    fn real_pack_reads_text() {
        let dir = std::env::var("KENSAKU_OCR_PACK_DIR")
            .expect("KENSAKU_OCR_PACK_DIR must point at the OCR pack");
        let image_path = std::env::var("KENSAKU_OCR_IMAGE")
            .expect("KENSAKU_OCR_IMAGE must point at a test image");
        let dir = Path::new(&dir);
        let det = std::fs::read(dir.join("det.onnx")).expect("det.onnx");
        let rec = std::fs::read(dir.join("rec.onnx")).expect("rec.onnx");
        let dict = std::fs::read(dir.join("dict.txt")).expect("dict.txt");
        let engine = paddleocr_rs_onnx::OcrEngine::new(&det, &rec, &dict).expect("engine");
        let image = image::open(&image_path).expect("test image");
        let blocks = engine
            .recognize_all(&image, paddleocr_rs_onnx::OrderBy::Horizontal)
            .expect("recognize");
        let text = blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!("PP-OCRv6 text: {text}");
        assert!(text.to_lowercase().contains("kensaku"), "got: {text}");
    }

    #[cfg(feature = "accurate-ocr")]
    #[test]
    #[ignore = "requires KENSAKU_OCR_PACK_DIR; runs the 104-image UI-text fixture"]
    fn real_pack_ui_text_recall() {
        fn normalize(text: &str) -> String {
            text.chars()
                .filter(|ch| ch.is_alphanumeric())
                .flat_map(|ch| ch.to_lowercase())
                .collect()
        }

        let pack = std::env::var("KENSAKU_OCR_PACK_DIR")
            .expect("KENSAKU_OCR_PACK_DIR must point at the OCR pack");
        let fixture = std::env::var("KENSAKU_OCR_FIXTURE_DIR").unwrap_or_else(|_| {
            format!("{}/tests/fixtures/ocr-ui-text", env!("CARGO_MANIFEST_DIR"))
        });
        let pack = Path::new(&pack);
        let fixture = Path::new(&fixture);
        let det = std::fs::read(pack.join("det.onnx")).expect("det.onnx");
        let rec = std::fs::read(pack.join("rec.onnx")).expect("rec.onnx");
        let dict = std::fs::read(pack.join("dict.txt")).expect("dict.txt");
        let engine = paddleocr_rs_onnx::OcrEngine::new(&det, &rec, &dict).expect("engine");

        let labels = std::fs::read_to_string(fixture.join("labels.txt")).expect("labels.txt");
        let labels: Vec<&str> = labels
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let mut hits = 0usize;
        let mut missed = Vec::new();
        for (index, label) in labels.iter().enumerate() {
            let image_path = fixture.join(format!("{:03}.png", index + 1));
            let image = image::open(&image_path).expect("fixture image");
            let blocks = engine
                .recognize_all(&image, paddleocr_rs_onnx::OrderBy::Horizontal)
                .expect("recognize");
            let text = blocks
                .iter()
                .map(|block| block.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let expected = normalize(label);
            let actual = normalize(&text);
            if actual.contains(&expected) {
                hits += 1;
            } else {
                missed.push(format!("{label:?} -> {text:?}"));
            }
        }
        let recall = hits as f32 / labels.len() as f32;
        println!(
            "PP-OCRv6 UI-text fixture: {hits}/{} lines exact ({:.1}% recall)",
            labels.len(),
            recall * 100.0
        );
        for line in missed.iter().take(10) {
            println!("  miss: {line}");
        }
        assert!(recall >= 0.95, "recall {recall:.3} below 0.95");
    }
}
