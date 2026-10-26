# Local search landscape

> Research date: 2026-10-26. Scope: how a production screenshot-search app works, and how the closest
> local-search / OCR / semantic products compare with Kensaku. Sources are
> linked at the end; performance figures are the projects' own claims unless
> marked otherwise.

## 1. Screenshot-search reference teardown

**What it is.** A screenshot-only search tool: it does not take screenshots, it
watches the folders your screenshot tool already writes to, OCRs every image,
and lets you search the text. Linux, macOS, Windows (preview).

### Pipeline

```
inotify / FSEvents / ReadDirectoryChangesW
  → 400 ms settle
  → OCR (PaddleOCR PP-OCRv6 via ONNX Runtime)
  → one SQLite row per screenshot + per-line boxes
  → SQLite FTS5 (trigram) for exact substring search
  → second folded FTS5 index for OCR look-alike near-matches
  → native search window queries the DB on every keystroke
```

### Index and search

- **One SQLite database**, FTS5 with the **trigram tokenizer**. One FTS row per
  screenshot (all lines joined by newlines), plus a `lines` table with text and
  bounding boxes for highlighting.
- Trigram matching gives **substring search**: `nutsmp` finds
  `donutsmp.net`, which matters because OCR is imperfect.
- **Near-match index**: text folded for OCR confusions (`0/o`, `1/l/i/|`,
  `5/s/$`, `8/b`, `m/rn`, `w/vv`, `d/cl`). Only searched when exact matches do
  not fill the limit; candidates are then checked against the raw text (1
  misread letter for 4–7 letter words, 2 for 8+, none under 4). This avoids
  `email` matching inside `internally`.
- Terms under 3 characters cannot use the trigram index and fall back to `LIKE`
  over rows already narrowed by the other terms.
- Filters: `in:`, `date:`, `before:`, `after:`. The parser treats anything else
  containing `:` as a literal term.
- **Bursts**: near-identical screenshots taken within 10 minutes are grouped at
  *write* time (line overlap + movement, or a 64-bit dHash of the thumbnail for
  text-poor shots). Results collapse by burst at query time.
- **Lazy highlighting**: matched lines are fetched only for visible tiles. A
  two-letter query can match 2,000 shots; fetching all lines took 60–90 ms vs
  1–10 ms for the ~30 visible ones.

### OCR

- PaddleOCR **PP-OCRv6 tiny detector + small recognizer**, ONNX Runtime loaded
  dynamically (`ort`, `load-dynamic`, API 27). Models 22 MB, ORT 24 MB,
  SHA-256 pinned, downloaded once.
- Tuned for screenshots: tiny detector (4.6× faster, 99.6% of words kept),
  downscale-only to ~1 MP (no upscaling), axis-aligned boxes, width-based
  recognition batching, single-character lines dropped as icons.
- OCR bake-off before writing Rust (four real screenshots, CPU only):
  - Tesseract 5.5: 0.4–0.9 s/shot, 40–75 MB, missed every small sidebar
    label (Home, Shorts, Like, Share) plus junk lines like `EERE`.
  - `ocrs` 0.13 (pure Rust): ~0.4 s, ~240 MB, fast but garbled.
  - PaddleOCR PP-OCRv6 small: 2–3.4 s, ~640 MB, read nearly everything
    including the URL bar and tiny labels.
- Optional Devanagari recognizer (PP-OCRv5), only applied to lines the default
  reader was unsure about or left gaps in.

### Runtime and scheduling

- The indexer/CLI and the window are separate processes. The app stays resident while hidden (37 MB) and is summoned over a
  Unix socket (Linux/macOS) or loopback TCP (Windows); summon ~120 ms.
- Indexer runs as a systemd user service / launchd agent / sign-in app, at
  `SCHED_IDLE` + idle I/O priority (`PRIO_DARWIN_BG` on macOS).
- **Newest-first backlog** (recent screenshots are most likely searched).
- On battery, one old screenshot every 3 s; fresh screenshots still jump the
  queue. Pausable from settings.
- 400 ms settle before reading; a file that fails within 10 s of being written
  is retried (browsers write then rename).
- `release_memory()` after each image: `malloc_trim(0)` on glibc,
  `malloc_zone_pressure_relief` on macOS.
- Inotify/FSEvents/Windows watcher, rename pairing, delete/rename handling,
  rescan on overflow, single-watcher lock + PID file.

### Numbers (project's own measurements)

| Metric | Value |
|---|---|
| Search, 5k+ screenshots | 1–10 ms/keystroke |
| Screenshot saved → searchable | 0.77 s |
| OCR, 25 screenshots, 4 cores | 0.67 s/shot, 278–311 MB peak |
| Full backfill, 5,159 screenshots, 6 threads | 43 min, 0 failures |
| Indexer memory between shots | ~60 MB |
| Hidden app memory | 37 MB |
| Text index | ~6.8 MB/1,000 shots |
| Thumbnails | ~23 MB/1,000 shots |
| Demo library | 6,887 screenshots |

### Engineering lessons

1. **Daemon-first, UI-second.** OCR/decoding memory never lives in the window
   process; the window can be hidden and cheap.
2. **Idle by design.** Idle scheduling, battery pacing, pause, settle delays.
3. **One SQLite file** is enough for a single-purpose index; FTS5 trigram gives
   substring search and very low operational complexity.
4. **OCR-error tolerance is a first-class search feature**, not a semantic
   afterthought (folded index + verification).
5. **Lazy work in the query path** (lines only for visible tiles).
6. **Return memory to the OS** after each image, not just at exit.
7. **Newest-first** is the right default for personal data.
8. **The OCR engine choice matters more than the language/framework.** They
   chose PaddleOCR for small UI text; tesseract was faster/more memory-friendly
   but missed the text people search for.

### Throughput vs accuracy: the most important contrast

The reference app's tuned PaddleOCR processes **one screenshot at a time** with intra-op
threads: ~0.67 s/shot on 4 cores, 43 min for 5,159 shots. Kensaku's current
pipeline runs **8 tesseract processes in parallel**, each pinned to one OpenMP
thread, and measured **~15 screenshots/s** on the same class of images (400 in
~26 s). So for bulk screenshot indexing Kensaku is roughly an order of
magnitude faster; the reference app buys much better small-UI-text recall for that cost.

The sensible synthesis is not to pick one: keep Tesseract as the fast, smaller
default for documents and bulk speed, and offer PaddleOCR PP-OCRv6 as a slower
alternative for screenshots and UI captures. Our own fixture showed parity, so
this is a user choice rather than a proven accuracy upgrade.

## 2. The comparable landscape

### 2.1 Classic desktop full-text search

| Project | Stack | Index | Content | OCR | Semantic | Notes |
|---|---|---|---|---|---|---|
| **Recoll** | C++/Python, Qt, Xapian | Xapian | Yes, very broad format coverage via helper scripts | Optional Tesseract/ABBYY, off by default (slow) | No | Mature, configurable, desktop Linux/Windows/macOS |
| **DocFetcher** | Java, SWT, Lucene | Lucene | Yes | Not a core feature | No | Portable index, ~200 files/min claimed, simple |
| **GNOME LocalSearch** (ex-Tracker) | C, RDF/SPARQL, Nepomuk ontology | SQLite + SPARQL | Yes, sandboxed MIME extractors | Depending on extractors | No | OS-integrated; 5 s dead-man switch on extractors; low-memory design goal |
| **KDE Baloo** | C++/Qt, KFileMetaData | Custom file store + search index | Optional | Depending on plugins | No | Small-footprint goal; 10 MB text cap; KDE-integrated |
| **Everything** | Windows, NTFS MFT + USN journal | In-memory/MFT | No (names only) | No | No | The speed bar: millions of names in seconds |
| **FSearch** (cboxdoerfer) | C, GTK3 | Own DB + inotify | No (names only) | No | No | Everything-like on Linux, wildcard/regex |
| **fsearch** (noahdunnagan) | Rust, macOS | mmap'd name arrays + trigram content + 3-bit filter | Yes (text/code, 1 MB, 10 MB in git) | No | No | Daemon + CLI + MCP; **21–35 MB daemon**, 0.1 ms names, 2.3 ms content, 8M files |

**Takeaway:** the classic field proves that *names-only* can be nearly free and
instant, while *content* is where the cost, complexity and format support live.
Recoll is the format-coverage benchmark; fsearch (noahdunnagan) is the
low-memory daemon benchmark.

### 2.2 AI-first / semantic local search

| Project | Stack | Index / retrieval | OCR / media | Footprint | Notes |
|---|---|---|---|---|---|
| **Omni** (hanxiao) | Native Swift, MLX on Metal | One vector space, in-process GPU embedding | OCR pack 4.2 GB; text, image, audio, video | 1.8 GB search model + 4.2 GB OCR | macOS only; HTTP + MCP; 2.7M files / 19 ms claim; app Apache-2.0, weights CC-BY-NC |
| **File Brain** | Python, Typesense, Tika, Docker | Typesense (lexical + vector), ONNX MPNet | Tika 1000+ formats, OCR | Heavy (Docker + Typesense + Tika) | GPLv3; cross-language |
| **Semantra** | Tauri + Rust + MLX/ONNX | LanceDB, EmbeddingGemma2 | Multimodal (text/PDF/image/audio/video) | ~1.5 GB model | Closest Tauri + semantic sibling |
| **Panoptikon** | Rust server + Python inference | SQLite + FTS + CLIP/text embeddings | OCR, Whisper, CLIP, tags, captions | Heavy inference models | AGPL; not high-concurrency by design |
| **Khoj** | Python/Django/Postgres/pgvector | Semantic + agents | Docs/notes, not a filesystem crawler | Server/cloud or local | Agent-first; MCP-adjacent |
| **ck** | Rust workspace | Tantivy + FastEmbed + ANN; hybrid RRF | Code/text only | Sidecar `.ck/` | CLI + MCP; BLAKE3 chunk cache; ~1M LOC in <2 min; Apache/MIT |
| **Kensaku** | Rust, Tauri, Tantivy, SQLite, static i8 embeddings, Tesseract | Lexical + semantic RRF, exact i8 scan | PDF/Office/EPUB/RTF/text/images OCR | ~30 MB model, ~73 MB indexer, Tauri webview extra | Rich query language, intent, CLI + MCP, MIT |

**Takeaway:** the AI-first field splits into *heavy but high-quality*
(Omni/Semantra, 1.5–4 GB models, GPU) and *light but lower-quality* (Kensaku's
static table). `ck` is the closest architectural sibling for code + MCP.
Nobody combines Kensaku's document breadth + semantic + desktop + permissive
license at a small footprint.

### 2.3 Document management / OCR pipelines

| Project | Stack | OCR | Search | Notes |
|---|---|---|---|---|
| **Paperless-ngx** | Django, Postgres/MariaDB/SQLite, Redis, Celery, Angular | OCRmyPDF + Tesseract 5 | Postgres full-text (relevance-ranked, autocomplete) | Tags/correspondents/types, ML classification, PDF/A archive; GPL-3.0; server-class memory (third-party test: ~540–820 MB) |
| **Docspell** | Scala/Http4s/Doobie, Elm, PostgreSQL | Tesseract + ocrmypdf + unoserver/unoconvert | Solr or PostgreSQL FTS | Multi-component (REST + JOEX), AGPLv3; server/self-hosted |
| **Immich** | Node/NestJS, Postgres + VectorChord, Python ML | OCR field, CLIP, faces | Metadata + CLIP semantic + faces | Photo/video first; server; best-in-class photo semantic |
| **Open Semantic Search** | Elasticsearch/Solr + Tika + Tesseract + NLP | Tesseract, embedded PDF images | Lucene/Solr + NLP enrichment | Enterprise search stack, heavy |

**Takeaway:** these are *pipelines* (queue, workers, retries, archive format)
more than desktop indexes. Their robustness and OCR-myPDF-style text layers are
worth borrowing; their deployment model is not.

### 2.4 File platform with an index

| Project | Stack | Index | Notes |
|---|---|---|---|
| **Spacedrive** | Rust daemon + Tauri/React, SQLite, FTS5 | VDFS; in-RAM index for "millions of files" (~50 B/entry, slab arena + name interning), SQLite FTS5 + semantic reranking | Daemon-client over JSON-RPC; content indexing phase; cross-device; closest architecture to a "Kensaku daemon" |

**Takeaway:** a Rust core + daemon + compact in-RAM index + SQLite FTS5 is a
proven shape for millions of files. Tauri is the client, not the indexer.

## 3. Kensaku vs the field

### Where Kensaku is strong

- **One index for documents, images, and code** with a real query language and
  an intent compiler. Most competitors are narrower.
- **Hybrid lexical + semantic** with RRF, and a **30 MB** static embedding table.
  Omni/Semantra/File Brain are 1.5–4 GB models; Recoll/DocFetcher/fsearch have
  no semantic tier.
- **CLI + MCP** with read-only tools; many desktop tools have neither.
- **MIT license**; the closest full-featured competitors are GPL/AGPL.
- **Measured speed**: ~2.8 ms warm fuzzy search at 100k; 703 mixed files in
  ~23–27 s including 400 OCR images; ~73 MB indexer peak.

### Where Kensaku is behind

| Gap | Evidence | Consequence |
|---|---|---|
| **No daemon split** | the reference app, Spacedrive, fsearch (noahdunnagan), and ck all separate indexer/service from UI | WebView memory never leaves the indexing process; MCP/CLI reload metadata every session |
| **Metadata footprint** | 128 B/entry + duplicate path map; Spacedrive ~50 B/entry; fsearch 21–35 MB daemon at 8M files | 1M files ≈ 128 MB+ just for entries; the "idle RSS < 60 MB at 1M" target is unreachable |
| **Full-image phash decoding** | the reference app hashes a 480 px thumbnail (~2.4 ms) | Our background hash pass still decodes full images; slow and memory-hungry on photos |
| **No trigram substring / OCR-fuzzy tier** | the reference app trigram + folded index; fsearch trigram | Partial words and OCR confusions miss; `re:` can scan but is not a fast tier |
| **Tesseract OCR on UI screenshots** | the reference app bake-off: tesseract missed small UI text; PaddleOCR nearly read everything | Screenshot text quality is lower than the best-in-class |
| **No idle/battery scheduling** | the reference app SCHED_IDLE/ioprio idle, battery pacing, pause | Crawls can compete with interactive work |
| **No memory release after extraction** | the reference app `malloc_trim` / `malloc_zone_pressure_relief` | Freed image buffers linger in RSS |
| **MCP surface is small** | ck: 6 tools + pagination; fsearch: find/grep/multi_grep | Agents can search but not reindex/health-check/paginate deeply |

## 4. What Kensaku should adopt (ranked)

### P0 — architecture and memory

1. **Split the indexer into a daemon.** One process owns the index; the Tauri
   window, CLI, and MCP attach to it (Unix socket / loopback, token auth later).
   This is already in `REMAINING.md`; make it the next milestone. It is the only
   way to get the GUI out of the indexing memory path and to make MCP sessions
   cheap. Reference: Spacedrive, fsearch, ck, and the reference app.
2. **Compact metadata arena.** Replace `Vec<FileEntry>` + `HashMap<PathBuf,
   usize>` with an arena: name bytes + parent id + attrs (~40–50 B/entry), and
   a hash/id map. Target < 60 B/entry. Reference: Spacedrive `NodeArena` +
   `NameCache`; noahdunnagan fsearch's mmap'd name arrays.
3. **Thumbnail-based hashing + memory release.** Generate the 480 px thumbnail
   once, hash that (the reference app: ~2.4 ms), reuse it for the grid, and call
   `malloc_trim`/`malloc_zone_pressure_relief` after each heavy image/PDF.
4. **Lazy snippets.** Fetch matched lines/snippets only for visible results.
   The reference app: 60–90 ms → 1–10 ms for broad queries. Our `read_snippet` command is
   already the right primitive; make the GUI use it per visible item.

### P1 — search quality and indexing behavior

5. **Add a folded/trigram content field** for substring and OCR-error tolerant
   matching (`nutsmp`, `0RDER`, `rn`/`m`). Keep BM25/semantic as the ranking
   tiers; use the folded field as a fallback tier, always after exact matches.
6. **Optional high-quality OCR pack.** PaddleOCR PP-OCRv6 tiny detector + small
   recognizer on ONNX Runtime for screenshots/UI text, with Tesseract remaining
   the default for documents and low-memory setups. Weigh 22 MB models + 24 MB
   ORT plus the OCR process peak (the reference app measured 278–311 MB while indexing on
   1–4 cores and ~640 MB in its Python bake-off). Native macOS Vision / Windows
   OCR is the lower-memory alternative where available.
7. **Idle / battery / pause scheduling.** `SCHED_IDLE` + idle I/O on Linux,
   `PRIO_DARWIN_BG` on macOS, background mode on Windows; newest-first fresh
   items; on battery, a slower pace. Expose a "full speed / balanced / eco"
   setting.
8. **Settle + still-writing handling.** 400 ms debounce, retry files written
   within the last 10 s (browser save/rename patterns). Our watcher already
   batches; add the settle/retry semantics.
9. **Newest-first within type.** Keep text-first for first-scan usability, but
   prioritize recently modified files within each group (the reference app's rationale).

### P2 — agent and ecosystem

10. **Expand MCP**: `reindex`, `health_check`, `index_status` (already), pagination
    with cursors, `grep`/`find_symbol`, and snippet budgets. Reference: ck.
11. **Pluggable extractors.** A small external-command registry (like Recoll's
    filters) for formats we do not parse natively. Do not bundle Tika.
12. **Model/download hygiene.** SHA-256 pinning, mirrors, resume, progress, and
    a checksum failure path. The reference app does this; our model downloads should too.

## 5. What not to adopt

- **1.5–4 GB embedding/OCR models** (Omni/Semantra): incompatible with a small
  desktop footprint and instant startup.
- **Server/Docker stacks** (Paperless/Docspell/File Brain/Open Semantic Search):
  wrong deployment model for a desktop index; useful only as pipeline
  references.
- **RDF/SPARQL metadata** (LocalSearch): powerful but far more complexity than a
  personal file index needs.
- **A full format-extraction framework in-process**: the dependency and security
  surface is large; external helpers/plugins are the safer path.
- **Single-threaded OCR with many intra-op threads** (the reference app): it is simpler
  and accurate, but our parallel-across-files + OMP-pinned tesseract is faster
  for bulk screenshots. Keep both options.

## 6. If we were choosing the stack today

| Goal | Best-fit choice | Why |
|---|---|---|
| Lowest memory desktop | Rust daemon + native UI (GPUI/Slint/egui) or CLI/MCP only | WebView baseline disappears; the reference app stays at 37 MB hidden, fsearch 21–35 MB daemon |
| Best cross-platform desktop | Tauri (current) | Mature, small binary, good Linux/macOS/Windows story; webview memory is the tradeoff |
| Best lexical search | Tantivy (current) | BM25, fields, fast at 100k–1M; SQLite FTS5 is simpler but weaker ranking |
| Best simple index | SQLite FTS5 trigram (the reference app and Spacedrive) | One file, substring search, low ops; weaker ranking/field control |
| Best semantic quality | GPU + larger model (Omni/Semantra) | Quality at 1.5–4 GB; wrong for our default |
| Best semantic footprint | Static i8 table (current) | ~30 MB, CPU-only, exact scan; ONNX only as an optional quality pack |
| Best OCR for documents | Tesseract (current) | Mature, 100+ languages, low memory |
| Best OCR for UI screenshots | PaddleOCR PP-OCRv6 or native OS OCR | Reads small UI text that tesseract misses |
| Best agent surface | MCP over stdio (current) + daemon | ck/fsearch/Omni show MCP is table stakes |

## 7. Sources

- Recoll: <https://www.recoll.org/> and the user manual
- DocFetcher: <https://docfetcher.sourceforge.io/>
- GNOME LocalSearch: <https://gnome.pages.gitlab.gnome.org/localsearch/>
- KDE Baloo: <https://github.com/KDE/baloo>, KDE wiki
- Everything / FSearch: voidtools, <http://cboxdoerfer.github.io/fsearch>
- fsearch (macOS): <https://github.com/noahdunnagan/fsearch>
- Spacedrive: <https://github.com/spacedriveapp/spacedrive>,
  <https://v2.spacedrive.com/core/indexing>
- Omni: <https://github.com/hanxiao/omni-macos>
- File Brain: <https://github.com/Hamza5/file-brain>
- Semantra: <https://github.com/freedmand/semantra>
- Panoptikon: <https://github.com/reasv/panoptikon>
- Khoj: <https://github.com/khoj-ai/khoj>
- ck: <https://github.com/BeaconBay/ck>
- Paperless-ngx: <https://docs.paperless-ngx.com/>
- Docspell: <https://docspell.org/docs/>
- Immich: <https://docs.immich.app/>
- Open Semantic Search: <https://opensemanticsearch.org/>
