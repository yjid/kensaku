# Roadmap — landscape-derived work

> Execution plan derived from [LANDSCAPE.md](LANDSCAPE.md) (screenshot-search teardown +
> competitive analysis). [REMAINING.md](REMAINING.md) is the full backlog;
> this file is the ordered, testable slice we should work through next.
>
> Status: K1-K17 are shipped (K17 unmeasured on a real model). K18-K20 are
> deferred/beta-only, K21 is blocked, and K22 is shipped as a non-visual golden
> set. See [INTELLIGENCE_VISION_PLAN.md](INTELLIGENCE_VISION_PLAN.md) for the
> detailed plan and status table.

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

## Status (current)

| Task | Status |
|---|---|
| K1 | Shipped — daemon, Unix socket + Windows named pipe, clients, metadata RPCs, snippets; release packaging in F |
| K2 | Shipped — 92.8 MB at 1M (target ≤ 100 MB) |
| K3 | Shipped — legacy 320 px fix, thumbnail hashing, one-decode backfill, thumbnail-only hashing |
| K4 | Shipped — visible-only snippets, daemon read_snippet, line ranges, LRU |
| K5 | Shipped — folded tier, OCR-lookalike verification, `internally` negative case |
| K6 | Shipped — PP-OCRv6 pack, parity with Tesseract on our fixture; native platform OCR in B |
| K7 | Shipped — default mode now `full`; measurement pending |
| K8 | Shipped |
| K9 | Shipped — Potion and PP-OCRv6 packs pinned, mirrored, resumed, disk-checked |
| K10 | Shipped |
| K11 | Shipped |
| K12 | Shipped |
| K13 | Spike shipped — egui summoner: 8.6 ms first UI, 223 MB hidden RSS (debug) vs Tauri 350 MB; no product integration |
| K14 | Shipped — real Potion artifacts validated: 7.56 MB i8 matrix, MRR 1.0 vs f32 1.0 |
| K15 | Shipped — CJK-aware Tantivy tokenization, schema marker v2, 9.73x index size on CJK fixture |
| K16 | Shipped — embedding provider contract, model-id isolation, doctor/stats reporting |
| K17 | Shipped — optional ONNX smart pack code path; real-model quality not measured yet |
| K18 | Beta only — Settings → Beta can download the pinned CLIP pack; model download/status is implemented, search integration is not |
| K19 | Deferred — visual RRF plumbing exists but is not wired; mock encoder is test-only |
| K20 | Deferred — command/daemon/MCP plumbing exists but returns "not available"; hidden from MCP when unavailable |
| K21 | Blocked — native OCR was not implemented; fake macOS/Windows paths were removed. Scanned-PDF image-stream OCR is partial |
| K22 | Shipped — 43 non-visual golden queries; Recall@5 1.0, MRR 0.988; visual cases skipped until a real encoder exists |

## P0 — foundation

### K1. Daemon split (epic)

**Why:** Spacedrive, fsearch, and ck all separate index ownership from
the UI. Our WebView currently sits in the indexing process and every CLI/MCP
session reloads metadata and vectors.

**Work**
- [x] K1.1 New `kensaku-daemon` binary/crate. It owns `IndexerManager`, SQLite,
  Tantivy, vectors, watcher, and the embedding queue.
- [x] K1.2 IPC: newline JSON-RPC over a Unix socket (`$XDG_RUNTIME_DIR/kensaku.sock`,
  0600) and a Windows named pipe; optional loopback TCP for MCP clients later.
  Version the protocol and include a token in the handshake. Both transports use
  `interprocess` local sockets; Windows packaging is release work in section F.
- [x] K1.3 Clients: Tauri app connects instead of constructing `IndexerManager`;
  `kensaku-cli` and MCP connect; the app auto-spawns the daemon if missing; CLI
  falls back to read-only direct access if the daemon is absent.
- [x] K1.4 Lifecycle: single-instance lock, PID/status file, graceful shutdown,
  `SIGTERM` handling, log to config dir, no zombie writers.
- [x] K1.5 Migration: keep the existing in-process path behind a feature flag
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
- [x] K3.4 Keep the backfill one-decode-at-a-time and pausable.
- [x] K3.5 If a thumbnail already exists, hash it without decoding the original.

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
- [x] K4.1 Search returns result metadata only (path, score, match type, range).
- [x] K4.2 GUI requests snippets for visible rows, debounced/batched; cache by
  `(path, mtime, query)`.
- [x] K4.3 MCP `read_snippet` stays the primitive; add optional line boxes.
- [x] K4.4 Keep a small LRU for the last N visible snippets.

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

### K6. Optional PP-OCRv6 OCR pack

**Why:** A public OCR bake-off found Tesseract missed small sidebar labels while
PaddleOCR PP-OCRv6 read nearly everything. Our own fixture shows parity, so
Tesseract stays the fast, smaller default and PP-OCRv6 is a slower alternative
the user can choose.

**Work**
- [x] K6.1 Add an OCR engine abstraction (`Tesseract`, `Paddle`, `Native`).
- [x] K6.2 PaddleOCR PP-OCRv6 tiny detector + small recognizer via `ort`;
  models downloaded with SHA-256 (~1.7 MB + 4.3 MB + 27 KB).
- [~] K6.3 Native OCR where available: macOS Vision, Windows OCR; lower memory
  than Paddle, best platform integration. (Deferred to the platform-native OCR
  backlog item; the Paddle pack is shipped.)
- [x] K6.4 Setup/Settings pack: opt-in, checksum, disk estimate, fallback to
  Tesseract when missing.
- [x] K6.5 Frozen 104-image UI-text fixture for recall and memory.

**Acceptance**
- On the frozen UI-text set, recall ≥ 95% of words (vs Paddle reference).
- Tesseract and PP-OCRv6 are presented as alternatives; only the selected
  engine's pack is offered in setup and Settings.
- Pack off: current Tesseract behavior; the engine is compiled but no pack is
  downloaded.
- Peak per worker documented; no OCR process left behind on cancel.
- License/attribution for models recorded.

**Measured:** PP-OCRv6 tiny pack, 104/104 lines exact; Tesseract 5.5.3 also
scores 104/104 on the same fixture, so it does not demonstrate the claimed
small-sidebar advantage. Pack size ~6.1 MB. The `accurate-ocr` feature is
compiled into default app builds; the model pack itself remains an explicit
download, and Tesseract stays the default until the user selects PP-OCRv6.
Minimal builds can use `--no-default-features`.

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

- [x] SHA-256 pinning for model/OCR/ORT downloads: `.part` files, HTTP Range
  resume, mirror fallback, atomic rename, checksum verification, disk precheck,
  and pinned checksums for the three Potion + three PP-OCRv6 artifacts.
- [x] One pack manifest shared by setup, Settings, and Doctor: semantic and
  PP-OCRv6 manifests drive setup, Settings, and Doctor.

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

- [x] Prototype a GPUI/Slint/egui summoner against the daemon; measure hidden
  RSS and summon latency.
- [x] Compare with the Tauri window (current baseline). Keep Tauri for settings
  if the native path wins.

**Measured (debug builds, X11):** egui first UI 8.6 ms, daemon round-trip
46 ms, hidden RSS ~223 MB; Tauri baseline RSS ~350 MB. Native egui wins on
RSS in this build, but release parity is unmeasured. No product integration.

**Decision:** native UI is an option, not a commitment. The daemon split must
land first either way.

### K14. Quantize the static embedding table to i8

- [x] Store the static table as i8 with per-row (or global) scales instead of
  f32; dequantize into the accumulator at query time.
- [x] Compare semantic recall/MRR against the f32 table on the golden set.
- [x] Keep the f32 table behind a config flag for one release.

**Acceptance:** table memory ≤ 8 MB (from ~30 MB); semantic recall within 1 % of
f32; query p50 unchanged; no new dependency.

**Measured (real Potion artifacts):** 7,559,168-byte i8 matrix + 118,112 bytes
of scales; MRR 1.000 for both f32 and i8 on the labelled set.

## P3 — Intelligence & vision

Detailed steps, acceptance criteria, and risks live in
[INTELLIGENCE_VISION_PLAN.md](INTELLIGENCE_VISION_PLAN.md). This is the backlog
section B expansion; identity and VLM captions remain deferred.

| Task | Scope | Effort | Status |
|---|---|---|---|
| K15 | CJK-aware Tantivy tokenization | 1 wk | Shipped |
| K16 | Embedding provider contract (wire `semantic_model`) | 3-5 d | Shipped |
| K17 | Optional ONNX smart embedding pack | 2 wk | Code shipped; unmeasured |
| K18 | Image embedding extraction pipeline | 2 wk | Beta model download only; not wired |
| K19 | Visual search tier and RRF integration | 2 wk | Deferred; tier disabled |
| K20 | Similar-image lookup | 1 wk | Deferred; returns unavailable |
| K21 | Native macOS/Windows OCR | 1-2 wk | Blocked |
| K22 | Extend the golden query set with CJK/multilingual cases | 3-5 d | In progress |

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
| 15 | K15 CJK tokenization | 1 wk | — |
| 16 | K16 provider contract | 3-5 d | K15 recommended |
| 17 | K17 ONNX smart pack | 2 wk | K16 |
| 18 | K18 image embedding pipeline | 2 wk | K17 |
| 19 | K19 visual search tier | 2 wk | K18 |
| 20 | K20 similar images | 1 wk | K19 |
| 21 | K21 native OCR | 1-2 wk | — (platform) |
| 22 | K22 multilingual golden queries | 3-5 d | K15-K17 + section D set |

## Definition of done (every task)

- [ ] Tests: unit + the relevant integration/benchmark fixture.
- [ ] Numbers recorded in `docs/BENCHMARKS.md` (before/after).
- [ ] Docs updated (`ARCHITECTURE.md`, `REMAINING.md`, this file).
- [ ] `cargo clippy --all-targets -- -D warnings` and `tsc --noEmit` pass.
- [ ] No new network call by default; no write capability in the agent surface.
- [ ] Changelog entry under `[Unreleased]`.
