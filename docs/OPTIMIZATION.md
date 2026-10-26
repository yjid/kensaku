# Kensaku — Optimization & Correctness Pass

> **Historical document.** This was a code-review plan written before the
> current v0.3.x hardening cycle. Several items below have since shipped (see
> [REMAINING.md](REMAINING.md) for the live backlog and
> [BENCHMARKS.md](BENCHMARKS.md) for current numbers); treat the code references
> as historical, not as a description of today's implementation.
>
> Source: deep code review of post-PR1–5 codebase (see `docs/PROGRESS.md`).  
> Goal: make search faster, more correct, and more relevant. Ordered by impact.

---

## Phase A — Correctness (must fix before anything else)

### A1. Stale semantic units on re-embed
- **Problem:** When a file changes, `upsert` resets `indexed_gen=0` but old units are never deleted. The embed queue appends new units → duplicate/stale vectors corrupt ranking forever.
- **Fix:** In `metadata_db.rs` upsert path (or in the embed queue before enqueuing), call `delete_units_for_node(node_id)` + `UnitStore::remove_units_for_node(node_id)`. Or key units by `content_hash` and rebuild only when hash changes.
- **Files:** `metadata_db.rs`, `embed/units.rs`, `embed/queue.rs`, `indexer/mod.rs`

### A2. Enable foreign keys + cascade clear
- **Problem:** `PRAGMA foreign_keys` never enabled, so `ON DELETE CASCADE` on units never fires. `MetadataStore::clear()` deletes only `nodes`, leaving orphan `units`/`usage` rows and a populated `UnitStore`.
- **Fix:** `PRAGMA foreign_keys=ON` on connection open. Make `clear()` also `DELETE FROM units`, `DELETE FROM usage`, and `UnitStore::clear()`.
- **Files:** `metadata_db.rs`, `metadata.rs`

### A3. `panic = "abort"` defeats `catch_unwind`
- **Problem:** Release profile has `panic = "abort"` (`Cargo.toml:43`). The `catch_unwind` in `pdf.rs:14` is a no-op in production — a lopdf panic aborts the entire app.
- **Fix:** Either remove `panic = "abort"` (accept slightly larger binary) or run extraction in a subprocess with timeout. Subprocess is safer for untrusted PDFs.
- **Files:** `Cargo.toml`, `extract/pdf.rs`, `extract/mod.rs`

### A4. Parsed-but-ignored grammar features
- **Problem:** `re:`, `name:`, `content:`, `path:` terms and `mode:exhaustive` are parsed but silently treated as plain terms. Users get wrong results with no error.
- **Fix (Option 1):** Implement them — `re:` as Tantivy regex query, `name:`/`content:`/`path:` as field-scoped terms, `mode:exhaustive` as paginated full scan.
- **Fix (Option 2):** Remove from grammar/help until implemented.
- **Recommended:** Option 1 for `name:`/`content:`/`path:` (easy, high value). Defer `re:` and `mode:exhaustive` to a later phase with a TODO.
- **Files:** `query/parser.rs`, `search/mod.rs`, `content.rs`

---

## Phase B — Biggest latency wins (P0 impact)

### B1. Eliminate per-query FileEntry clones
- **Problem:** `search/mod.rs:156-159` deep-clones every candidate `FileEntry` (PathBuf + String + Option) per keystroke.
- **Trick:** Add `FuzzySearcher::search_indices(query, entries: &[FileEntry], candidate_indices: &[usize], limit) -> Vec<usize>`. Return slot indices, not owned entries. Resolve paths only for the final top-`limit`.
- **Impact:** Eliminates ~50-200 allocations per query. Likely the single biggest win.

### B2. Replace `Vec::contains` with bitmap membership
- **Problem:** `search/mod.rs:192,260,286` call `candidate_indices.contains(&orig_idx)` inside merge loops → O(candidates × hits).
- **Trick:** Use a `Vec<bool>` or `roaring::RoaringBitmap` as a membership bitmap. O(1) lookup. Key `bm25_ranks`, `semantic_ranks`, and `candidates` by `usize` entry index. Resolve `PathBuf` only for final results.
- **Impact:** O(N) → O(1) per hit lookup.

### B3. Precompile filters at parse time
- **Problem:** `search/mod.rs:451-456, 463, 471, 534, 537` allocate `to_lowercase()` per entry per filter per query.
- **Trick:** Parse filters into a normalized struct at query time (once): intern extensions, pre-lowercase path/name fragments, build type bitsets. Evaluate cheap filters first (`is_dir`, `ext` bitset) before string filters. Cache the compiled filter for the query string (LRU, 16 entries).
- **Impact:** Eliminates 5-10 allocations per entry per query.

### B4. Push filters into engines
- **Problem:** Tantivy and semantic scanner search the full corpus, then results are rejected by `candidate_indices`. Filters are applied post-hoc.
- **Trick:** Accumulate `(scope_prefix, filters)` into a Tantivy `BooleanQuery` with `PathPrefix` and term filters. In `ExactScanner::scan`, pass a candidate mask or skip non-candidate nodes. This avoids searching 1M docs when only 4k are in scope.
- **Impact:** 10-100× fewer docs scored in Tantivy. Semantic scan skips 99% of units.

### B5. Stop rebuilding trie on every file add
- **Problem:** `metadata.rs:132-133` bumps generation on every `add()`. `scope_trie.rs` rebuilds the entire trie on any generation change. During crawling, every query invalidates the trie.
- **Trick:** Only bump a separate `dir_generation` when a *directory* entry changes (not files). Build the trie lazily outside the read lock. Cache it in an `Arc<Trie>` behind an `AtomicU64` generation check.
- **Impact:** Trie rebuilds only when directories change (rare), not on every file.

### B6. Batch embed worker
- **Problem:** `embed/queue.rs:201-215` does per-unit DB insert + per-unit vector-store write lock. Up to 40 lock acquisitions per file.
- **Trick:** Fetch 16 nodes, one SQLite transaction for all inserts, one `indexed_gen` update per node, one `unit_store.write()` for the whole batch. Convert `i8→u8` with a single `unsafe` slice cast (or store `Vec<u8>` internally). Use `hash_cache` hit path without re-cloning (store `Arc<Vec<...>>`).
- **Impact:** 40 lock acquisitions → 2 per batch. 10-20× faster embedding throughput.

### B7. Index `indexed_gen` column
- **Problem:** `metadata_db.rs:313-350` does a full table scan for `get_nodes_for_embedding` every poll cycle.
- **Trick:** `CREATE INDEX IF NOT EXISTS idx_nodes_indexed_gen ON nodes(indexed_gen) WHERE indexed_gen = 0` (partial index). Query uses `WHERE indexed_gen = 0 ORDER BY modified DESC LIMIT 16`.
- **Impact:** O(N) → O(log N) per poll.

### B8. Snippet caching + avoid `to_lowercase`
- **Problem:** `content.rs:251-297` lowercases the entire 64KB content per result per query. Mutex held across disk I/O.
- **Trick:** Cache snippets on `(path, mtime)` in an LRU (200 entries). Release mutex before file I/O. Use `bstr::BStr` case-insensitive search or precompute a lowercase sidecar cached alongside content. Return `Cow<str>`.
- **Impact:** Eliminates 20 × 64KB lowercase per query. Snippet cache hits skip disk I/O entirely.

---

## Phase C — Indexing throughput

### C1. Parallelize crawl callback
- **Problem:** `indexer/mod.rs:161-209` runs extraction + Tantivy indexing on a single thread despite jwalk being parallel.
- **Trick:** Use jwalk's parallel iterator or a bounded worker pool (rayon, 4 threads) for `extract_document` + `add_document`. Channel results to a single Tantivy writer thread. Keep metadata upserts in a batched transaction every 100 files.
- **Impact:** 4× indexing throughput on multi-core.

### C2. Avoid `entry.clone()` + double lock acquisition
- **Problem:** `indexer/mod.rs:179` clones the entire `FileEntry` and acquires `metadata.write()` after already holding `metadata.read()` (`:166`).
- **Trick:** Take the write lock once per batch. Insert entries into a local Vec, then flush to metadata + DB in one transaction. No per-file clone.
- **Impact:** Eliminates N clones + 2N lock acquisitions during crawl.

### C3. Coalesce watcher events
- **Problem:** `watcher.rs:98` clones entire config per event. Extraction runs on the notify thread (`:282`), blocking event delivery.
- **Trick:** Debounce by path (150ms coalescing window). Read config once per batch. Move extraction to a bounded worker pool. Process events in batches of 50.
- **Impact:** Prevents event storms from freezing the watcher.

---

## Phase D — Extraction & UI

### D1. Stream zip XML instead of `read_to_string`
- **Problem:** `office.rs:13-14, 96-97, 162, 227-228, 304-305, 350-351` read entire XML entries into String before parsing. XLSX collects all shared strings before capping (`:159-191`).
- **Trick:** Use `BufReader` + streaming XML parser. Apply size caps during parsing (stop reading after N bytes). Cap shared strings at 1000 entries before pushing.
- **Impact:** 10-50× less memory on large Office documents.

### D2. Cache extraction results
- **Problem:** Every preview, embedding, and re-index re-parses the document from disk. PDFs re-parsed on every keystroke over a PDF result set.
- **Trick:** LRU cache keyed by `(dev, ino, mtime, size)` → extracted text + page count + metadata. 64 entries, 32MB cap. Reuse for preview, embedding, and Tantivy.
- **Impact:** Eliminates repeated PDF/Office parsing. Preview switches from 50ms to <1ms on cache hit.

### D3. Raise 100KB cap to 1MB
- **Problem:** `indexer/mod.rs:184` indexes only 100KB despite docs claiming 1MB. `content.rs:331` delegates to `extract_document` which caps at `min(max_bytes, 1MB)`.
- **Trick:** Pass `max_bytes = 1_048_576` in the crawl callback. Store per-page units for PDFs so snippets map to pages.
- **Impact:** 10× more content searchable. PDF snippets point to exact pages.

### D4. Frontend memoization + stale-while-revalidate
- **Problem:** `ResultItem` not memoized → all 50 cards re-render per keystroke. `PreviewPane.renderContentWithHighlights` runs on every render with a new `TextEncoder` per line. `isSearching` cleared for stale responses.
- **Trick:**
  - `React.memo(ResultItem)` with custom comparator (query + path).
  - Memoize `renderContentWithHighlights` and preview content by `(path, searchQuery, best_range)`.
  - Single `TextEncoder` instance (or use `line.length` for ASCII previews).
  - Stale-while-revalidate: keep previous results visible while new query runs. Only clear on empty query.
  - Raise debounce to 70ms on large corpora (adaptive based on last query latency).
- **Impact:** 5-10× fewer React re-renders per keystroke. No spinner flicker.

---

## Phase E — Smart filtering (user-facing)

### E1. Type-based pre-filtering
- **Problem:** `excluded_extensions` is dead code (no callers). Type categories exist but aren't used for pre-filtering.
- **Trick:** Wire `is_extension_excluded` into the walker (skip excluded extensions during crawl) and into the watcher. Add a `type:` filter that maps to extension sets (e.g., `type:code` → [rs, ts, js, py, go, ...]). Pre-filter candidates by extension bitset before fuzzy/BM25.
- **Impact:** `type:code` instantly excludes 80% of files. Crawl skips 30%+ of files.

### E2. Extension interning
- **Problem:** Extensions stored as `Option<String>` per entry → duplicate strings for every file.
- **Trick:** Intern extensions into a `Vec<String>` with `u16` indices. `FileEntry.ext: u16` instead of `Option<String>`. 2 bytes instead of 24+ bytes per entry.
- **Impact:** 6-12MB saved at 1M files. Faster filter comparison (integer equality).

### E3. Scope-aware candidate pruning
- **Problem:** Even with scope filters, the fuzzy matcher scans all candidates.
- **Trick:** When a scope is resolved, pre-compute the candidate index set (DFS range) and pass it to the fuzzy matcher. The matcher only scores entries in the scope — no post-filter rejection.
- **Impact:** Scoped queries (the common case) scan 100-1000 entries instead of 1M.

---

## Implementation order

1. **Phase A** (correctness) — do first, ~1 day. Fixes silently wrong results.
2. **Phase B** (latency) — biggest perceived speed improvement, ~2-3 days.
3. **Phase C** (indexing) — faster cold start and live updates, ~1-2 days.
4. **Phase D** (extraction/UI) — better previews and less re-parsing, ~1-2 days.
5. **Phase E** (filtering) — smarter search, less irrelevant results, ~1 day.

## Key files to touch

| Phase | Files |
|---|---|
| A | `Cargo.toml`, `metadata_db.rs`, `metadata.rs`, `extract/pdf.rs`, `query/parser.rs`, `search/mod.rs` |
| B | `search/mod.rs`, `search/fuzzy.rs`, `content.rs`, `metadata.rs`, `scope_trie.rs`, `embed/queue.rs`, `metadata_db.rs` |
| C | `indexer/mod.rs`, `indexer/walker.rs`, `watcher.rs`, `content.rs` |
| D | `extract/office.rs`, `extract/mod.rs`, `extract/text.rs`, `App.tsx`, `ResultItem.tsx`, `PreviewPane.tsx` |
| E | `config.rs`, `walker.rs`, `watcher.rs`, `search/mod.rs`, `metadata.rs` |

## Verification

After each phase:
- `cargo build` passes
- `cargo clippy -- -D warnings` (fix new warnings only)
- `npx tsc --noEmit` passes (if frontend changed)
- Manual: search latency < 15ms p50 at 100k files, zero ghost results after 100 renames