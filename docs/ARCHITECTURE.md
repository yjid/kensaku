# Kensaku v2 — Architecture & Implementation Plan

> **Status:** design through `v0.3.3`. This document describes the target
> architecture as well as what shipped; they are not identical. In particular,
> the daemon topology in §3 and the HTTP transport in §9.3 are **planned**, not
> current — today the GUI backend is in-process and the CLI/MCP load the index
> themselves. Product direction and sequencing live in
> [`VISION.md`](VISION.md); the engineering backlog lives in
> [`REMAINING.md`](REMAINING.md).

> **One sentence:** a warm, local, read-only index of your machine that answers in under 15 ms — for you, and for every AI agent you use.

---

## 1. North star

| Goal | Concrete meaning |
|---|---|
| **Instant** | p50 keystroke → painted results < 15 ms at 1M files. p99 < 60 ms. |
| **Correct** | Zero ghost results. Zero silent staleness. Freshness is a visible number. |
| **Simple** | Zero-config default. One shortcut. One box. One list. No modes, no tabs. |
| **Smart** | Semantic search that costs ~0.1 ms per query and never needs a GPU. |
| **Universal** | The same index serves the GUI, the CLI, and MCP/HTTP for agents. |
| **Small** | Core binary ~15 MB, GUI bundle ~25 MB, semantic model ~8 MB bundled. |

### UX invariants (non-negotiable, encode these in tests)

1. Typing never blocks. Ever. Degrade results, not input.
2. Never blank → spinner → results. Show the previous results until the new ones are ready.
3. Results never reorder while you're still typing. Stability beats optimality.
4. Freshness is always visible (`12 ms · 1.2M files · index 2 s old`).
5. Filters are visible chips you can click off. An invisible filter is a bug report.
6. Every capability is a `filter:` or a `>` command — never a new screen.
7. First run is one guided flow: folders, preferences, optional packs, first scan, Done.

---

## 2. Non-goals (deliberate)

- No cloud, no accounts, no telemetry, no sync, no cross-device.
- No bundled LLM. "Ask" uses a local Ollama/LM Studio if present.
- No graph database, no agent framework, no plugin marketplace.
- No write operations through the agent API. Read-only, always.
- No cross-encoder reranker in the hot path (30–60 ms; it does not fit).
- No ANN index below 400k semantic units (exact scan is faster and bug-free).

---

## 3. System topology

```
┌────────────────────────────── clients ──────────────────────────────┐
│  GUI (Tauri 2 / React)      CLI (`kensaku search`)     AI agents     │
│         │                          │                   (MCP stdio / │
│         │  IPC (UDS/named pipe)    │                    HTTP)       │
└─────────┼──────────────────────────┼───────────────────┼────────────┘
          └──────────────┬───────────┴───────────────────┘
                         ▼
        ┌────────────────────────────────────────────┐
        │        kensaku-daemon (one process)        │
        │  single instance · single writer · 0600    │
        ├────────────────────────────────────────────┤
        │  Query planner   →   Fusion/RRF   →  Snips │
        ├──────────┬──────────────┬──────────────────┤
        │ Meta     │ Content      │ Semantic         │
        │ tier     │ tier         │ tier (optional)  │
        │ arena +  │ Tantivy BM25 │ model2vec +      │
        │ tree     │ + fastfields │ i8 vectors       │
        ├──────────┴──────────────┴──────────────────┤
        │ Extraction workers (bounded pool, priority) │
        │ Watcher backends: inotify/FSEvents/USN/Poll │
        │ Reconciler · Embed queue · Merger           │
        ├────────────────────────────────────────────┤
        │ SQLite WAL (durable truth) + mmap snapshots │
        └────────────────────────────────────────────┘
```

**Why a daemon, not an in-app backend:** the agent surface and the CLI require the index to exist without the GUI. Building it in-process first makes that a rewrite. The GUI becomes a client — which also means index work never competes with webview paint.

**Two binaries, one core crate:** `kensaku` (daemon + CLI + MCP) and `kensaku-app` (Tauri GUI). The GUI auto-spawns the daemon if it isn't running.

---

## 4. Data model

### 4.1 SQLite (durable truth, WAL, `mmap_size=512MB`)

```sql
CREATE TABLE roots (
  id INTEGER PRIMARY KEY, path BLOB NOT NULL UNIQUE,
  mode TEXT NOT NULL,            -- index | meta_only | ignore
  added_at INTEGER NOT NULL
);

CREATE TABLE nodes (             -- files AND directories
  id         INTEGER PRIMARY KEY,
  parent     INTEGER REFERENCES nodes(id),
  name       BLOB NOT NULL,      -- raw OsStr bytes
  name_fold  BLOB NOT NULL,      -- NFC + lowercase, for matching only
  kind       INTEGER NOT NULL,   -- 0 dir | 1 file
  ext        INTEGER,            -- interned extension id
  size       INTEGER,
  mtime_ns   INTEGER,
  ctime_ns   INTEGER,
  dev        INTEGER,
  ino        INTEGER,
  file_id    BLOB,               -- Windows FILE_ID / macOS fsobj_id
  flags      INTEGER,            -- symlink, hidden, cloud_placeholder, remote_fs
  content_hash BLOB,             -- BLAKE3, only when extracted
  extract_state INTEGER,         -- 0 pending 1 ok 2 skipped 3 failed
  extract_attempts INTEGER,
  extract_error  TEXT,
  indexed_gen INTEGER            -- generation of content index for this node
);
CREATE INDEX nodes_parent ON nodes(parent);
CREATE INDEX nodes_ident  ON nodes(dev, ino);
CREATE UNIQUE INDEX nodes_path ON nodes(parent, name);

CREATE TABLE units (             -- semantic units (doc-level or chunk-level)
  id INTEGER PRIMARY KEY,
  node_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
  byte_start INTEGER NOT NULL,
  byte_end   INTEGER NOT NULL,
  vec_id     INTEGER NOT NULL,
  model_id   INTEGER NOT NULL
);
CREATE INDEX units_node ON units(node_id);

CREATE TABLE usage (             -- frecency
  node_id INTEGER PRIMARY KEY, opens INTEGER DEFAULT 0,
  last_open INTEGER, last_query_hit INTEGER
);

CREATE TABLE meta (key TEXT PRIMARY KEY, value BLOB NOT NULL);
-- schema_version, index_gen, model_id, last_reconcile, daemon_token
```

**Why SQLite + in-memory snapshot, not a pure mmap file:** crash safety and migrations for free; the custom mmap format (Everything-style) is a later optimization if load time exceeds 400 ms at 1M.

### 4.2 In-memory snapshot (built at startup, mutated incrementally)

```rust
struct Snapshot {
    // Struct-of-arrays: one allocation each, no per-entry PathBuf/String.
    names:      Vec<u8>,        // concatenated folded names
    name_off:   Vec<u32>,       // len = n+1
    parent:     Vec<u32>,
    first_child:Vec<u32>,       // + next_sibling: Vec<u32>  → child lists
    next_sibling: Vec<u32>,
    kind_ext:   Vec<u32>,       // kind bit + ext id
    size:       Vec<u64>,
    mtime:      Vec<u64>,
    char_bloom: Vec<u64>,       // 64-bit letter-bucket mask per name
    id_map:     HashMap<u32, u32>, // disk id ↔ slot, only for mutations
}
```

- ~64 bytes/file → **64 MB per 1M files** (vs several hundred MB with `PathBuf`).
- `char_bloom` is the cheap prefilter: a query's character set is AND-ed against each entry; one AND rejects 80–95% before the matcher runs.
- Folder scope = DFS over child lists, cached per `(dir_id, generation)`. Targeted scopes (the common case) visit hundreds of nodes, not millions.
- Mutations go to a small delta buffer and merge into the arena at idle; a full rebuild takes ~300 ms per 1M and happens only on schema change.

---

## 5. Index tiers

### 5.1 Metadata tier (always fresh, in-memory) — budget **2–4 ms**

- Built from the filesystem journal (USN/FSEvents/inotify), not from crawling.
- Lookup: `(dev, ino)` and `(parent, name)` — O(1).
- Used for: filename search, scoping, filters on size/mtime/kind, dedupe, path display.

### 5.2 Content tier (Tantivy, batched) — budget **1–3 ms**

Schema:

| field | type | purpose |
|---|---|---|
| `node_id` | u64, fast, indexed | delete-by-term, join to metadata |
| `name` | text (code tokenizer) | filename relevance |
| `content` | text (code+CJK tokenizer), **not stored** | BM25 |
| `ext` | u32, fast | `type:` filter |
| `mtime` | u64, fast | `modified:` range |
| `size` | u64, fast | `size:` range |
| `lang` | u8, fast | `lang:` filter |

- **Custom tokenizer:** split `camelCase`/`snake_case`/`kebab-case`, lowercase, min length 2, max token length 48, CJK bigrams, drop pure-numeric/hash-like tokens. This single change is worth more relevance than any embedding model.
- **Never** `QueryParser`. Build `Query` objects from our AST. No OR-by-default.
- **Content policy:** full text up to 1 MB; head 512 KB + tail 512 KB for larger; metadata-only above 20 MB.
- **Commit policy:** debounce 2 s or 2,000 docs, writer 128 MB. `wait_merging_threads()` + `garbage_collect_files()` at idle, and a weekly force-merge.
- **Snippets:** never read a file inside the query. Fetch after ranking, in the blocking pool, for the top 20 only, with an LRU cache of the last 200 files.

### 5.3 Semantic tier — budget **0.5–2 ms**

**Default model: `potion-base-8M` (model2vec, static, 256-dim, int8 table ≈ 8 MB, bundled).**
No ONNX, no neural graph at query time: tokenize → table lookup → weighted mean → normalize. **~50 µs per query.** ~92 % of MiniLM's MTEB average. Embedding the corpus is also cheap (~50k docs/s), so full re-embed after a model change is minutes, not hours.

**Optional "Smart" pack** (downloaded, ~40 MB): `fastembed-rs` + `ort` with **`load-dynamic`** (never static — prebuilt static ORT requires AVX2 and SIGILLs on older CPUs; dynamic also fixes macOS codesign and Windows dylib swaps). `bge-small-en-v1.5` or `EmbeddingGemma-Q4` for multilingual quality.

**Units and chunking:**

```rust
struct Unit { node_id, byte_start, byte_end, model_idx }  // 1 vector per unit
```
- Files ≤ 240 tokens → one doc-level unit (offset 0).
- Longer files → 240-token windows, 48 overlap, max 20 units/file.
- Embedding cost is deduped by `content_hash`: identical files share one vector computation (and one vector, referenced twice).
- Query returns **nodes**, with the best unit's byte range attached → highlight and scroll position.

**Retrieval:**
- ≤ 400k units: exact scan over memory-mapped `Vec<i8>` (256 B/unit → 100 MB at 400k), 8-way parallel → < 2 ms.
- \> 400k units: build USearch HNSW (`cos`, i8, M=16, efConstruction=200, ef=64) at idle. Deletions via tombstones; rebuild at idle when tombstones > 15 %.
- **Model identity is per-index:** `units.model_id` + `meta.model_id`. Changing the model never corrupts results — the old index keeps serving while the new one fills.

**Embedding queue:** bounded, resumable cursor in `nodes.indexed_gen`, priority = recently-modified first, then small files first. Hard rules: stop on battery, stop when a query is in flight, stop during fullscreen apps, max 2 threads.

---

## 6. Query pipeline

```
input ──► Lexer ──► AST ──► Router ──► Planner ──► Executors ──► RRF ──► Rank ──► Snip ──► IPC
         (0.02ms)          (0ms)        (0.05ms)    ┌ fuzzy  2–4ms │  0.1ms   0.2ms   5–20ms
                                                    │ BM25   1–3ms │         (top-20 only,
                                                    └ vector 0.5–2ms          post-paint)
```

### 6.1 AST

```rust
struct Query {
    terms:   Vec<Term>,        // { text, field: Any|Name|Content|Path, phrase: bool }
    filters: Vec<Filter>,      // Ext(Type) | Modified(..) | Size(..) | Kind(..) | Lang(..) | Not(..)
    scopes:  Vec<Scope>,       // dir id + include/exclude
    mode:    Mode,             // Ranked | Exhaustive
    raw:     String,
}
```

### 6.2 Grammar (all optional — plain typing always works)

```
type:pdf ext:rs in:~/Projects/kensaku path:src-tauri
modified:<7d size:>2mb lang:rust is:image -node_modules "exact phrase"
re:"fn\s+\w+"        name:config        content:timeout
```

Rules: quoted strings support spaces; `-` negates; unknown `key:` is treated as literal text; parse errors produce an inline hint, never zero results.

### 6.3 Router (0 ms, no model)

| Signal | Route |
|---|---|
| ≤ 2 tokens, no spaces, has `camelCase`/`_`/`.` | name-heavy + code tokenizer |
| quoted or `re:` | content + phrase/regex |
| `type:`/`in:` present | filtered, scope-first |
| ≥ 4 tokens or contains NL stopwords | semantic weight ↑, BM25 ↓ |
| ends with `/` | pure scope navigation (list the folder) |

### 6.4 Scope resolution

`in:` and leading directory-looking tokens resolve against a **directory name trie** (folded). Exact > prefix > fuzzy. Ambiguous match → keep as a search term *and* show a "did you mean 📁 kensaku?" chip. A resolved scope uses cached DFS ranges, so scoped queries are O(subtree), not O(corpus).

### 6.5 Fusion and ranking

```
score(doc) = Σ_tier  w_tier · 1/(60 + rank_tier)      // RRF
           + 0.20 · exact_substring_in_name
           + 0.15 · prefix_in_name
           + 0.10 · exp(-age_days / 7)                 // recency, smooth
           + 0.08 · log1p(opens) · exp(-last_open_days/14)  // frecency
           - 0.02 · depth
```

Weights live in config; RRF is chosen because BM25 scores are corpus-dependent and unbounded. Ties break on path (deterministic — no `HashMap` iteration order).

### 6.6 Execution rules

- All work happens on a bounded blocking pool; the async layer only routes and serializes.
- One `Arc<Snapshot>` + one Tantivy `Searcher` per query → consistent snapshot, no write locks.
- Per-stage time budget; if the semantic stage blows its budget, return lexical results and mark the response `partial: true`.
- Requests carry a monotonic `seq`. A new request cancels the previous scan (`AtomicBool` checked every 1k iterations) and the client drops stale responses.
- `mode: exhaustive` streams results in pages with a total count — for agents and regex-style enumeration.

---

## 7. Freshness & correctness model

This is the section that makes the speed claims true.

1. **Identity, not paths.** Every node is keyed by `(dev, ino)` / `file_id`. A rename updates one row. Content is never re-extracted on a move.
2. **Three generations.** `meta_gen` (metadata changes), `content_gen` (BM25), `vector_gen`. Every response carries all three plus `as_of` timestamps.
3. **Search first, reconcile after.** Startup loads the durable snapshot and is searchable in < 400 ms. A background **stat-only** reconcile then diffs the tree (`jwalk` without reads: 1M files ≈ 20–40 s warm) and patches gaps. Until it finishes, the UI shows `index Ns old`.
4. **Watcher is a hint, not the truth.** Events are coalesced per path with a 150 ms debounce. Any of `IN_Q_OVERFLOW`, FSEvents `MustScanSubDirs`, or Windows buffer overflow schedules a scoped rescan. A watch failure downgrades to polling with a visible banner.
5. **Reconcile-on-resume.** After sleep, replay from the last event cursor; if the cursor is invalid (USN journal rolled over), full rescan.
6. **Repair pass** every 6 h or on boot: verify a random 1 % sample by `(size, mtime)`; mismatch → re-extract. Full integrity check is a manual command.
7. **Content diff rule:** re-extract only if `(size, mtime_ns, ino)` changed, or `content_hash` mismatches after a settle re-read. `chmod`/`atime`-only events never trigger content work.
8. **Junk filter:** `.swp`, `4913`, `~$*.docx`, `.git/index.lock`, `*.tmp`, `.DS_Store`, `.goutputstream-*` — filtered before any work.
9. **Deletes** are tombstoned, then purged in batches at idle (Tantivy delete-by-term, vector tombstone, SQLite delete), so the interactive path never pays for them.
10. **Crash safety:** SQLite WAL + Tantivy's own crash recovery. A `kill -9` mid-crawl leaves a resumable state; unfinished content is marked `pending`, never presented as indexed.

---

## 8. Extraction

```
magic-byte sniff ─► dispatcher ─► extractor (isolated worker, timeout) ─► state machine
```

| Format | Path | Notes |
|---|---|---|
| text/code/config | encoding detect → UTF-8 | UTF-16 BOM, Latin-1 fallback; strict-UTF-8 rejection is a bug |
| DOCX/PPTX/XLSX/EPUB | ZIP + quick-xml | shared strings, speaker notes, hidden sheets; **DTDs disabled** (billion-laughs) |
| PDF | `pdf-extract` first; `pdfium` feature flag if quality fails | page-level offsets → "jump to page"; encrypted/scanned → explicit state |
| Images | platform OCR (Vision / Windows OCR / optional tesseract) → text units | OCR before CLIP; screenshots prioritized |
| Archives | metadata only (v2) | no recursive indexing |

State machine: `pending → ok | skipped(reason) | failed(reason)`, max 3 attempts with backoff. Counts are surfaced ("38 files could not be read"), never silently dropped. Each extractor runs with a timeout and a panic boundary in its own worker — a malformed PDF must never take down the daemon.

---

## 9. The three surfaces

### 9.1 GUI — "super simple" is a spec, not a vibe

- **First run (shipped):** guided flow — welcome, folders, exclusions, preferences, optional packs, first scan, Done. The first scan finishes with counts and a Done summary before the main window opens; setup markers are gated by `first_scan_done`. Settings → About offers **Run setup again**.
- **Default screen:** input + list + right-side preview. No tabs, no sidebar, no modes.
- **Chips** under the input appear only when a filter/scope is active; click to remove.
- **`⌘K` / `Ctrl+K`** on a result: Open · Open at line · Reveal · Copy path · Copy content · Ask about this · Find similar.
- **`>`** in the input switches to commands: `>reindex`, `>add folder`, `>model`, `>doctor`.
- **Bottom bar:** `12 ms · 1,204,553 files · 2 s old · semantic ✓`.
- **Preview:** monospace text with highlighted matches for code/text; page render for PDF; thumbnail for images. Virtualized list, fixed row height, no layout shift.
- **Semantic badge:** a subtle `≈` on results that only the vector tier found. No mode switch — semantics are part of the one box.
- **Settings:** progressive disclosure. Watched folders, exclusions preset, shortcut, and AI toggle stay visible. Today downloads and some setup controls live here; planned: setup owns initial configuration and downloads (§18.8). Everything else is a config file, documented, not a UI.

> This is the **shipped `v0.3.x`** surface. The planned zero-syntax interaction —
> plain-language intent, `/` commands, `@` scopes, interpretation chips, facet
> counts, grouped versions — is specified in §18.1–18.2 and
> [`VISION.md`](VISION.md). `>` is intended to become `/`; the old grammar stays
> available underneath.

**Perceived-speed rules:** render previous results during computation; fire immediately on Enter/paste; prefetch top-5 snippets; never animate reordering; empty state = recent files, rendered before any query.

### 9.2 CLI

```bash
kensaku search "docker error" --type png --since 7d --json
kensaku find --name '*.rs' --in src-tauri --exhaustive
kensaku state --json
kensaku index add ~/Projects
kensaku doctor
kensaku mcp            # stdio MCP server
```

Same AST and index as the GUI. Today the CLI loads the index per invocation; a
warm daemon (§18.4) removes that cost and lets GUI, CLI, and MCP share one index.

### 9.3 MCP / agent API

Transport: **stdio** (default, spawned per client, proxies to daemon) + **HTTP/SSE** on `127.0.0.1` with a token from `~/.local/state/kensaku/token` (0600).

| Tool | Contract |
|---|---|
| `search` | `query`, `mode: ranked\|exhaustive`, `path?`, `ext?`, `modified_after?`, `limit≤50`, `offset` → path, score, `line_start/line_end`, `snippet`, `as_of` |
| `read_snippet` | `path`, `query?`, `start_line?`, `max_tokens≤512` → ranked windows, **never whole files** |
| `find_related` | `path`, `limit` → semantic neighbours (cheap, static embeddings) |
| `index_state` | → per-root counts, generations, `as_of`, queue depth |
| `find_symbol` | (v2.1, tree-sitter) definition/reference lookup |

Design rules:
- **Ranked vs exhaustive is the key distinction.** Agents need completeness (exhaustive, paginated, streamed); humans need relevance. Conflating them produces bad tools.
- Responses are token-budgeted (~1,500 tokens default) and delimited so file content reads as data, never instructions.
- `.gitignore` / `.cursorignore` are honored by default (`include_ignored` opt-in) — agents assume parity and produce garbage context otherwise.
- Read-only. No write, no shell, no deletion.
- Ship ready-to-paste config for Cursor, Claude Code, and VS Code in the README and in Settings.

---

## 10. Performance budgets (1M files, warm, p50)

| Stage | Budget |
|---|---|
| parse + route + plan | 0.1 ms |
| scope resolve (cached) | 0.02 ms |
| filename prefilter + match (parallel) | 2–4 ms |
| BM25 | 1–3 ms |
| static-embedding query + exact scan (≤400k units) | 0.5–2 ms |
| fusion + rank | 0.2 ms |
| IPC serialize (50 results) | 0.5 ms |
| React render (virtualized) | 3–8 ms |
| **first paint** | **≈ 8–18 ms** |
| snippet fetch (post-paint, top 20) | 5–20 ms |

Gates: p50 < 15 ms, p95 < 40 ms, p99 < 60 ms. Idle RSS < 60 MB at 1M. Indexing throughput > 2,000 files/s metadata, > 150 files/s content. Semantic embedding > 2,000 docs/s.

---

## 11. Degradation matrix

| Failure | Behavior |
|---|---|
| Tantivy open error | lexical + semantic only; persistent banner; "Rebuild index" action |
| Watcher unavailable / limit hit | poll mode (30 s), banner, no silent staleness |
| Vector file corrupt | drop semantic tier, re-embed at idle, lexical unaffected |
| Model missing | static tier absent; everything else works |
| Disk full | pause writers, banner, keep serving reads |
| Extract worker panic/hang | per-file timeout, mark failed, continue |
| Second instance | focuses the first, exits |
| Sleep/resume | replay cursor; invalid → scoped rescan |

---

## 12. Repository layout

```
crates/
  kensaku-core/      model, snapshot, sqlite, watcher, reconcile, config
  kensaku-index/     metadata arena, fuzzy, tantivy, semantic
  kensaku-query/     lexer, ast, router, planner, fusion, rank, snippet
  kensaku-extract/   text, encoding, docx/pptx/xlsx/epub, pdf, ocr
  kensaku-embed/     model2vec, optional ort/fastembed, model registry
  kensaku-proto/     request/response types (shared by CLI, GUI, MCP)
  kensaku-daemon/    server, scheduler, lifecycle, MCP, HTTP
  kensaku-cli/       kensaku binary
  kensaku-bench/     corpus generator, golden queries, criterion benches
src-tauri/ + src/    GUI (client only — no indexing logic)
```

Rule: **`kensaku-core` must not depend on Tauri.** The GUI must not depend on index internals. That boundary is what makes the CLI and MCP possible.

---

## 13. Phase plan (each phase ships something usable)

### Phase 0 — Trustworthy foundation · ~2 weeks
Workspace split · SQLite schema + snapshot warm-load · `(dev, ino)` identity · mtime/size diffing · rename & subtree delete · watcher rewrite (overflow → rescan, single owned watcher, events during crawl) · search-first/reconcile-after startup · `spawn_blocking` + snapshot queries + cancellation · single instance · atomic config · schema versioning · `kensaku doctor`.
**Gate:** all §10 gates; 1,000 random renames/deletes → zero ghosts; `kill -9` × 20 → no corruption.

### Phase 1 — The query layer · ~1.5 weeks
Lexer/AST/parser · filters (`type/ext/in/path/modified/size/is/lang/-`) · directory trie + DFS scope cache · router · RRF fusion · deterministic ranking · frecency · char-bloom + top-k heap · snippets post-rank · chips, preview pane, actions, `>` commands · events instead of polling · request sequence guard.
**Gate:** recall@5 ≥ 0.90 on the golden set; `type:pdf invoice` on 800k files < 5 ms; filters never produce silent zero-results.

### Phase 2 — Semantic, default-on · ~1 week
`model2vec` static embeddings bundled · unit chunking + dedupe by hash · i8 exact scan · query path wiring · `≈` badge · embed queue with battery/idle policy · model registry + per-index `model_id`.
**Gate:** semantic query p50 < 2 ms; NDCG@10 ≥ 0.85 of a MiniLM baseline on the golden set; model swap re-embeds without breaking search.

### Phase 3 — Documents · ~1.5 weeks
Magic sniffing · encoding detection · DOCX/PPTX/XLSX/EPUB · PDF text layer + page offsets · extract state machine + surfacing failures · 1 MB/20 MB content policy.
**Gate:** 50-page PDF < 50 ms; malformed/encrypted/scanned handled without daemon crash; zero panics on a 500-file fuzz corpus.

### Phase 4 — Agent surface · ~1 week
Daemon socket + protocol · CLI commands · MCP stdio + HTTP with token auth · `search` / `read_snippet` / `find_related` / `index_state` · `.gitignore` parity · token budgets · config snippets for Cursor/Claude Code.
**Gate:** `search` p95 < 50 ms; MCP client round-trip works from Cursor and Claude Code; exhaustive mode paginates 100k matches without OOM.

### Phase 5 — Vision & OCR · ~1.5 weeks
Screenshot OCR (Vision/Windows OCR/tesseract) → text units · image embeddings (SigLIP or CLIP ViT-B/32 int8, **not** MobileCLIP on x86) · EXIF orientation · megapixel/size caps · perceptual-hash dedupe · thumbnails.
**Gate:** "screenshot of docker error" returns the right image in top 5; 50k-image scan < 10 ms.

### Phase 6 — Ask & act · ~1 week
Ollama/LM Studio detection → "Ask about this folder" with path+line citations · summarize/explain actions · duplicate detection · near-duplicate file cleanup view.
**Gate:** answers cite real paths; no network when no local model is configured.

### Phase 7 — Launch · ~1 week
GitHub Actions (AppImage/deb/AUR, dmg universal, msi/exe) · benchmark harness + published numbers · 15-second demo · docs · crash reporting (local, opt-in).

**Sequencing rationale:** foundation before features (or every feature multiplies divergence bugs), query layer before semantics (filters + RRF beat embeddings for perceived quality), agent surface before vision (it's the differentiator and cheap once the AST exists), "ask" last (it's a garnish on a working index, and it needs Ollama anyway).

---

## 14. Test & benchmark harness (build this in Phase 0, not at the end)

- **Corpus generator:** seeded, 10k/100k/1M synthetic files; configurable mix of code, prose, CJK, minified, binary, deep nesting, duplicate content.
- **Golden set:** 50 real queries with expected files (human-labelled once, versioned in git). Metrics: recall@5, MRR, NDCG@10.
- **Latency gates in CI** (`criterion` + thresholds): fail the build on > 20 % regression.
- **Chaos suite:** `kill -9` mid-crawl, disk full, permission flips, clock jumps, watcher overflow, index dir deleted at runtime.
- **Platform matrix:** ext4, APFS (case-insensitive), NTFS; non-UTF-8 names; CJK; emoji; cloud placeholders; 2-core no-AVX2 machine.
- **24 h soak:** RSS drift, fd leaks, watcher thread count, segment count.

---

## 15. Risk register

| Risk | Mitigation |
|---|---|
| Daemon lifecycle scares users | GUI auto-spawns; autostart optional but default-on; `kensaku status` explains everything |
| Windows USN journal effort | Ship `ReadDirectoryChangesW` + overflow rescan first; USN as a 2× indexing speedup in Phase 5 |
| Portable model quality below expectations | Ship the static tier as default, the ORT "smart pack" as a one-click upgrade; measure on the golden set |
| Tantivy memory/segment growth | Batched commits, idle merges, weekly force-merge, RSS gate in CI |
| Agent API scope creep | Read-only, five tools, versioned schema; refuse writes by design |
| 1M-file snapshot load too slow | Threshold at 400 ms; fall back to a memory-mapped snapshot format |
| Semantic storage at 1M files | Units capped 20/file, i8, hash dedupe, HNSW above 400k, chunk only mid-size files |

---

## 16. Decision log

1. **Daemon-first, GUI-as-client.** Enables CLI + agents without a rewrite; isolates indexing from paint.
2. **SQLite as truth, arena as hot path.** Durability without per-file `PathBuf` memory.
3. **Identity by inode/file_id, never by path.** Makes renames cheap and ghosts impossible.
4. **Static embeddings by default, ONNX optional.** Semantic search at ~50 µs/query, 8 MB, zero runtime deps; honest about the quality/size trade.
5. **No ANN until 400k units.** Exact scan is faster, exact, and dependency-free at desktop scale.
6. **RRF over weighted score fusion.** No calibration data, robust to corpus-dependent BM25.
7. **Search-first, reconcile-after startup.** The only way "instant on boot" coexists with correctness.
8. **Exact scan + fast fields before fancy relevance.** Filters are the real speed feature.
9. **Read-only agent surface.** Security, simplicity, and a clean mental model.
10. **No bundled LLM.** Ollama integration gives "ask" without shipping a 4 GB dependency.

---

## 17. First three PRs

1. **Workspace + durable metadata + snapshot.** Split crates, add SQLite schema, build the arena, warm-load at startup, keep current search working on top of it. *No new features — just a foundation you can measure.*
2. **Identity + incremental reconcile + watcher rewrite.** `(dev, ino)` keys, mtime/size diffing, rename/subtree-delete, single owned watcher, overflow → rescan, search-first startup.
3. **Query layer skeleton.** AST + lexer + `type:`/`in:`/`modified:`/`-` filters + scope trie + RRF fusion + deterministic tiebreak. Ship behind the existing UI; add chips.

After those three, the remaining phases are additive rather than corrective.

---

## 18. Planned product layers (v0.4+)

Product intent and acceptance tests are in [`VISION.md`](VISION.md). This
section is the technical shape of those layers.

### 18.1 Intent layer

Today the parser turns grammar into an AST. The next layer turns **language**
into the same AST, then shows the result as chips.

```rust
struct Interpretation {
    terms:    Vec<Term>,        // text, with aliases expanded
    filters:  Vec<Filter>,      // existing Filter enum
    scopes:   Vec<Scope>,       // existing directory-trie resolution
    sort:     Sort,             // new: BestMatch | Newest | Oldest | Largest | Name
    aliases:  Vec<AliasHit>,    // what was expanded, for the chips
    confidence: f32,
    source:   InterpretationSource, // Rules | Llm | Grammar
}
```

Pipeline:

```
raw input
  ├─ grammar parse (existing) ──────────────► AST
  ├─ alias expansion ───────────────────────► terms
  ├─ deterministic intent rules ─────────────► filters + sort + scope
  └─ optional local LLM compiler ───────────► canonical JSON, schema-validated
        └─ on failure/timeout: keep the deterministic result
```

Rules:

- Rules first, LLM never required. The app must work offline and instantly.
- The LLM returns **only** an interpretation (a strict, versioned JSON schema),
  never file results. Retrieval stays with the index.
- Validate every field against the existing AST enums; unknown keys are dropped,
  not executed.
- Show `interpretation` in the response and render it as chips.
- Cache `(raw input, config version) -> Interpretation` with a small LRU.
- If confidence is low, ask one chip-based clarification ("@Work or @Personal?"),
  never a chat paragraph.

Sort is executed after candidate collection, before the display limit. `Newest`
sorts matches by `mtime` desc with the existing deterministic path tiebreak;
`BestMatch` is today's RRF order. Recency stays a ranking bonus only in
`BestMatch`.

### 18.2 Facets and filter pushdown

Request additions:

```json
{ "query": "...", "facets": ["type", "lang", "scope", "modified", "size"] }
```

Response additions:

```json
{
  "facets": {
    "type":  [{ "value": "image", "count": 1204 }, ...],
    "lang":  [{ "value": "rust",  "count": 88 }],
    "modified": [{ "value": "7d", "count": 41 }]
  },
  "total": 1402
}
```

- Counts are for the **whole** result set, computed with the facet's own
  dimension excluded (standard faceted search).
- Push `ext`, `size`, `mtime`, and `lang` into Tantivy as fast fields and a
  `BooleanQuery`; stop scoring documents that cannot match.
- Compute metadata-only facets from the snapshot where possible; content facets
  from fast fields. Cache by `(query, generation)` and invalidate on write.
- Budget: facet computation ≤ 3 ms at 100k, ≤ 10 ms at 1M.
- Multi-select is OR within a dimension, AND across dimensions; negation is a
  `MustNot`. An empty facet selection is never sent as a filter.

### 18.3 Visual tier

Add a fourth retrieval tier alongside fuzzy name, BM25, and static vectors.

- **Model:** SigLIP or CLIP ViT-B/32 int8, `load-dynamic`, optional Vision Pack.
  Normalize embeddings; one vector per image; optional region/caption vectors.
- **Storage:** `visual_units(node_id, model_id, vec_id)`; vectors in the existing
  i8 store with `model_id` so a model swap never corrupts results. Reuse the
  thumbnail and dHash already stored per image.
- **Query:** text → visual embedding, or selected image → embedding. Fuse the
  visual rank with the other tiers through RRF; apply the same semantic floor
  and the same metadata filters first. A visual-only hit is marked as such.
- **Captions:** optional idle-time VLM captions enter the BM25 tier as text
  units, so "system design diagram" works even when the visual model is weak.
- **Face identity:** separate, opt-in table of 512-d face embeddings, clustered
  at idle; the user labels a cluster as "me"; search is cosine-to-centroid
  intersected with visual attributes. Explain the match
  (`face 0.82 · red cap 0.91`). Never upload, never export, delete-all must be
  verifiable.
- **Cost:** ~20 ms/image on 2 low-priority threads; 50k photos ≈ 17 min at idle.
  Honor battery/fullscreen/query-in-flight pauses; resume from a cursor.

### 18.4 Daemon migration

Current: the GUI owns the indexer in-process; `kensaku-cli` and MCP load the
metadata themselves. Target: one warm process, three clients.

```
GUI ─┐
CLI ─┼─► kensaku-daemon (UDS / named pipe, 0600 token)
MCP ─┘        │
              ├─ indexer + watcher + embed queue
              └─ query planner + snippets
```

- Keep a **dual mode**: if the daemon is absent, the GUI runs in-process as it
  does today. This avoids making the app unusable when the daemon fails.
- Start with UDS/named pipes; add loopback HTTP/SSE only for clients that need
  it. Token auth, read-only, no shell, no network beyond loopback.
- Migrate in this order: protocol types → daemon skeleton → GUI client →
  CLI/MCP proxy → HTTP. Do not block Milestones 1–2 on this.
- Single instance moves from the current file lock to the daemon lock.

### 18.5 Memory and personalization

All local, all inspectable, all deletable.

- **Aliases:** config-backed `alias -> [terms]`, seeded by onboarding preset
  (developer, designer, student, office) and user-editable.
- **Usage:** extend the existing frecency with query→result opens, refinements,
  and pins. Never upload; expose a "clear history" action.
- **Saved searches:** a named `Interpretation` + chips. User-created first;
  auto-suggestions require explicit acceptance.
- **Version groups:** conservative name + size + time heuristics, user-mergeable.
  A wrong group is worse than no group.
- **Home:** recent files, recent queries, pins, and suggestions derived from the
  local usage table.

### 18.6 Response and schema evolution

Additive fields only; existing clients keep working.

- `interpretation`: terms, filters, scopes, sort, aliases, confidence, source.
- `sort`: the effective sort order.
- `facets`: counts keyed by dimension.
- `partial`: which tier missed its deadline.
- `as_of`: already present; keep per-root freshness.

Bump the JSON schema version for MCP/CLI when a field changes meaning, not when
a field is added. The CLI default output stays `path<TAB>score<TAB>snippet`.

### 18.7 Testing these layers

- **Intent golden set:** 40–50 labelled queries, including `latest resume`,
  `screenshots from last week`, `docker error in Projects`, `my biggest videos`,
  `the invoice from March`. Gate on top-1/top-3 and one-shot success.
- **Facet tests:** count accuracy with multi-select and negation; latency at
  100k and 1M.
- **Visual tests:** a frozen 200-image set with labelled queries (`red cap`,
  `diagram`, `whiteboard`), measured at top-5.
- **Privacy tests:** default configuration makes zero network calls; sensitive
  paths never enter an index; face data delete-all is verified.
- **Migration tests:** index/model version changes never mix vector spaces.

### 18.8 Setup and first-run flow

`SetupModal` now runs the guided flow, `complete_setup` saves config and calls
`start_indexing`, and the modal stays on screen through the first scan until a
Done summary is acknowledged. Downloads (OCR language, semantic models) are
offered as optional packs in setup and remain available in Settings; future
Vision Packs will follow the same pattern.

**State machine**

```text
Welcome → Folders → Preferences → Optional packs → Scanning → Done
                                        │             │
                                        └─ skipped ───┘
Scanning ── interrupted → resume on next launch
Scanning ── failed → actionable error + retry, never Done
```

**Rules**

- **Setup offers downloads.** `download_ocr_language` and `download_models` are
  offered as optional packs during setup and remain available in Settings.
  Settings → About offers a "Run setup again" entry point.
- **Setup owns initial configuration.** Watched folders, exclusion preset,
  theme, shortcut, and optional packs are asked in context. `complete_setup`
  persists them atomically; changing them later means re-running setup.
- **Gate on the first scan.** A persisted `first_scan_done` marker is written
  when the crawl completes. `is_setup_complete()` returns true only when both
  setup is configured and the first crawl finished. `complete_setup` starts the
  crawl but does not mark the first scan complete. `run_setup_again` resets the
  markers without deleting the index.
- **Resume, don't lie.** If the app exits mid-scan, the next launch resumes the
  crawl and re-enters the Scanning state. A partial index is never reported as
  Done. Persist the setup config before starting the crawl; on launch, config
  present + `first_scan_done == false` routes straight to Scanning, not Welcome.
- **Show real progress.** The Scanning screen reads `get_indexing_status` and
  the metadata/content counts: files, folders, documents, images, OCR'd images,
  and embedded units, plus the current path and an ETA. The Done screen shows
  the final summary and "Start searching".
- **Optional means optional.** Declining packs leaves keyword + lexical search
  fully usable. The flow never blocks on a download the user did not choose.
- **Only the first scan waits.** After `first_scan_done`, startup is the
  existing search-first + background reconcile path. Do not regress
  warm-start latency for returning users.
- **Re-running setup** resets the setup markers, not the index, unless the user
  explicitly chooses a rebuild.

**Acceptance**

- A fresh install reaches Done without opening Settings and without any download
  initiated elsewhere.
- Killing the app during Scanning resumes and eventually reaches Done.
- Done is unreachable while the index is unusable.
- Second launch paints results while reconcile runs in the background.
