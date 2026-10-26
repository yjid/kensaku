# Index-format decision: keep Tantivy

**Status:** accepted (K12, timeboxed spike)
**Date:** 2026-10-26
**Production impact:** none. This memo and its benchmark example do not change
the live index format.

## Decision

Keep Tantivy for the content tier and keep the K5 folded trigram field. Do not
replace Tantivy with SQLite FTS5 and do not add an FTS5 trigram sidecar.

The 1M-document run makes the trade-off clear:

| Backend | Index time | Index size | Build peak RSS | Query RSS | Name p50 / p95 | Substring p50 / p95 | BM25 p50 / p95 | Filter p50 / p95 | Folded p50 / p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Tantivy baseline | 4.8 s | 66.7 MB | 75.9 MB | 66.6 MB | 1.6 / 1.7 µs | 0.9 / 1.0 µs (0 hits) | 226 / 296 µs | 2.58 / 2.61 ms | 0.9 / 0.9 µs (0 hits) |
| Tantivy + folded | 17.0 s | 99.6 MB | 67.2 MB | 61.0 MB | 2.6 / 2.7 µs | 20.9 / 21.8 µs (10 hits) | 275 / 285 µs | 2.59 / 2.60 ms | 22.1 / 22.6 µs (10 hits) |
| SQLite FTS5 trigram | 22.2 s | 918.4 MB | 146.6 MB | 147.0 MB | 1,046 / 1,060 µs | 59 / 65 µs (10 hits) | 258,101 / 259,799 µs | 256,984 / 258,640 µs | 58 / 66 µs (10 hits) |

FTS5 is:

- **9.2× larger** than the folded Tantivy index and **13.8× larger** than the
  baseline;
- **1.3× slower to build** than folded Tantivy and **4.6× slower** than baseline;
- **~2.2–2.4× the query RSS**;
- **~900–1,100× slower** on broad BM25 and **~100× slower** on filtered BM25,
  because FTS5 ranks every match before applying `LIMIT` while Tantivy uses
  top-k collectors.

The folded field costs **32.9 MB (+49%)** and **12.2 s** of build time at 1M
documents in this isolation harness. It adds about 49 µs to the broad BM25
p50 (226 → 275 µs), keeps the exact tier sub-millisecond, and turns the
`nutsmp` lookalike query from zero hits into ten hits in 22 µs. That is the
intended K5 trade-off, and it is worth keeping.

A separate FTS5 trigram table is not justified as a sidecar either. It would
add roughly 0.9 GB and ~150 MB RSS at 1M documents while duplicating text
storage, and it is slower for the broad queries the desktop app runs on every
keystroke. If a future feature needs arbitrary substring matching over a small
metadata-only table, a bounded FTS5 table can be reconsidered in isolation; it
should not own the content tier.

## Method

Harness: `src-tauri/examples/index_format_bench.rs`.

The synthetic corpus is deterministic and generated in a streaming fashion
(no corpus is retained in memory):

- 1,000,000 filenames of the form `docNNNNNNN.{pdf,md,txt}`, with every third
  document in `pdf` for the filter query;
- ~220-byte text documents built from eight topics, shared report boilerplate,
  and two planted markers: `needle` every 1,000 docs and `donutsmp.net` every
  5,000 docs;
- paths under `/corpus/NN/`.

Every backend indexes the same generated stream with one writer thread and a
16 MB Tantivy arena. Tantivy commits every 100,000 documents and waits for
merges; SQLite uses one transaction per 100,000 rows, `synchronous=OFF`,
`journal_mode=OFF`, a 128 MB cache, and an FTS5 table with
`tokenize='trigram'`. The FTS5 table stores `path` and `ext` unindexed and
indexes `filename` and `content`.

Query mix (10 warmups, 100 timed iterations, p50/p95 reported):

| Label | Query |
| --- | --- |
| name | `doc0000001` on the filename field |
| substring | `eedl` (a substring of `needle`), content field; the folded backend uses its trigram field |
| BM25 | `quarterly revenue` on content, top 10 |
| filter | BM25 query plus `ext = 'pdf'`, top 10 |
| folded | `nutsmp` (a substring of `donutsmp`, not a whole token) |

Tantivy name/BM25 queries are parsed once before timing and folded queries are
built once from their trigrams; FTS5 statements are prepared once before timing.
This isolates execution cost rather than SQL/query-parser setup.
RSS is read from `/proc/self/statm`; build peak is sampled every 50,000
documents, and query RSS is read after the timed queries. Index size is the
on-disk directory size after commit/merge.

The Tantivy harness omits the production `content_raw` stored field so the
format comparison is not inflated by snippet storage. Production adds that
field; the K5 benchmark measured folded + stored-raw growth at 1.61× the prior
index. The folded configuration here is therefore a lower bound for production
index size, but even doubling it would stay far below FTS5's 918 MB.

## Reproduce

Build the example once, then run one backend per process (the harness removes
its scratch directory unless `KEEP=1`):

```bash
cargo build --release --manifest-path src-tauri/Cargo.toml \
  --example index_format_bench

for format in tantivy folded fts5; do
  ./src-tauri/target/release/examples/index_format_bench \
    "$format" 1000000 "/tmp/kensaku-k12-$format"
done
```

The `/tmp` filesystem must have at least ~1 GB free for the FTS5 run. Use
`KEEP=1` to inspect the generated indexes. The corpus and query strings are
fixed in the example, so the numbers are comparable across runs.

## Caveats

- This is a synthetic corpus with relatively uniform topic terms. Broad BM25
  result sets amplify FTS5's rank-everything behavior; a corpus with very
  selective query terms would narrow the gap. It would not erase the 9×
  index-size or 2× RSS gap, and broad terms are common in desktop search.
- Absolute timings include synthetic document construction, as both backends
  share that cost. Relative index-build differences are therefore slightly
  understated for FTS5.
- The benchmark uses a tmpfs on this machine; index time is not storage-bound.
- The decision is about the content tier at desktop scale. It does not rule out
  FTS5 for an unrelated, small, metadata-only feature.

## Acceptance

- [x] Synthetic 1M-document corpus and query mix.
- [x] Index time, index size, p50/p95, indexer RSS, and query RSS measured.
- [x] Tantivy baseline, Tantivy + folded field, and SQLite FTS5 trigram
  compared.
- [x] Reproducible commands recorded above.
- [x] No production code or index-format change.
