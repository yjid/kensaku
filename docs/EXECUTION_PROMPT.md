# Kensaku — execution prompt (K1–K14, completed)

> **Status: completed.** K1–K14 landed in the local `master` history. This file
> is kept as a historical execution guide; see `docs/ROADMAP.md` for the current
> status table and `docs/REMAINING.md` for what is still open. Do not re-run the
> tasks without checking the current tree.

You are a senior Rust/Tauri engineer working in this repository. The roadmap in
`docs/ROADMAP.md` (tasks K1–K14) was implemented one task at a time, with tests,
clippy, and docs updated per commit.

## 0. Read first (in this order)

1. `docs/ROADMAP.md` — the tasks and acceptance criteria.
2. `docs/ARCHITECTURE.md` — target topology, index tiers, budgets.
3. `docs/LANDSCAPE.md` — anonymized research; use the ideas, not code.
4. `docs/REMAINING.md` — full backlog and sequencing notes.
5. `docs/AGENTS.md` — the read-only agent surface contract.
6. `docs/BENCHMARKS.md` — how to measure.

Do not add attribution, names, or links for the anonymized screenshot-search
reference in `docs/LANDSCAPE.md`. Keep that document anonymized.

## 1. Current state (verify in the repo; do not assume)

- Rust 2024, Tauri 2 + React/Vite, Tantivy 0.26, SQLite (rusqlite bundled),
  static potion embeddings (256-dim, i8 vectors), Tesseract subprocess OCR.
- `IndexerManager` lives in `src-tauri/src/indexer/mod.rs` and is owned by
  `kensaku-daemon`; the Tauri app uses `DaemonClient` and keeps a read-only
  manager only as a fallback (`src-tauri/src/lib.rs`).
- The CLI (`cli/src/main.rs`) is a thin wrapper over `kensaku_lib::cli::run`.
  MCP lives in `src-tauri/src/agent/mcp.rs`.
- Metadata: `indexer/metadata.rs` (`Vec<FileEntry>` + `HashMap<PathBuf, usize>`)
  and `indexer/metadata_db.rs` (SQLite WAL).
- Content: `indexer/content.rs` (Tantivy, 16 MB writer arena, snippet cache).
- Extraction: `indexer/extract/` (`pdf.rs`, `office.rs`, `image.rs`, `text.rs`).
- Images: dimensions from header; perceptual hashing deferred to a background
  backfill; thumbnails generated on demand.
- Embeddings: `embed/table.rs` (f32 static table, ~30 MB), `embed/units.rs`,
  `embed/queue.rs`, `embed/scanner.rs` (exact i8 scan).
- Search: `search/mod.rs` (RRF), `search/fuzzy.rs` (nucleo), `query/`.
- Benchmarks: `src-tauri/examples/bench.rs`, `index_bench.rs`, `crawl_bench.rs`.
- Docs: `docs/`.
- Current verification (at the start): 103 Rust tests, clippy clean, tsc clean.
  Do not trust these numbers; rerun them.
- Dev profile optimizes dependency code (`opt-level = 2`, dependency debug info
  off). Release builds are expensive; prefer `cargo check`, `test`, and the
  example harnesses unless a task needs a release number.
- The repository is clean and local. **Do not push. Do not rewrite history.
  Do not run destructive git commands.**

## 2. Hard rules

- **Do not push** to any remote. Commit locally only.
- **Do not add** the anonymized source app's name, URL, screenshots, or
  attribution anywhere.
- **Do not copy** code from GPL/AGPL projects. Use ideas and reimplement in
  this codebase. Keep Kensaku MIT-compatible.
- **No new network calls by default.** Any download is explicit, checksummed,
  and initiated by the user.
- **Read-only agent surface by default.** Do not add write/shell/network tools
  to MCP unless the task explicitly says so and the tool is opt-in.
- **Never reset** a user's real index/config in tests. Use temp dirs
  (`std::env::temp_dir`, `XDG_CONFIG_HOME`, or the existing test helpers).
- **No schema change without a migration.** Bump a version, write the migration,
  and provide a rebuild/fallback path. Never silently drop user data.
- **One task per commit.** Commit message: `task Kx: <short description>`.
  Keep unrelated files out. Update `CHANGELOG.md` under `[Unreleased]`.
- **Definition of done per task:** tests pass, clippy clean, tsc clean if the
  UI changed, benchmark/receipt recorded, docs updated, changelog entry.
- If a task is too large, split it into sub-tasks (e.g. K1.1, K1.2) and commit
  each. Report exactly where you stopped.

## 3. Working protocol

For each task:

1. Write a 5–10 line plan in the commit message or a scratch note.
2. Read the relevant files before editing. Preserve existing style.
3. Implement the smallest correct change.
4. Add tests: unit tests near the code; integration/benchmark where noted.
5. Run the verification commands for that task (see §6).
6. Update docs and changelog.
7. Commit locally.
8. Report: files changed, tests run, numbers, risks, next task.

If blocked, stop and report the blocker. Do not invent APIs or silently skip
acceptance criteria.

## 4. Task order

Work in this order. Dependencies are explicit.

| Order | Task | Depends on |
|---|---|---|
| 1 | K2 compact metadata arena | — |
| 2 | K1 daemon split | K2 |
| 3 | K3 thumbnail hashing + memory release | K1 |
| 4 | K4 lazy snippets | K1 |
| 5 | K7 scheduling + watcher robustness | K1 |
| 6 | K5 folded/trigram tier | K4 |
| 7 | K6 optional PP-OCRv6 OCR pack | K1 |
| 8 | K8 MCP v2 | K1 |
| 9 | K9 download hygiene | — |
| 10 | K10 pluggable extractors | K1 |
| 11 | K11 result grouping | K3 |
| 12 | K12 index-format spike | — |
| 13 | K13 native UI spike | K1 |
| 14 | K14 i8 embedding table | K2 |

## 5. Task specifications

### K2 — Compact metadata arena

**Goal:** replace the fat per-file in-memory entry with contiguous arenas; target
1M entries ≤ 100 MB (stretch 64 MB) while keeping filename search fast.

**Current:** `MetadataStore { entries: Vec<FileEntry>, path_index: HashMap<PathBuf, usize>, node_id_to_idx: HashMap<u64, usize>, ... }`. `FileEntry` holds `PathBuf`, `String`, `Option<String>`, etc. `get_entries()` is used by search, watcher, commands, and tests.

**Implement**
- New `NodeArena`:
  - `NodeId = u32`.
  - Fields: `parent: NodeId`, `name_off: u32`, `name_len: u32`, `ext_id: u16`,
    `flags: u8`, `size: u64`, `mtime: u64`, `dev: u64`, `ino: u64`,
    `char_bloom: u64`, `node_id: u64` (SQLite id).
- `NameArena`: one `Vec<u8>` for names; append-only. Store names as UTF-8 bytes.
  Document that non-UTF-8 paths are stored lossily for now (existing behavior)
  and add a TODO for `OsStr` bytes.
- Extension interner: `Vec<Box<str>>` + `HashMap<Box<str>, u16>`.
- Path lookup without storing full paths: a `HashMap<u64, u32>` keyed by a stable
  hash of the full path; on collision, compare the reconstructed path. Reconstruct
  by walking parents (only for displayed results/filters).
- Keep `generation`/`dir_generation` and the scope-trie invalidation behavior.
- Port call sites:
  - `search/fuzzy.rs`, `search/mod.rs`: iterate node ids/names/blooms; do not
    materialize `FileEntry` per candidate.
  - `query/scope_trie.rs`: build from arena directory nodes.
  - `indexer/watcher.rs`, `commands.rs`, `agent/mcp.rs`: use accessors.
- Keep a temporary `FileEntry`-shaped view (`EntryRef`/`materialize`) for code
  that still needs owned values; do not keep the old `Vec<FileEntry>`.
- Add a benchmark/test helper that builds N synthetic entries and prints
  approximate memory. Record results in `docs/BENCHMARKS.md`.

**Acceptance**
- All existing tests pass.
- 1M synthetic entries: metadata RSS ≤ 100 MB (stretch 64 MB); report the number.
- 100k filename p50 ≤ 2.8 ms; no regression on `bench`.
- Incremental add/remove of 10k entries does not regress.
- No duplicate full-path storage remains in the hot path.

**Pitfalls:** borrow checker across arena views; path reconstruction cost;
non-UTF-8 paths; `HashMap` collision handling; mutations while iterating.

### K1 — Daemon split

**Goal:** one process owns the index; GUI, CLI, and MCP attach as clients.

**Implement**
- New binary `kensaku-daemon` (add `[[bin]]` in `src-tauri/Cargo.toml` or a new
  crate; keep the core library reusable).
- Move `IndexerManager`, watcher, config hot-reload, and embedding queue into the
  daemon. Keep `IndexerManager` public for tests/fallback.
- IPC: newline-delimited JSON-RPC 2.0. Linux/macOS: Unix domain socket at
  `$XDG_RUNTIME_DIR/kensaku.sock` (fallback `$CONFIG/kensaku/kensaku.sock`),
  permissions 0600. Windows: named pipe `\\.\pipe\kensaku`. Prefer a small
  cross-platform local-socket abstraction; if adding a crate, justify it.
- Handshake: protocol version + token from `$CONFIG/kensaku/daemon.token` (0600,
  generated once). Reject mismatched versions/tokens with a clear error.
- Methods: `search`, `read_snippet`, `index_state`, `get_status`, `reindex`,
  `health`, `shutdown`, and a status subscription (or polling).
- Single instance: reuse/replace the existing `kensaku.lock`; write
  `daemon.pid`; remove stale sockets; graceful SIGTERM.
- Tauri app:
  - Replace the in-process `IndexerManager` state with a `DaemonClient`.
  - On startup, connect; if absent, spawn the daemon (dev and bundled paths) and
    retry with backoff.
  - `commands.rs` calls the daemon; preserve the existing TS command shapes.
- CLI/MCP:
  - Connect to the daemon if available.
  - Fallback to the current read-only direct open when the daemon is absent.
- Feature flag: keep the in-process path available for one release
  (`--no-daemon` / env) so we can compare and roll back.

**Acceptance**
- Closing the GUI does not stop indexing; reopening reconnects to the same crawl.
- `kensaku-cli state` answers in < 50 ms and does not load metadata/units.
- A second daemon exits cleanly; no duplicate writers; socket is 0600.
- CLI/MCP fallback works with no daemon.
- IPC tests: version mismatch, bad token, malformed JSON, disconnect/reconnect.
- Existing tests pass; daemon integration test uses a temp config dir.

**Pitfalls:** Tauri `State`/async runtime; Windows paths; stale sockets; daemon
lifecycle on app quit; packaging the binary; log locations; not blocking the
UI thread.

### K3 — Thumbnail hashing + memory release

**Goal:** never decode a full image just for a perceptual hash; reuse thumbnails;
return freed memory to the OS.

**Implement**
- Add `release_memory()` in a platform module:
  - Linux glibc: `libc::malloc_trim(0)`.
  - macOS: `malloc_zone_pressure_relief(malloc_default_zone(), 0)`.
  - Windows: `EmptyWorkingSet`/`SetProcessWorkingSetSize` or equivalent.
  - No-op elsewhere; never fail.
- Generate a 480 px thumbnail once per image (reuse the existing thumbnail code
  and `thumbs/` directory; adjust width/quality if needed).
- Compute the dHash from the thumbnail (not the source image). Reuse the
  thumbnail for the grid/preview.
- Backfill:
  - If a fresh thumbnail exists, hash it without reading the source.
  - Otherwise decode once, write the thumbnail, hash it.
  - One image at a time; pause during crawls/queries; call `release_memory()`
    after each image.
- Record a `image_hash_version` in `meta`; if the hashing source changes, mark
  old rows for rehash (do not silently mix hash spaces). `/dupes` must remain
  correct.
- Call `release_memory()` after large PDF/Office extraction too.

**Acceptance**
- 1,400 images: no full-resolution decode when thumbnails exist; peak indexer
  RSS ≤ 100 MB; report before/after.
- Daemon RSS returns to baseline within 5 s after a batch.
- `/dupes` still groups near-identical images; no false merges introduced.
- Tests for thumbnail reuse, hash determinism, and memory-release no-op paths.

**Pitfalls:** old vs new hash spaces; thumbnail freshness (`mtime`); EXIF
orientation; huge images; Windows file locking.

### K4 — Lazy snippets and line highlighting

**Goal:** do not fetch snippets for results that are not visible.

**Implement**
- Ranked search returns result metadata only: path/name, score, match type,
  best range (and line boxes if cheap), no snippet text.
- New batched command/API: `read_snippets(items: [{path, query, max_chars}])`,
  capped (e.g. 50 items), debounced.
- Frontend: IntersectionObserver on result rows; fetch only visible items;
  keep a small LRU keyed by `(path, mtime, query)`; show a subtle placeholder.
- MCP: keep `read_snippet`; search may return `snippet_available: true` instead
  of text. Preserve current consumers.
- Snippet extraction must never invoke the full document extractor in the query
  path; use the extraction cache/sidecar or a bounded read.

**Acceptance**
- 2-char query matching ~2,000 files: p95 ≤ 20 ms to first paint.
- Instrumented test: no snippet read/extraction for off-screen results.
- Snippets/highlight ranges match current behavior for visible rows.
- MCP contract and token budgets still pass.

**Pitfalls:** stale snippets after edits; query changes while requests are in
flight (sequence guard); huge snippets; Unicode boundaries.

### K7 — Scheduling and watcher robustness

**Goal:** indexing never fights typing; files are read only after they settle.

**Implement**
- Config: `crawl_mode = full | balanced | eco` (serde default `balanced`),
  `pause_indexing: bool`.
- Priority:
  - Linux: `SCHED_IDLE` + `ioprio_set` idle.
  - macOS: `PRIO_DARWIN_BG`.
  - Windows: background mode or `SetPriorityClass` idle.
  - Apply only in `balanced`/`eco`; `full` stays normal.
- Battery detection (Linux `/sys/class/power_supply`, macOS IOKit, Windows
  `GetSystemPowerStatus`). In `eco`, pace the backlog; fresh files still jump
  the queue.
- `start_indexing`: apply priority at thread start; check pause/battery between
  files; expose status.
- Ordering: text/small first for the first scan, but **newest-first within each
  type**; fresh watcher events jump the queue.
- Watcher:
  - 400 ms settle per path; coalesce events.
  - Retry files written within 10 s (browser save/rename pattern).
  - Rename pairing (from/to), subtree delete, overflow → rescan.
  - Pause still processes renames/deletes.
- Tests: config parsing; watcher unit tests for settle/rename/delete/overflow;
  a synthetic crawl with pause/battery stubs.

**Acceptance**
- During a full crawl on 10k files, keystroke p95 ≤ 40 ms.
- Eco mode is visibly slower but responsive; fresh files still index first.
- No half-written files indexed; no ghosts after rename/delete.
- Settings take effect without restart where possible.

**Pitfalls:** SCHED_IDLE makes tests/dev slow; gate it; platform syscalls must
fail soft; battery detection on desktops.

### K5 — Folded / trigram content tier

**Goal:** fast substring and OCR-error-tolerant content search, ranked after
exact matches, with raw-text verification.

**Implement**
- Add `fold(text)`: lowercase; map `0↔o`, `1/l/i/|`, `5/s/$`, `8/b`, `m↔rn`,
  `w↔vv`, `d↔cl`; preserve spaces (do not fold across spaces).
- Index a second content field (`content_folded`) with an n-gram/trigram
  tokenizer (Tantivy `NgramTokenizer`, min=3, max=3) or evaluate a SQLite FTS5
  trigram sidecar in K12. Do not replace BM25 for normal queries.
- Store enough original text to verify candidates (a stored `content_raw` field
  or a compact sidecar). Keep the 1 MB cap; measure index-size growth.
- Query flow:
  1. exact BM25/name/semantic as today.
  2. if results < limit, query folded trigrams for terms ≥ 3 chars.
  3. verify each candidate against raw text: 1 misread letter for 4–7 letters,
     2 for 8+, none under 4; reject false positives.
  4. append folded hits after exact hits, with a lower RRF weight and
     `match_type = ContentFolded`.
- Terms < 3 chars: use the existing bounded `LIKE`/regex fallback only over
  already-narrowed rows.
- Schema migration + full rebuild path; never mix folded/raw versions.

**Acceptance**
- `nutsmp` finds `donutsmp.net`; `0RDER` finds `ORDER`.
- `email` does not match inside `internally`; no mass false positives.
- Folded tier only runs on shortfall; p95 stays within K4 budget.
- Index size/time recorded; all tests pass (including short/CJK terms).

**Pitfalls:** trigram query construction (AND of ngrams), Unicode, regex
metacharacters, stored-text size, index migration.

### K6 — Optional PP-OCRv6 OCR pack

**Goal:** Tesseract stays the fast default; an opt-in pack reads small UI text.

**Implement**
- `OcrEngine` trait: `name()`, `read(path) -> Option<DocumentExtract>` or text.
- `TesseractEngine`: current behavior (OMP pinned, bounded workers).
- `PaddleEngine` (feature `paddle-ocr`): PP-OCRv6 tiny detector + small
  recognizer on ONNX Runtime via `ort` `load-dynamic`; models + ORT downloaded
  with SHA-256; 22 MB + 24 MB. Run in a worker; catch panics; document peak
  memory. Reuse the model-download manifest from K9.
- `NativeEngine` (feature per OS): macOS Vision, Windows OCR. Lower memory;
  best platform integration.
- Config/UI: `ocr_engine = auto | tesseract | paddle | native`; setup/Settings
  pack with disk estimate, checksum, fallback. `auto` = native if available,
  else Tesseract.
- Keep one extraction pass; the engine is chosen per file type/format.
- Frozen fixture: 100 UI screenshots (or synthetic) + expected words. Add a
  benchmark/test that reports word recall vs the Paddle reference and peak RSS.

**Acceptance**
- Recall ≥ 95% of words on the frozen UI-text set for the PP-OCRv6 engine.
- Pack off: no new dependency, no memory cost, current behavior.
- Missing/corrupt pack → Tesseract fallback, clear error, no orphan process.
- License/attribution recorded for models/ORT.

**Pitfalls:** ONNX Runtime ABI/glibc, model licensing, in-process crash isolation,
memory spikes, platform feature flags.

### K8 — MCP v2

**Goal:** a daemon-backed, paginated, agent-safe surface.

**Implement**
- Connect MCP to the daemon; fallback read-only when absent.
- Add tools:
  - `health_check` (daemon/db/index/model).
  - `index_status` (keep; extend with phase/progress).
  - `reindex` — **opt-in write action**, disabled by default; only if the user
    enables it in config. Document clearly.
  - `grep` / `find_symbol` (tree-sitter) if feasible; otherwise a bounded
    `regex_search`.
- Pagination: cursor + `page_size` for ranked/exhaustive; keep the 10k cap.
- Per-tool token budgets; truncate snippets with `truncated: true`.
- Contract tests: tool schemas, pagination, auth/version, read-only default,
  token budgets, error shapes.

**Acceptance**
- MCP search p95 < 50 ms warm; pagination beyond 10k without OOM.
- No write tool enabled by default; read-only guarantees hold.
- Cursor/Claude Code manual check documented in the PR/commit.

**Pitfalls:** stdout purity (JSON-RPC only), backpressure, daemon reconnect,
security of `reindex`.

### K9 — Model and download hygiene

**Goal:** every download is verifiable, resumable, and honest about failure.

**Implement**
- One pack manifest: id, name, version, URL(s), SHA-256, size, license, target
  dir, optional disk precheck.
- Downloader: `.part` file, HTTP Range resume, progress events, atomic rename,
  checksum verification, mirror fallback, clear errors.
- Use it for: embedding model, OCR models, ONNX Runtime, future Vision Packs.
- Setup/Settings/Doctor share the manifest; no parallel download path.
- Tests: corrupt checksum, interrupted download (mock/local server), resume,
  disk-full behavior, mirror fallback.

**Acceptance:** corrupt download fails loudly and retries; interrupted download
resumes; no unverified artifact is loaded; user sees progress/errors.

**Pitfalls:** redirects, proxies, TLS, file locking on Windows, disk space.

### K10 — Pluggable extractors

**Goal:** add formats without recompiling, safely.

**Implement**
- Config `extractors: [{name, extensions, mime, command, args, timeout_ms,
  max_output_bytes, enabled}]`.
- Runner: `Command` without a shell; no env inheritance beyond a safe allowlist;
  cwd = file's dir or a temp dir; capture stdout only; enforce timeout and
  output cap; kill the process group on timeout.
- Disabled by default; no plugins shipped. Require explicit user consent and
  show the command before enabling.
- Output becomes content text; failures are `skipped(reason)` and surfaced.
- Tests: echo/cat plugin, timeout kill, non-zero exit, output cap, filename
  injection, disabled-by-default.

**Acceptance:** a new format can be added via config; hung plugin is killed;
no shell injection; no plugin runs unless enabled.

**Pitfalls:** process groups on Windows, encoding, binary output, security.

### K11 — Search-result grouping

**Goal:** collapse near-identical screenshots/versions without merging distinct
pages.

**Implement**
- At write time, compare a new image with settled images in a 10-minute window:
  text line overlap, movement (scrolling), and thumbnail dHash. Chain groups;
  store `burst` = lowest id; `look` = dHash.
- Backfill old rows at idle, 50 at a time.
- Search collapses results sharing `burst`, showing a count/stack; `/dupes`
  reuses the same data.
- Rules must keep distinct pages (shared sidebar, different content) apart.

**Acceptance:** a screenshot burst is one row; distinct pages stay separate;
grouping costs < 0.1 ms/keystroke; tests on a labeled fixture.

**Pitfalls:** false merges, time windows, concurrent writes, old rows.

### K12 — Index-format spike (timeboxed)

**Goal:** decide Tantivy vs SQLite FTS5 trigram with numbers, not vibes.

**Implement**
- Synthetic corpus generator: 1M names + text; query mix: name, substring,
  BM25, filters, folded.
- Benchmark: index time, index size, p50/p95, indexer RSS, query RSS.
- Compare: current Tantivy, SQLite FTS5 trigram, Tantivy + folded field.
- Write `docs/decisions/index-format.md` with the numbers and a recommendation.
- Do not change production code in this task.

**Acceptance:** decision memo with reproducible commands; no production change.

### K13 — Native quick-search window (spike)

**Goal:** decide whether a native summoner beats the Tauri webview on memory and
latency, after the daemon exists.

**Implement**
- Minimal GPUI/Slint/egui window that connects to the daemon, runs a query, shows
  results, and hides/shows on a hotkey.
- Measure: hidden RSS, summon latency, search p50, build complexity.
- Compare with the Tauri window; write `docs/decisions/quick-window.md`.
- Keep Tauri for settings unless the native path clearly wins.

**Acceptance:** decision memo + runnable prototype; no production switch.

### K14 — Quantize the static embedding table to i8

**Goal:** reduce the ~30 MB f32 table to ~8 MB with no meaningful quality loss.

**Implement**
- Store `Vec<i8>` + per-row (or global) scales; dequantize while accumulating in
  `embed/table.rs`.
- Keep the f32 path behind a config flag for one release.
- Compare semantic recall/MRR vs f32 on the golden set; report memory and p50.
- Update model metadata/version if the vector space changes (it should not).

**Acceptance:** table ≤ 8 MB; recall within 1% of f32; query p50 unchanged;
tests pass; no new dependency.

## 6. Verification commands

Run the narrowest command first, then the full set for each task.

```bash
# format only touched files (avoid reformatting the repo)
rustfmt --edition 2024 --check <files>

# type-check / compile
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo check --manifest-path cli/Cargo.toml

# tests
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml

# lints
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# frontend (if UI changed)
bunx tsc --noEmit   # or npx tsc --noEmit

# benchmarks (use --release only when a task needs published numbers)
cargo run --manifest-path src-tauri/Cargo.toml --example bench --release -- <dir>
cargo run --manifest-path src-tauri/Cargo.toml --example index_bench --release -- <dir> <n>
cargo run --manifest-path src-tauri/Cargo.toml --example crawl_bench --release -- <dir>
```

Notes:
- Release builds are large and slow. Check disk first (`df -h`). Clean
  `src-tauri/target` if needed, but warn that it removes build artifacts.
- The dev profile already optimizes dependencies; debug numbers are closer to
  release than before, but still label them.
- Never run benchmarks against the user's real index. Use temp dirs.

## 7. Report format (after every task)

```
Task: Kx — <name>
Status: done | partial | blocked
Commit: <hash>
Files: <paths>
Tests: <commands + results>
Numbers: <before/after, memory, latency>
Risks: <what could break>
Next: <task id>
```

## 8. Stop conditions

Stop and ask for a decision if:

- A task requires a destructive schema migration or resetting a real index.
- A task would add a new network dependency or a write-capable agent tool.
- A task's acceptance criteria conflict with the read-only/security rules.
- The repo is not clean or contains unrelated user changes.
- Disk space is insufficient for the required build/benchmark.
- You cannot verify a platform-specific change on the current OS; implement it
  behind a feature flag and report it as unverified.

## 9. First action

Start with **K2**. Do not start K1 until K2 is committed and its benchmark
numbers are recorded. After K2, proceed in the order table. Do not attempt to
implement multiple tasks in one commit.
