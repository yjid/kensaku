# Benchmarks

The in-tree benchmark measures the hot path of Kensaku: walking a directory
tree, building the in-memory entry list, and running fuzzy filename queries.
It deliberately skips SQLite/Tantivy indexing so the numbers isolate the code
that runs on every keystroke.

## Running

```bash
# from the repository root
cargo run --manifest-path src-tauri/Cargo.toml --example bench --release -- ~/Projects

# or from src-tauri/
cargo run --manifest-path src-tauri/Cargo.toml --example bench --release -- ~/Projects
```

Always use `--release`. Debug builds are 10–50× slower and are not comparable
with the reference numbers. Point `<dir>` at a corpus of the desired size; the
100k reference numbers below were collected from a tree of ~100,000 files.

The harness uses `walker::walk_directories`, so hidden entries and the default
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

## Reference numbers (100k files)

Measured on Linux, release build, warm page cache, local SSD:

| Phase | Result |
| --- | --- |
| Walk | ~190 ms per 100k files (~525k files/s) |
| Fuzzy search | ~2.8 ms average per query (p50) |
| Memory | 128 B per entry (~12.8 MB per 100k files) |

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
