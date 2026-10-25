use image::{DynamicImage, ImageReader, Limits};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Upper bound for a single decodable image (50 megapixels).
///
/// Anything larger is rejected before the decoder allocates the pixel buffer,
/// so a malformed or malicious image cannot exhaust memory.
pub const MAX_IMAGE_PIXELS: u64 = 50_000_000;

/// Value used to disambiguate temporary thumbnail files.
static THUMB_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Whether the extension is one of the raster formats we can decode cheaply.
///
/// HEIC/HEIF and SVG are deliberately excluded: the former is not supported by
/// the `image` crate without a platform decoder, the latter is not a raster
/// format.
pub fn is_supported_image_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif"
    )
}

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
    if text.is_empty() { None } else { Some(text) }
}

/// Reads the image dimensions without decoding the pixel data.
///
/// Returns `None` for unreadable/corrupt images and for images above the
/// [`MAX_IMAGE_PIXELS`] budget.
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let (width, height) = image::image_dimensions(path).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return None;
    }
    Some((width, height))
}

/// Backwards-compatible alias for [`dimensions`].
///
/// Older callers shelled out to ImageMagick's `identify`; the `image` crate is
/// faster, has no external dependency and enforces the decode budget.
pub fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    dimensions(path)
}

/// Decodes an image with the 50 MP budget enforced.
///
/// The dimensions are checked up front and the decoder itself is configured
/// with matching width/height/allocation limits, so an image cannot expand
/// into a much larger in-memory buffer than advertised.
fn decode_guarded(path: &Path) -> Option<DynamicImage> {
    let (width, height) = dimensions(path)?;

    let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(MAX_IMAGE_PIXELS * 5);
    reader.limits(limits);

    reader.decode().ok()
}

/// Computes a 64-bit difference hash (dHash) for an image.
///
/// The image is reduced to 9x8 grayscale; each bit records whether the left
/// pixel of a row-major adjacent pair is brighter than the right one. Images
/// that fail to decode, or exceed the pixel budget, return `None` instead of
/// panicking.
pub fn phash64(path: &Path) -> Option<u64> {
    let image = decode_guarded(path)?;
    let small = image
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .into_luma8();

    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            hash <<= 1;
            if left > right {
                hash |= 1;
            }
        }
    }
    Some(hash)
}

/// Absolute path of the cached thumbnail for `path` inside `out_dir`.
pub fn thumbnail_path(path: &Path, out_dir: &Path) -> PathBuf {
    let hash = fnv1a(path.to_string_lossy().as_bytes());
    out_dir.join(format!("{:016x}.jpg", hash))
}

/// Generates (or reuses) a 320px JPEG thumbnail for `path`.
///
/// Existing thumbnails that are at least as new as the source are returned
/// unchanged, which makes repeated calls idempotent. New thumbnails are
/// written to a temporary file and renamed into place so readers never observe
/// a partially written JPEG.
pub fn thumbnail(path: &Path, out_dir: &Path) -> Option<PathBuf> {
    let target = thumbnail_path(path, out_dir);
    if is_fresh(&target, path) {
        return Some(target);
    }

    let image = decode_guarded(path)?;
    std::fs::create_dir_all(out_dir).ok()?;

    let temp = out_dir.join(format!(
        ".thumb-{}-{}.tmp",
        std::process::id(),
        THUMB_TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| -> Option<()> {
        let rgb = image.thumbnail(320, 320).to_rgb8();
        let mut writer = BufWriter::new(File::create(&temp).ok()?);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 80);
        encoder
            .encode(
                &rgb,
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .ok()?;
        writer.flush().ok()?;
        Some(())
    })();

    if result.is_none() {
        let _ = std::fs::remove_file(&temp);
        return None;
    }

    // `rename` replaces the destination atomically on Unix. Windows refuses to
    // overwrite, so fall back to removing the stale file once.
    if std::fs::rename(&temp, &target).is_err() {
        let _ = std::fs::remove_file(&target);
        if std::fs::rename(&temp, &target).is_err() {
            let _ = std::fs::remove_file(&temp);
            return None;
        }
    }

    Some(target)
}

fn is_fresh(target: &Path, source: &Path) -> bool {
    let Ok(target_meta) = std::fs::metadata(target) else {
        return false;
    };
    let Ok(source_meta) = std::fs::metadata(source) else {
        return false;
    };
    match (target_meta.modified(), source_meta.modified()) {
        (Ok(target_time), Ok(source_time)) => target_time >= source_time,
        _ => false,
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use std::time::Duration;

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    /// A temp directory that removes itself on drop.
    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kensaku-image-test-{}-{}-{}",
                tag,
                std::process::id(),
                seq
            ));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn save(image: &RgbImage, path: &Path) {
        image
            .save_with_format(path, ImageFormat::Png)
            .expect("write test image");
    }

    fn solid(path: &Path, color: [u8; 3]) {
        save(&RgbImage::from_pixel(64, 48, Rgb(color)), path);
    }

    /// Horizontal brightness ramp; `invert` flips the direction, which flips
    /// every dHash comparison.
    fn gradient(path: &Path, invert: bool) {
        let image = RgbImage::from_fn(64, 48, |x, _| {
            let value = ((x * 255) / 63) as u8;
            let value = if invert { 255 - value } else { value };
            Rgb([value, value, value])
        });
        save(&image, path);
    }

    #[test]
    fn dimensions_reads_generated_png() {
        let dir = TempDir::new("dimensions");
        let path = dir.join("small.png");
        save(&RgbImage::from_pixel(10, 7, Rgb([1, 2, 3])), &path);

        assert_eq!(dimensions(&path), Some((10, 7)));
    }

    #[test]
    fn dimensions_rejects_corrupt_and_zero_byte_files() {
        let dir = TempDir::new("corrupt");
        let corrupt = dir.join("corrupt.png");
        std::fs::write(&corrupt, b"definitely not a png").expect("write corrupt file");
        let empty = dir.join("empty.png");
        std::fs::write(&empty, b"").expect("write empty file");

        assert_eq!(dimensions(&corrupt), None);
        assert_eq!(dimensions(&empty), None);
        assert_eq!(phash64(&corrupt), None);
        assert_eq!(thumbnail(&corrupt, &dir.path), None);
    }

    #[test]
    fn identical_images_have_zero_distance() {
        let dir = TempDir::new("phash-identical");
        let first = dir.join("one.png");
        let second = dir.join("two.png");
        solid(&first, [200, 30, 30]);
        solid(&second, [200, 30, 30]);

        let a = phash64(&first).expect("first phash");
        let b = phash64(&second).expect("second phash");
        assert_eq!(a, b);
        assert_eq!((a ^ b).count_ones(), 0);
    }

    #[test]
    fn inverted_pattern_has_large_distance() {
        let dir = TempDir::new("phash-pattern");
        let first = dir.join("ramp.png");
        let second = dir.join("inverted.png");
        gradient(&first, false);
        gradient(&second, true);

        let a = phash64(&first).expect("first phash");
        let b = phash64(&second).expect("second phash");
        assert!(
            (a ^ b).count_ones() >= 32,
            "inverted patterns should be far apart, distance {}",
            (a ^ b).count_ones()
        );
    }

    #[test]
    fn slight_modification_stays_close() {
        let dir = TempDir::new("phash-modified");
        let base = dir.join("base.png");
        let modified = dir.join("modified.png");
        gradient(&base, false);

        // Same ramp with the far-right quarter painted black, which reverses
        // the last gradient comparisons without touching the rest.
        let image = RgbImage::from_fn(64, 48, |x, _| {
            let value = ((x * 255) / 63) as u8;
            if x >= 48 {
                Rgb([0, 0, 0])
            } else {
                Rgb([value, value, value])
            }
        });
        save(&image, &modified);

        let a = phash64(&base).expect("base phash");
        let b = phash64(&modified).expect("modified phash");
        let distance = (a ^ b).count_ones();
        assert!(
            distance > 0 && distance < 32,
            "a small edit should produce a small distance, got {distance}"
        );
    }

    #[test]
    fn solid_colors_are_color_invariant() {
        // dHash compares neighbouring luminance values, so two *uniform*
        // images have no gradient signal regardless of their color. This
        // documents the deliberate behavior; colour-only differences are not
        // near-duplicates under a dHash.
        let dir = TempDir::new("phash-solid");
        let red = dir.join("red.png");
        let blue = dir.join("blue.png");
        solid(&red, [255, 0, 0]);
        solid(&blue, [0, 0, 255]);

        let a = phash64(&red).expect("red phash");
        let b = phash64(&blue).expect("blue phash");
        assert_eq!((a ^ b).count_ones(), 0);
    }

    #[test]
    fn thumbnail_is_written_and_reused() {
        let dir = TempDir::new("thumbnail");
        let source = dir.join("photo.png");
        let out_dir = dir.join("thumbs");
        gradient(&source, false);

        let first = thumbnail(&source, &out_dir).expect("thumbnail generated");
        assert!(first.exists());
        assert_eq!(first.extension().and_then(|e| e.to_str()), Some("jpg"));

        let decoded = image::open(&first).expect("decode thumbnail");
        assert!(decoded.width() <= 320 && decoded.height() <= 320);

        let first_mtime = std::fs::metadata(&first)
            .and_then(|meta| meta.modified())
            .expect("thumbnail mtime");

        std::thread::sleep(Duration::from_millis(60));
        let second = thumbnail(&source, &out_dir).expect("thumbnail reused");
        assert_eq!(first, second);

        let second_mtime = std::fs::metadata(&second)
            .and_then(|meta| meta.modified())
            .expect("thumbnail mtime");
        assert_eq!(
            first_mtime, second_mtime,
            "fresh thumbnail must not be rewritten"
        );
    }
}
