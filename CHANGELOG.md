# Changelog

All notable changes to Kensaku. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning: [SemVer](https://semver.org/).

## [Unreleased]

### Added
- K15 CJK-aware Tantivy tokenization: custom "kensaku" tokenizer decomposing Han,
  Hiragana, Katakana, and Hangul into interleaved unigram and bigram tokens with
  exact byte offsets while preserving standard SimpleTokenizer behavior on Latin,
  identifiers, and paths. Flushes at mixed script boundaries (`README最終.md`, `src/组件/index.ts`).
  Schema marker `content.schema` (v2) detects legacy indexes, cleanly triggers
  one-time index rebuilds, and surfaces the rebuild reason in Doctor and stats.
  Measured 50 CJK docs (100 CJK chars each): index size grows from 4,944 B to
  48,128 B (9.73x) due to unigram, bigram, and folded trigram expansion.
- K13 native quick-search spike: an opt-in `kensaku-quick` egui binary
  (`--features quick-ui`) backed by the daemon. Debug measurements: 8.6 ms to
  first UI, 46 ms daemon round-trip, ~223 MB hidden RSS versus ~350 MB for the
  Tauri debug window. Not wired into the product.
- K6 optional accurate OCR pack: PP-OCRv6 tiny detector + recognizer behind
  the `accurate-ocr` feature, downloaded with pinned SHA-256 and selectable in
  Settings/Setup with automatic Tesseract fallback. A frozen 104-image UI-text
  fixture scores 104/104 exact lines. Native macOS/Windows OCR remains future
  platform work.
- K1 daemon parity: the GUI now fetches snippets, `read_snippet`, stats, home
  data, duplicate groups, scope suggestions and pin toggles through the daemon
  instead of opening metadata in-process. IPC now uses `interprocess` local
  sockets, giving the same protocol a Unix socket and a Windows named pipe.
  K4 visible-row snippets use a bounded insertion-order LRU, and
  `read_snippet` returns line ranges.
- K14 startup quantization drops the temporary f32 matrix after quantizing.
  Validated on real Potion artifacts: 7,559,168-byte i8 matrix + 118,112 bytes
  of scales, with MRR 1.000 for both f32 and i8 on the labelled set.
- K9 download hygiene framework: resumable `.part` downloads, HTTP Range resume,
  mirror fallback, atomic rename, SHA-256 verification, progress reporting, a
  disk-space precheck, and pinned checksums for the three Potion and three
  PP-OCRv6 artifacts. Existing files are re-verified instead of trusted by name.
- K7 default scheduling mode is now `full`; balanced/eco still opt in.
- K3 thumbnail version fix: legacy 320 px thumbnails are no longer hashed;
  `image_hash_version` bumped to 2.
- K2 metadata memory fix: replaced the `HashMap` path/id/burst maps with a
  compact open-addressed path index, lazy id index, and lazy burst vector.
  1M synthetic entries now measure 92.8 MB RSS (target ≤ 100 MB); path lookup
  p50 0.261 µs.
- Index-format decision memo and reproducible benchmark: at 1M synthetic
  documents, SQLite FTS5 trigram used 918 MB and took 258 ms for a broad BM25
  query, versus 100 MB and 275 µs for Tantivy + folded. Keep Tantivy; no
  production index change.
- Screenshot burst grouping: images captured within ten minutes in the same
  folder are compared by text-line overlap and thumbnail dHash, chained to the
  lowest-id burst, and collapsed to one search row with a version count.
  Assignments store `burst` and `look`; old rows are backfilled 50 at a time
  while idle. `/dupes` reuses the same burst data. Distinct pages that share
  chrome stay separate.
- Opt-in external extractors: config-defined commands keyed by extension or
  MIME selector, with an absolute-executable allowlist, no shell, cleared
  environment, 100–7000 ms timeout, 1 KiB–4 MiB stdout cap, and process-group
  kill. Zero plugins ship by default and each entry requires `enabled: true`;
  changing the registry clears cached results and queues affected files for
  re-extraction. Doctor lists configured commands and recent failure reasons.
  See [docs/EXTRACTORS.md](docs/EXTRACTORS.md).
- MCP v2: daemon-first sessions avoid opening SQLite, Tantivy or the embedding
  table in the agent process; `health_check`, bounded `regex_search`, cursor +
  `page_size` pagination, per-tool token budgets, `truncated` snippet flags, and
  an extended `index_state` (phase/progress/model freshness). The only write
  tool, `reindex`, is hidden and disabled unless `mcp_allow_reindex` is enabled
  in config and the daemon is running; Settings → Agents exposes the switch.
  Daemon search now queries the live manager stores instead of re-opening the
  index per request. Verified with a manual MCP stdio session and daemon
  handshake; see BENCHMARKS.md.
- Folded/trigram content tier: `content_folded` plus stored `content_raw`
  fields, OCR-lookalike folding, raw-text verification (1 misread for 4–7
  letters, 2 for 8+), and a `ContentFolded` result badge. The tier only runs
  when exact/fuzzy/semantic results leave a shortfall. Existing indexes are
  preserved as `index.legacy-*` and rebuilt; all files are queued for content
  re-extraction once.
- Background indexing policy: `crawl_mode` (`full` / `balanced` / `eco`) and a
  persisted `pause_indexing` switch, both in Settings → Search. Balanced/eco use
  OS idle CPU/I/O priority; eco also paces new extraction jobs on battery.
  Pausing still applies renames and deletes.
- Watcher hardening: 400 ms settle per path, coalesced events, fresh-file
  fingerprint retries for 10 s, overflow rescan that reconciles missing and
  ghost paths, and newest-first ordering within text/image classes.
- Instrumented fast-search test proving the Tauri path never reads or returns
  snippets; snippets remain a separate batched `get_snippets` round-trip.
- Thumbnail-based perceptual hashing (480 px thumbnails reused for dHash and
  previews), `image_hash_version` invalidation, and best-effort
  `release_memory()` after image/PDF/Office work.
- `kensaku-daemon` scaffold: a headless process that owns `IndexerManager`,
  listens on `$XDG_RUNTIME_DIR/kensaku.sock` (0600), enforces single-instance
  locking, writes `daemon.pid`, and serves token-authenticated JSON-RPC
  `health`/`index_state`/`shutdown`. CLI/MCP `state`/`index_state` now prefer
  the daemon and fall back to direct read-only access when it is absent.
  The daemon also exposes `get_status`, `ping`, `health`, `reindex`, and
  search with an offset; it handles SIGTERM/SIGINT, validates the
  token/protocol handshake, removes its socket/PID on SIGTERM and shutdown, and
  clients can opt out with `KENSAKU_NO_DAEMON=1`. CLI/MCP search now prefers
  the warm daemon and falls back locally. The Tauri app still runs the
  in-process indexer (the `--no-daemon` fallback).
- Compact metadata arena (`NodeArena`/`NameArena`): per-entry node is 64 bytes
  (down from 128-byte `FileEntry`), full paths are reconstructed from parent
  chains instead of being stored per entry.
- Guided first-run setup: folders → exclusions → preferences → optional packs →
  first scan → Done. The first scan is gated by `first_scan_done`; interrupted
  scans resume. Settings → About offers **Run setup again**.
- Deterministic plain-language compiler ("latest resume", "screenshots from last
  week", "big videos in Projects") with removable interpretation chips.
- Personal aliases (`resume → cv`, seeded + editable in Settings → Search).
- Inline `/` command menu and `@` directory scope picker.
- Explicit sort: Best match / Newest / Oldest / Largest / Name, applied
  server-side before the result limit.
- Server-side facet counts and multi-select/exclude category facets.
- Version grouping (`resume_v2`, `resume_final`, `(1)`, `-copy`).
- Home screen with pins, recent files, saved searches, and suggested queries.
- Pagination / **Load more**.
- Duplicate review view (`/dupes`).
- Default-deny sensitive paths and credential extensions.
- `Makefile` with dev/build/test/bench and `reset` / `reset-index` targets.

### Changed
- Removed the `Ctrl/Cmd+K` command palette and the old `>` commands; `/` replaces
  them. The full query grammar remains for power users, CLI, and MCP.
- Setup owns OCR/model downloads. Settings shows installed status and links to
  **Run setup again**.
- First-run scanning shows live counts, a progress bar, and an ETA for metadata
  indexing and extraction.

### Performance
- **Static embedding table shrunk ~4x.** `potion-base-8M` is stored as i8 with
  one scale per row (30.2 MB → 7.7 MB) and dequantized while mean-pooling.
  `semantic_f32_table` (or `KENSAKU_F32_TABLE=1`) keeps the f32 matrix for
  comparison; rankings and query p50 are unchanged.
- **One extraction pass.** The crawl now feeds the lexical (Tantivy) and
  semantic indexes from the same extraction instead of re-reading and
  re-extracting every file for embeddings. Images are OCR'd once, not twice.
- **OCR no longer thrashes the CPU.** Each tesseract child is pinned to one
  thread (`OMP_THREAD_LIMIT=1`) while Kensaku runs several OCR processes in
  parallel; previously every child spawned one OpenMP thread per core, which
  made concurrent OCR slower than sequential.
- **No unbounded timeout backlog.** Extraction slots are bounded, so slow
  documents can no longer queue detached work faster than it completes (which
  used to turn a crawl into hundreds of spurious 8-second timeouts).
- **Perceptual hashing no longer blocks indexing.** Images contribute their
  dimensions from the header during the crawl; the perceptual hash is computed
  afterwards by a one-decode-at-a-time background pass for `/dupes`. Previously
  a two-slot decode gate sat in front of OCR and serialized image-heavy crawls.
- **Live progress counters.** Document and embedding counts are published during
  extraction instead of only at commit boundaries, so the first-run UI no
  longer shows `0` while work is happening.
- **Faster dev builds.** `make dev` now compiles dependencies (`image`, Tantivy,
  PDF/Office parsers) at `opt-level = 2` without dependency debug info, so
  profile builds are much closer to release and `target/debug` is several GB
  smaller.
- **Streaming index commit.** Extracted text is written through a bounded
  channel instead of accumulating every document in memory until the end.
- **No full index purge per crawl.** Disappeared files are removed by path
  directly instead of scanning every stored document.
- **Smaller steady-state footprint.** Tantivy's writer arena is 16 MB (down from
  50 MB), extraction/snippet caches are bounded, semantic units load without a
  duplicate copy, and image hashing decodes at most two images at a time.

### Fixed
- Shortcut recording now captures a real key combo (modifiers accumulate across
  key presses) instead of staying on the default.
- Optional-pack downloads show live progress, success, and errors.
- React hook-order crash after setup gating that blanked the window.

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
