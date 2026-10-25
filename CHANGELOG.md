# Changelog

All notable changes to Kensaku. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning: [SemVer](https://semver.org/).

## [0.3.3] — 2026-10-26

### Fixed
- **Images and PDFs were not indexed for existing installs.** Early UI configs
  stored exclusions with a leading dot (`.jpg`, `.pdf`), which the v1 migration
  did not normalize, so those files were skipped forever. A v2 migration strips
  dotted document/image extensions and bumps the config version. Restart after
  upgrading; the startup crawl picks the files up.
- **Thumbnails were blank.** The Tauri asset scope used `$APPCONFIG`, which is
  `<config>/<identifier>`, while thumbs live at `<config>/kensaku/thumbs`. The
  scope now starts with `$CONFIG/kensaku/thumbs/**`.
- **Search took 5–9 s.** Snippets called the full extractor (PDF/OCR) inside the
  query. They now read plain text directly or a cached extraction only; a second
  batched request fills them after the results paint. Measured: 3.86 s → 0.14 s.
- **The metadata DB was 310 MB for 382 rows.** Startup now vacuums when free
  pages exceed 20%, caps the WAL at 64 MB, and checkpoints after crawls
  (310 MB → 604 KB).
- Image previews no longer run OCR; previews are debounced and sequenced.
- `content_docs` reports the real Tantivy document count.

### Changed
- Release pipeline builds bundles and CLI binaries as workflow artifacts;
  publishing is done with `gh` (see `docs/RELEASING.md`).
- Packaging: `bench` is now a cargo example and `kensaku-cli` is a separate
  crate, so Tauri bundles exactly one binary. This fixes the macOS universal
  build failing while trying to copy the extra binaries.

## [0.3.1] — 2026-10-26

### Fixed
- **OCR could not be turned on.** The Settings switch was a disabled placeholder
  and `ocr_enabled` was never read by the backend. The switch now persists to
  config, extraction is gated on it, and changing it re-scans only images.
- **OCR silently produced nothing when tesseract had no language data.** Settings
  now reports engine/language status, can download `eng.traineddata` (~4 MB) into
  Kensaku's own directory, and shows platform install commands when the engine is
  missing. Doctor gained an OCR check.

### Changed
- `content_pending` column lets a settings change re-process only the affected
  files; pending images with no extractable text have stale documents removed.

## [0.3.0] — 2026-10-26

The open-source launch. Seven focused changesets on top of v0.2.0.

### Added
- **Agent surface.** `kensaku-cli` with `search`, `state`, and `mcp`; an MCP stdio
  server exposing `search`, `read_snippet`, and `index_state` over the same
  read-only index. Token-budgeted snippets, ranked and exhaustive modes, path/ext/
  lookback filters. See `docs/AGENTS.md`.
- **Real static embeddings.** Manual safetensors (F32/F16) + BERT WordPiece loader
  for potion-base-8M, loaded once via a global registry; automatic re-embed when
  the model id changes. Hashing estimator remains the fallback.
- **Image support.** Lazy cached thumbnails (320 px JPEG), dimensions, 64-bit dHash,
  perceptual near-duplicate groups merged into `find_duplicates`, thumbnail-forward
  results and preview.
- **Query semantics.** `re:` regex execution, true `name:`/`content:` field scoping
  in BM25, `mode:exhaustive`, and a semantic candidate prefilter.
- **Robustness.** Single-instance lock, atomic config writes with corrupt-file
  backup, unavailable-root preservation, 8-second extraction timeout on a bounded
  pool, directory-rename subtree rewrite, Windows cloud-placeholder guard.
- **Launch plumbing.** Start at login, rotating local crash log (256 KiB) with a
  Doctor check, unsigned multi-OS draft release workflow, CLI artifacts.
- **Tests and CI.** 77 Rust tests (parser, config migration, extraction, fuzzy
  relevance, ranking, MCP/CLI, images, crash log), `clippy -D warnings`, web and
  rust workflows, benchmark methodology.

### Changed
- Ranking now normalizes RRF before bonuses, bounds the depth penalty, clamps
  scores at zero, and drops weak semantic-only hits. This removes negative scores
  and shallow-but-irrelevant top results.
- The main window is files-only: search, category chips, results, preview. Stats
  and actions live in Settings.
- Grid view navigates by rows (Up/Down jump a row, Left/Right stay in the row).
- Default exclusions no longer skip PDF/Office/image formats now that extraction
  and OCR exist; a `config_version` migration cleans existing configs.
- Rewritten README, install/usage/FAQ/security/contributing docs, LICENSE.

### Fixed
- Document and image indexing were effectively disabled by legacy default
  exclusions.
- `state` reports the units' model id separately from the active query model.
- Embed queue no longer renders an inverse fill for queue depth.

## [0.2.0] — 2026-10-25

- Durable SQLite metadata with warm start; incremental crawl; identity by
  `(dev, ino)`; rename/delete handling; stale-document purge.
- Full query language and three-tier RRF fusion.
- PDF, Office, EPUB/ODT, RTF extraction; tesseract OCR.
- UI v2: `paper`/`void` themes, category chips, list/grid, command palette,
  preview pane, Settings with Doctor, Stats for Nerds, model download and factory
  reset.

## [0.1.0] — 2026-10-24

- Initial release: Rust engine (nucleo, Tantivy, jwalk, notify) and Tauri shell
  with a floating search window, tray, global shortcut and onboarding.
