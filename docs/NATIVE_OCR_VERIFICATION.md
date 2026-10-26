# Native Platform OCR — status and verification plan

> **Status: blocked / not implemented.** K21 was attempted by an automated pass,
> but the macOS path shelled out to `osascript` and the Windows path returned
> `None`. Neither is a native OCR implementation. The fake paths were removed so
> Kensaku never claims a backend it cannot provide.

## Current behavior

Kensaku has two working OCR paths:

1. **Tesseract** — the default, external binary.
2. **PP-OCRv6 tiny** — compiled into default app builds via `accurate-ocr`, plus
   a downloaded model pack that the user opts into.

The retired `ocr_engine = "native"` value falls back to Tesseract. There are no
`native-ocr-macos` or `native-ocr-windows` Cargo features in the tree.

Scanned PDFs are no longer only a marker: `extract_pdf` attempts to pull embedded
image streams and send them through the normal image OCR path. This is partial
(page rendering is not implemented), but it is the same Tesseract/PP-OCRv6
pipeline, not a native backend.

## What a real implementation requires

### macOS

- Use a maintained Objective-C binding such as `objc2-vision` and
  `objc2-foundation`.
- Create a `VNImageRequestHandler` from the image URL.
- Run `VNRecognizeTextRequest` at the desired recognition level.
- Join the top candidate strings with newlines.
- Feature-gate the dependency and module behind `target_os = "macos"`.
- Add a macOS CI job or a documented manual run; a Linux-only build cannot
  validate this path.

### Windows

- Use the `windows` crate with `Media::Ocr`, `Storage`, and
  `Graphics::Imaging` features.
- Create a `SoftwareBitmap` from the image and an `OcrEngine` for the user's
  language.
- Run `RecognizeAsync` and join the returned lines.
- Feature-gate the dependency and module behind `target_os = "windows"`.
- Add a Windows CI job or a documented manual run.

### Scanned PDFs

- Full-page rendering needs a licensed renderer. Extracting embedded image
  streams is only a partial solution and must stay documented as such.
- Do not claim scanned-PDF OCR is complete until a rendered page is OCR'd.

## Verification checklist for the future implementation

- [ ] Building on Linux without native features is unchanged.
- [ ] macOS build without Tesseract extracts text from the frozen UI fixture.
- [ ] Windows build without Tesseract extracts text from the frozen UI fixture.
- [ ] Missing language pack returns a clear state and falls back to Tesseract.
- [ ] No shell command is used to reach the platform API.
- [ ] Per-image latency and peak memory are recorded in `BENCHMARKS.md`.
- [ ] The docs status table is updated only after a real platform run.

Until then, K21 remains blocked in `REMAINING.md` and `ROADMAP.md`.
