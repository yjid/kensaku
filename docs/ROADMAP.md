# Roadmap — landscape-derived work

> Execution plan derived from [LANDSCAPE.md](LANDSCAPE.md) (screenshot-search teardown +
> competitive analysis). [REMAINING.md](REMAINING.md) is the full backlog;
> this file is the ordered, testable slice we should work through next.
>
> Status: proposed. Update the checkboxes as tasks land.

## Principles

1. **Daemon owns the index.** The GUI, CLI, and MCP are clients. Indexing and
   decoding must never share a process with the webview.
2. **Memory is a feature.** Every index tier has a byte budget; work is lazy;
   freed memory is returned to the OS.
3. **One extraction pass, one OCR per file.** Text feeds lexical + semantic
   indexes together.
4. **Exact before fuzzy before semantic.** Add tolerance deliberately, not by
   letting weak matches pollute results.
5. **Measure on a frozen corpus.** Every task below has an acceptance number.

## P0 — foundation

### K1. Daemon split (epic)

**Why:** Spacedrive, fsearch, and ck all separate index ownership from
the UI. Our WebView currently sits in the indexing process and every CLI/MCP
session reloads metadata and vectors.

**Work**
- [ ] K1.1 New `kensaku-daemon` binary/crate. It owns `IndexerManager`, SQLite,
  Tantivy, vectors, watcher, and the embedding queue.
- [ ] K1.2 IPC: newline JSON-RPC over a Unix socket (`$XDG_RUNTIME_DIR/kensaku.sock`,
  0600) and a Windows named pipe; optional loopback TCP for MCP clients later.
  Version the protocol and include a token in the handshake.
- [ ] K1.3 Clients: Tauri app connects instead of constructing `IndexerManager`;
  `kensaku-cli` and MCP connect; the app auto-spawns the daemon if missing; CLI
  falls back to read-only direct access if the daemon is absent.
- [ ] K1.4 Lifecycle: single-instance lock, PID/status file, graceful shutdown,
  `SIGTERM` handling, log to config dir, no zombie writers.
- [ ] K1.5 Migration: keep the existing in-process path behind a feature flag
  until the daemon passes the acceptance tests.

**Acceptance**
- Closing the GUI does not stop indexing; reopening reconnects to the same crawl.
- `kensaku-cli state` answers in < 50 ms without loading metadata/units.
- One writer only; a second daemon exits cleanly.
- Socket is 0600; no network listener by default.
- Existing tests pass; new IPC tests cover version mismatch and auth failure.

**Files:** `src-tauri/src/lib.rs`, `commands.rs`, `indexer/mod.rs`,
`cli/src/main.rs`, `agent/mcp.rs`, new `daemon/`.

### K2. Compact metadata arena

**Why:** `Vec<FileEntry>` + `HashMap<PathBuf, usize>` costs ~128 B/entry plus a
duplicate path string. Spacedrive targets ~50 B/entry; fsearch's daemon is
21–35 MB for 8M names.

**Work**
- [x] K2.1 `NodeArena`: parent id, name offset/len, size, mtime, dev, ino,
  extension id, flags, bloom — contiguous, no per-node `PathBuf`.
- [x] K2.2 `NameArena`: one byte buffer per folder; resolve a full path only for
  displayed results and filters.
- [x] K2.3 Extension interning (`u16`) and a compact open-addressed path index
  (8 bytes/slot) instead of `HashMap<u64, u32>` + `HashMap<PathBuf, u32>`.
- [x] K2.4 Rebuild the scope trie from the arena; keep the generation counters.
- [x] K2.5 Migration/loader from SQLite; keep `FileEntry` as a temporary view
  for search/watcher until they are ported.

**Acceptance**
- 1M synthetic entries: metadata RSS ≤ 100 MB (stretch: 64 MB).
- Filename p50 at 100k stays ≤ 2.8 ms; at 1M ≤ 15 ms with the existing prefilter.
- Incremental updates of 10k files do not regress.
- All existing indexer/watcher tests pass.

**Files:** `indexer/metadata.rs`, `search/fuzzy.rs`, `query/scope_trie.rs`,
`indexer/watcher.rs`.

### K3. Thumbnail-based hashing + memory release

**Why:** Hashing a 480 px thumbnail takes ~2.4 ms; our backfill still decodes
full images and holds the buffers. `malloc_trim`/`malloc_zone_pressure_relief`
returns pages between images.

**Work**
- [x] K3.1 Generate the 480 px thumbnail once per image (during or just after
  extraction) and compute the dHash from it.
- [x] K3.2 Reuse the thumbnail for the grid/preview; never decode twice.
- [x] K3.3 `release_memory()` after each image/PDF: `malloc_trim(0)` (glibc),
  `malloc_zone_pressure_relief` (macOS), `EmptyWorkingSet`/`SetProcessWorkingSetSize`
  (Windows).
- [x] K3.4 Reject legacy 320 px thumbnails for hashing and regenerate them;
  bump `image_hash_version` to 2 so mixed hash spaces cannot occur.
- [ ] K3.4 Keep the backfill one-decode-at-a-time and pausable.
- [ ] K3.5 If a thumbnail already exists, hash it without decoding the original.

**Acceptance**
- 1,400 images: hashes complete without any full-resolution decode; peak
  indexer RSS ≤ 100 MB.
- Daemon RSS returns to baseline within 5 s after an image/PDF batch.
- Thumbnail cache hit avoids a source decode.
- `/dupes` still groups near-identical images.

**Files:** `indexer/extract/image.rs`, `indexer/mod.rs`, new `util/memory.rs`.

### K4. Lazy snippets and line highlighting

**Why:** Broad queries stay at 1–10 ms when matched lines are fetched only
for visible tiles (60–90 ms → 1–10 ms). Our result payload still carries
snippets for the ranked set.

**Work**
- [ ] K4.1 Search returns result metadata only (path, score, match type, range).
- [ ] K4.2 GUI requests snippets for visible rows, debounced/batched; cache by
  `(path, mtime, query)`.
- [ ] K4.3 MCP `read_snippet` stays the primitive; add optional line boxes.
- [ ] K4.4 Keep a small LRU for the last N visible snippets.

**Acceptance**
- A 2-character query matching ~2,000 files: p95 ≤ 20 ms to first paint.
- No file read/extraction for off-screen results (instrumented).
- Snippet text and highlight ranges match the current build.

**Files:** `search/mod.rs`, `commands.rs`, `src/components/*`, `agent/mcp.rs`.

## P1 — search quality and scheduling

### K5. Folded / trigram content tier

**Why:** Trigram + folded indexing makes `nutsmp` find
`donutsmp.net` and tolerates OCR look-alikes (`0/o`, `1/l/i/|`, `5/s/$`,
`8/b`, `m/rn`, `w/vv`, `d/cl`). fsearch uses trigram content matching. Our
`re:` scans but is not a fast tier.

**Work**
- [x] K5.1 Add a folded content field (normalized text) indexed with an ngram /
  trigram tokenizer, or a sidecar FTS5 table.
- [x] K5.2 Search order: exact terms → folded candidates verified against raw
  text (1 misread in 4–7 letters, 2 in 8+, none under 4) → semantic.
- [x] K5.3 Keep the current `re:` path for power users; folded search must never
  produce more false positives than the exact tier.
- [x] K5.4 Schema/migration and index rebuild plan.

**Acceptance**
- `nutsmp` finds `donutsmp.net`; `0RDER` finds `ORDER`.
- `email` does not match inside `internally` after folding.
- Folded tier only runs when exact results do not fill the limit.
- p95 for a broad query stays within the K4 budget.

**Files:** `indexer/content.rs`, `search/mod.rs`, migrations.

### K6. Optional high-accuracy OCR pack

**Why:** A public OCR bake-off found Tesseract missed every small sidebar label,
while PaddleOCR PP-OCRv6 read nearly everything. We keep Tesseract as the fast
default and offer the accurate path.

**Work**
- [ ] K6.1 Add an OCR engine abstraction (`Tesseract`, `Paddle`, `Native`).
- [ ] K6.2 PaddleOCR PP-OCRv6 tiny detector + small recognizer via `ort`
  `load-dynamic`, models + ORT downloaded with SHA-256 (22 MB + 24 MB).
- [ ] K6.3 Native OCR where available: macOS Vision, Windows OCR; lower memory
  than Paddle, best platform integration.
- [ ] K6.4 Setup/Settings pack: opt-in, checksum, disk estimate, fallback to
  Tesseract when missing.
- [ ] K6.5 Frozen 100-image UI-text fixture for recall and memory.

**Acceptance**
- On the frozen UI-text set, recall ≥ 95% of words (vs Paddle reference).
- Pack off: no new dependency, no memory cost, current Tesseract behavior.
- Peak per worker documented; no OCR process left behind on cancel.
- License/attribution for models recorded.

**Files:** `indexer/extract/image.rs`, `models.rs`, new `ocr/`, setup UI.

### K7. Scheduling and watcher robustness

**Why:** The indexer runs at idle priority, paces on battery, pauses,
settles writes for 400 ms, and retries files still being written. Our crawl can
compete with typing and read half-written files.

**Work**
- [x] K7.1 Priority: `SCHED_IDLE` + idle I/O (Linux), `PRIO_DARWIN_BG` (macOS),
  background mode (Windows), behind a "full speed / balanced / eco" setting.
- [x] K7.2 Battery detection and pacing; pause/resume in Settings.
- [x] K7.3 Newest-first within each type (keep text-first for first scan).
- [x] K7.4 400 ms settle; retry files written within 10 s; rename pairing and
  overflow rescan hardening.
- [x] K7.5 Fresh files jump the queue even during a backfill.

**Acceptance**
- During a full crawl, keystroke p95 ≤ 40 ms (10k-file corpus).
- Eco mode on battery: crawl proceeds at a bounded rate; fresh files still index
  first.
- Save/rename/delete tests: no partial files, no ghosts.
- Settings persist and take effect without restart.

**Files:** `indexer/mod.rs`, `indexer/watcher.rs`, `config.rs`, platform modules.

## P2 — agent and ecosystem

### K8. MCP v2

- [x] `reindex`, `health_check`, extended `index_state`, bounded `regex_search`
  (tree-sitter `find_symbol` deferred), pagination with cursors, per-tool token
  budgets.
- [x] Daemon-backed session: no per-session metadata/vector load.
- [x] Contract tests + manual MCP stdio check (Cursor/Claude Code were not
  installed; the wire protocol and tool calls were exercised directly).

**Acceptance:** MCP search p95 < 50 ms warm; pagination beyond 10k without OOM;
read-only guarantees preserved.

### K9. Model and download hygiene

- [~] SHA-256 pinning framework for model/OCR/ORT downloads: `.part` files,
  HTTP Range resume, mirror fallback, atomic rename, and checksum verification
  are implemented and tested. Production artifact checksums still need to be
  pinned from a trusted source.
- [~] One pack manifest shared by setup, Settings, and Doctor: the semantic
  pack manifest exists; OCR/ORT packs still need manifests.

**Acceptance:** corrupt download fails loudly and retries; interrupted download
resumes; no unverified artifact is loaded.

### K10. Pluggable extractors

- [x] External command registry keyed by MIME/extension, with timeout, output
  cap, allowlist, cleared environment and process-group kill. An OS-level
  sandbox remains future work; only explicitly configured absolute executables
  run.
- [x] Ship zero plugins by default; [docs/EXTRACTORS.md](EXTRACTORS.md) carries
  an optional pack example.
- [x] Document the plugin contract; never execute a plugin without user consent.

**Acceptance:** a new format can be added without recompiling Kensaku; a hung
plugin is killed and marked failed; no shell injection from file names.

### K11. Search-result grouping

- [x] Group near-identical screenshots/versions at write time (text overlap +
  thumbnail dHash, like screenshot bursts); collapse in results with a count.
- [x] Reuse the existing `/dupes` data; do not merge distinct pages that share a
  sidebar.

**Acceptance:** a screenshot burst is one row; distinct pages stay separate;
grouping costs < 0.1 ms per keystroke.

### K12. Index-format spike (timeboxed)

- [x] Benchmark Tantivy vs SQLite FTS5 trigram on the 1M corpus for name,
  substring, BM25, and memory.
- [x] Decision memo: keep Tantivy, add a sidecar trigram, or replace.
- [x] Do not change the index without the numbers.

### K13. Native quick-search window (spike)

- [ ] Prototype a GPUI/Slint/egui summoner against the daemon; measure hidden
  RSS and summon latency.
- [ ] Compare with the Tauri window (current baseline). Keep Tauri for settings
  if the native path wins.

**Decision:** native UI is an option, not a commitment. The daemon split must
land first either way.

### K14. Quantize the static embedding table to i8

- [x] Store the static table as i8 with per-row (or global) scales instead of
  f32; dequantize into the accumulator at query time.
- [x] Compare semantic recall/MRR against the f32 table on the golden set.
- [x] Keep the f32 table behind a config flag for one release.

**Acceptance:** table memory ≤ 8 MB (from ~30 MB); semantic recall within 1 % of
f32; query p50 unchanged; no new dependency.

## Order and effort

| Order | Task | Effort | Depends on |
|---|---|---|---|
| 1 | K2 metadata arena | 1–2 wk | — |
| 2 | K1 daemon split | 2–3 wk | K2 (preferred) |
| 3 | K3 thumbnail hash + memory | 1 wk | K1 |
| 4 | K4 lazy snippets | 1 wk | K1 |
| 5 | K7 scheduling + watcher | 1 wk | K1 |
| 6 | K5 folded/trigram tier | 1–2 wk | K4 |
| 7 | K6 optional OCR pack | 2 wk | K1 |
| 8 | K8 MCP v2 | 1 wk | K1 |
| 9 | K9 download hygiene | 3–5 d | — |
| 10 | K10 pluggable extractors | 2 wk | K1 |
| 11 | K11 result grouping | 1 wk | K3 |
| 12 | K12 format spike | 3 d | — |
| 13 | K13 native UI spike | 1 wk | K1 |
| 14 | K14 i8 embedding table | 3 d | K2 |

## Definition of done (every task)

- [ ] Tests: unit + the relevant integration/benchmark fixture.
- [ ] Numbers recorded in `docs/BENCHMARKS.md` (before/after).
- [ ] Docs updated (`ARCHITECTURE.md`, `REMAINING.md`, this file).
- [ ] `cargo clippy --all-targets -- -D warnings` and `tsc --noEmit` pass.
- [ ] No new network call by default; no write capability in the agent surface.
- [ ] Changelog entry under `[Unreleased]`.
