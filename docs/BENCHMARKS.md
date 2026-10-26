# Benchmarks

There are three in-tree harnesses:

- `bench` measures the hot path of Kensaku: walking a directory tree, building
  the in-memory entry list, and running fuzzy filename queries. It deliberately
  skips SQLite/Tantivy indexing so the numbers isolate the code that runs on
  every keystroke.
- `index_bench` measures crawl extraction: the same walk, image dimensions
  (plus optional perceptual hashing), and text/OCR extraction the indexer
  performs. It does not touch the real index.
- `crawl_bench` builds a throwaway index in an isolated config directory and
  runs a fresh crawl plus an incremental re-crawl, reporting counts, wall time,
  and peak RSS (Linux).

## Running

```bash
# query hot path
cargo run --manifest-path src-tauri/Cargo.toml --example bench --release -- ~/Projects

# crawl extraction (optionally cap the number of files)
cargo run --manifest-path src-tauri/Cargo.toml --example index_bench --release -- ~/Pictures 400

# full fresh + incremental crawl in a throwaway index
cargo run --manifest-path src-tauri/Cargo.toml --example crawl_bench --release -- ~/Pictures
```

Always use `--release`. Debug builds are 10–50× slower and are not comparable
with the reference numbers. Point `<dir>` at a corpus of the desired size; the
100k reference numbers below were collected from a tree of ~100,000 files.

The harnesses use `walker::walk_directories`, so hidden entries and the default
excluded directories (`node_modules`, `.git`, `target`, `dist`, …) are skipped.

## Warm vs cold

- **Cold**: first run after dropping the page cache (`sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'`
  on Linux) or after a reboot. The walk is dominated by disk metadata reads.
- **Warm**: subsequent runs with the corpus already in the page cache. This is
  the steady-state a user sees after Kensaku has indexed once.

The harness itself warms each query once (`FuzzySearcher::search` before the
timed loop), then takes 10 samples per query per process run. Report warm
numbers for search latency and cold numbers for the walk.

## What each phase measures

1. **Walk** (`walker::walk_directories`): directory traversal plus `stat`
   metadata (size, mtime, dev/ino) and char-bloom construction. Reported as
   total milliseconds, files walked, and files/second.
2. **Metadata store build**: skipped by design. Entries are used directly so
   the benchmark does not pay for SQLite writes.
3. **Fuzzy search**: eight fixed queries (`main`, `index`, `config`, `search`,
   `mod`, `content`, `metadata`, `watcher`) over every entry, top-50 results.
   Each query prints the average and best of 10 runs; the summary prints the
   average across queries. This covers the char-bloom prefilter, nucleo
   scoring, path/name weighting, and top-k selection.
4. **Memory estimate**: `size_of::<FileEntry>()` × entry count. This is the
   in-memory metadata footprint only; it excludes the path index, Tantivy, and
   SQLite.

## p50 / p95

The harness prints averages; percentile reporting is done by repeating the
whole run (each run is already aggregated over 10 samples) and taking
percentiles across runs. For example, 20 warm runs:

```bash
for _ in $(seq 1 20); do
  cargo run --manifest-path src-tauri/Cargo.toml --example bench --release -- ~/Projects \
    | awk '/Average across/ {print $(NF-1)}'
done | sort -n | awk '{a[NR]=$1} END {printf "p50 %.3f ms\np95 %.3f ms\n", a[int(NR*0.5)], a[int(NR*0.95)]}'
```

Use the same approach for the walk time (grep `Walk time`) when a percentile is
needed. Run on an otherwise idle machine; disable power saving and keep the
corpus on local storage.

## Indexing and extraction reference

The crawl is a single extraction pass: extracted text feeds both Tantivy and the
semantic index, so an image is OCR'd once rather than once per index. Extraction
concurrency is bounded (up to 8 workers, and each tesseract child is pinned to
one OpenMP thread), and the indexer commits through a bounded channel instead of
collecting every document in memory. Perceptual image hashing is **not** part of
the crawl: dimensions come from the header, and hashes are filled in afterwards
by a one-decode-at-a-time background pass used by `/dupes`.

Measured on the development machine (Linux, local SSD). **These runs used a
debug build**, so they are conservative; release builds are faster. Since the
`dev` profile now optimizes dependency code (`image`, Tantivy, PDF parsing),
`make dev` numbers are much closer to release than a plain debug build. OCR time
is dominated by the external tesseract process and is build-independent.

| Workload | Result |
| --- | --- |
| K2 metadata arena | Node size 64 bytes (vs 128-byte `FileEntry`); 1M entries ≈ 64 MB metadata (stretch target met) |
| K3 thumbnail hashing | dHash now reuses the 480 px JPEG thumbnail; `phash64_cached` decodes sources only when the thumbnail is stale. 1400-image RSS run still pending. |
| K5 folded/trigram tier | 100 docs × 6,899 B raw: 84,029 B → 135,318 B index (1.61x, includes stored raw text). Folded `invoice` query over 500 docs (debug): p50 172 µs, p95 262 µs. `nutsmp`→`donutsmp.net`, `0RDER`→`ORDER`, `email` does not match `internally`, and d/cl false positives are rejected by raw verification. |
| K7 scheduling + watcher | 400 ms settle and 500 ms fresh-file verification added; 6 watcher tests cover coalescing, fresh retries, rename, delete-while-paused, and overflow rescan. Keystroke p95 on a 10k-file crawl was not measured on this machine (no GUI corpus); priority is applied at crawl-thread start and can be disabled with `KENSAKU_NO_BG_PRIORITY=1`. |
| K8 MCP v2 | Warm MCP search over stdio + daemon on a 1-file index (debug): p50 37.4 ms, p95 39.8 ms over 60 calls (acceptance < 50 ms). Local read-only MCP fallback over a 100-file fixture: p50 5.9 ms, p95 6.7 ms. Cursor pagination is tested across 5 exhaustive results; offsets are capped at 10,000 with an explicit `cursor_limit_reached` flag. Read-only tools are listed by default; `reindex` only appears when `mcp_allow_reindex=true` and the daemon is running. |
| K10 external extractors | 19 tests: config round-trip/default-off, extension/MIME matching, relative-command rejection, odd filenames (no shell side effects), `{path}` as one argument, 150 ms timeout kills the process group, 1 KiB output cap, non-zero exit reporting, disabled-by-default enforcement, config validation, Doctor surfacing of commands and recent runtime failure reasons (recorded/cleared), a helper holding stdout after the parent exits is killed, and targeted pending marks. Integration test runs a configured plugin through `extract_document`; a manual daemon run indexed a configured `.k10ext` format on first scan and after a watcher-created file. Zero plugins by default. |
| K11 search-result grouping | 10 tests (4 burst-signature/labeled-fixture, 3 SQLite burst assignment/chain merge/leader detach, 1 image/non-image collapse, 1 end-to-end burst collapse, 1 ignored release benchmark). Labeled fixture keeps identical/scrolled screenshots together and distinct pages with shared chrome apart. Release grouping over 50 results (including cloning the result list): p50 6,252 ns, p95 6,502 ns (< 0.1 ms acceptance). Backfill processes 50 images per idle batch; `/dupes` reads the same burst rows. |
| K14 i8 embedding table | 29,528 × 256: f32 30,236,672 B (~28.8 MiB) → i8 + per-row scales 7,677,280 B (~7.3 MiB). Synthetic 96-row golden set: recall@5 1.000, MRR 1.000 (f32) / 1.000 (i8). 512-token `embed` p50 (debug): 1.227 ms f32 / 1.230 ms i8. Production model artifacts were not installed, so the real-corpus quality check remains to be run on a machine that has them. |
| 200 screenshots, OCR | ~12 s (~17 files/s) |
| 400 screenshots, OCR | ~26 s (~15 files/s) |
| Fresh crawl, 703 mixed files (400 images + 300 text), SQLite + Tantivy + embeddings | ~23–27 s |
| Warm re-crawl of the same tree | ~0.7 s |
| Indexer process peak RSS, text-only / mixed | ~73 MB / ~73 MB |

The mixed crawl used the fallback embedder; a real install adds the ~30 MB static
embedding table when the semantic pack is installed. Desktop RSS is dominated by
the OS webview and is not comparable with the indexer-process numbers above. The
1,400-image `~/Pictures` corpus used during development extrapolates to roughly
90 s in this build; a release build is faster, and text-only corpora finish in
milliseconds per thousand files.

## Reference numbers (100k files)

Measured on Linux, release build, warm page cache, local SSD:

| Phase | Result |
| --- | --- |
| Walk | ~190 ms per 100k files (~525k files/s) |
| Fuzzy search | ~2.8 ms average per query (p50) |
| Metadata footprint | 128 B per entry (~12.8 MB per 100k files) |

These are reference values for spotting regressions, not hard guarantees.
Hardware, filesystem, path length, and the query mix all move the numbers.

## 1M-file extrapolation caveat

The 1M numbers are **extrapolated, not measured**: ~1.9 s walk, ~28 ms fuzzy
search, ~128 MB of entry metadata. Linear scaling is optimistic:

- the path `HashMap` and entry vector outgrow CPU caches, so lookups and
  scoring become memory-bound;
- filesystem traversal is less cache-friendly at scale and varies wildly with
  directory shape;
- top-k selection over 1M candidates costs more than the prefilter saving.

Holding the < 15 ms search target at 1M files needs a stronger prefilter
(trigram/prefix index) or a sharded scan. Treat the 1M figures as a planning
budget until a real corpus is measured.
