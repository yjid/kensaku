# Kensaku — Progress & Handoff

> **For the next agent/session:** this file is the single source of truth for what's done, what's next, and how to continue. Read `docs/ARCHITECTURE.md` for the full design, `docs/EDGE-CASES.md` for the correctness catalog.

---

## Current state

- **Branch:** `master`
- **Last commit before v2 work:** `d690ada` (initial release v0.1)
- **v2 work:** PR 1, PR 2, PR 3, PR 4, and PR 5 implemented and committed. Builds clean (`cargo build` and `npx tsc --noEmit` pass; `cargo clippy -- -D warnings` clean).
- **Working tree:** clean after commits.

## What's done

### PR 1 — Durable metadata + incremental crawl
**Files:** `src-tauri/Cargo.toml`, `src-tauri/src/indexer/metadata_db.rs` (new), `metadata.rs`, `mod.rs`

- Added `rusqlite` (bundled) dependency.
- `MetadataDb`: SQLite WAL store at `~/.config/kensaku/metadata.db`. Schema: `nodes(path, name, extension, size, modified, is_dir)`. Methods: open, load_all, upsert, remove, clear, count.
- `MetadataStore` now has `db: Option<MetadataDb>` field. `new()` opens DB and pre-loads all rows into in-memory structures (warm start). `add`/`remove`/`clear` mirror to DB (errors logged, never panic).
- `load_from_db()` method. `get()` method added.
- Incremental crawl: skip `add` + content indexing when existing entry has identical `modified` and `size`.
- Stale pruning: after walk, remove metadata entries whose paths weren't crawled (deleted files).
- `clear()` now actually clears both memory and DB (was dead code).

### PR 2 — Identity + Tantivy purge + watcher fix
**Files:** `walker.rs`, `metadata_db.rs`, `content.rs`, `watcher.rs`, `mod.rs`

- **(dev, ino) identity:** `FileEntry` gained `dev: u64, ino: u64` fields. DB schema migrated with `ALTER TABLE ADD COLUMN` (idempotent, ignores "duplicate column"). `nodes_dev_ino` index added. Walker and watcher populate dev/ino via `std::os::unix::fs::MetadataExt` (non-unix defaults to 0).
- **Tantivy stale purge:** `content.rs` gained `purge_stale(&HashSet<PathBuf>)` — uses `AllQuery` + `DocSetCollector` to find all indexed paths, deletes those not in the valid set, commits once. Called in `mod.rs` after crawl + stale metadata pruning.
- **Watcher rename handling:** `Modify(Name(RenameMode::Both))` → remove old path + upsert new. `From` → remove. `To` → upsert. Unknown rename modes → reconcile by existence check. `Remove` on a directory → recursive removal of all descendants from metadata + Tantivy.
- **Orphan guard:** `handle_file_upsert` skips paths whose parent no longer exists in metadata.
- **Single-watcher lifecycle:** `WatcherHandle` struct (stop flag + join handle, `Drop` stops). `IndexerManager` stores `watcher_handle: Arc<Mutex<Option<WatcherHandle>>>`. `start_indexing` stops/drops any existing watcher before crawling, then stores the new handle after crawl completes. `is_running` cleared only after handle is stored (no duplicate-watcher race).
- **Post-crawl ordering:** crawl → stale metadata prune → Tantivy purge → start watcher → clear is_running.

### PR 3 — Query layer + ranking + preview + actions
**Files:** `src-tauri/src/query/` (new module: `ast.rs`, `lexer.rs`, `parser.rs`, `router.rs`, `scope_trie.rs`, `mod.rs`), `src-tauri/src/search/mod.rs`, `src-tauri/src/search/fuzzy.rs`, `src-tauri/src/indexer/content.rs`, `src-tauri/src/indexer/metadata.rs`, `src-tauri/src/indexer/metadata_db.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/App.tsx`, `src/types.ts`, `src/components/FilterChips.tsx` (new), `src/components/PreviewPane.tsx` (new), `src/components/ActionPalette.tsx` (new)

- **Lexer + AST (`query/ast.rs`, `query/lexer.rs`):** tokenizes query strings with quotes, `key:value` pairs, `-` negation, and parentheses into `Query { terms, filters, scopes, mode, raw, hints, active_filters_dto }`.
- **Filter grammar (`query/parser.rs`):** parsed filters for `type:`, `ext:`, `in:`, `path:`, `modified:` (`<7d`, `>30d`, `7d..30d`), `size:` (`>2mb`, `<100kb`), `is:` (`image`, `dir`, `file`), `lang:` (`rust`, `python`, etc.), `-` negation (`-node_modules`, `-ext:log`), `"exact phrase"`, `re:"regex"`, `name:config`, `content:timeout`. Unknown `key:` treated as literal search terms (never 0 results); parse errors emit inline hints.
- **Directory trie + DFS scope cache (`query/scope_trie.rs`):** builds a folded directory name trie from `MetadataStore`; resolves `in:` and leading directory tokens using exact > prefix > fuzzy precedence. Ambiguous matches retain search term and display "did you mean 📁 dir?" hints. Caches subtree descendant candidate lists per `(dir_id, generation)` with DFS over child lists ($O(\text{subtree})$ instead of $O(\text{corpus})$).
- **Router (`query/router.rs`):** 0ms zero-model classifier for query shape: code identifiers, content phrases / regex, filtered scopes, NL stopwords / long queries, trailing `/` folder navigation.
- **RRF fusion + ranking (`search/mod.rs`):** replaced magic constants with Reciprocal Rank Fusion ($k=60$) over fuzzy and BM25 tiers. Added bonuses: `+0.20` exact substring in name, `+0.15` prefix in name, `+0.10` smooth recency exponential decay ($\exp(-\text{age}/7)$), `+0.08` frecency ($\ln(1+\text{opens}) \cdot \exp(-\text{last\_open}/14)$), `-0.02` depth penalty. Deterministic tiebreak by score then path.
- **Frecency usage tracking (`metadata_db.rs`, `metadata.rs`, `commands.rs`):** persistent SQLite `usage(path, opens, last_open)` table updated whenever a file is opened.
- **Char-bloom prefilter & top-k heap (`search/fuzzy.rs`):** 64-bit character presence bitmask per filename, AND-checked against query chars before nucleo-matcher runs. Swapped full sorting with `select_nth_unstable_by` for $O(N)$ top-k selection.
- **Post-rank snippet extraction (`indexer/content.rs`):** decoupled snippet extraction from Tantivy search; snippets are extracted only for the top-20 ranked results using an in-memory 200-file LRU content cache.
- **Request sequence guard (`commands.rs`, `src/App.tsx`):** atomic monotonic sequence numbers on queries; client drops stale responses if a newer request finishes first.
- **Tauri event indexing status:** replaced 1.5s interval polling loop with live `indexing-status` Tauri events emitted by the indexer.
- **UI additions (`src/App.tsx`, `FilterChips.tsx`, `PreviewPane.tsx`, `ActionPalette.tsx`):**
  - Interactive filter chips below input showing active filters with click-to-remove.
  - Right-side preview pane with syntax text preview, match highlights, image thumbnails, PDF badge, folder contents, and quick action buttons.
  - `>` command mode in input with suggestions and execution for `>reindex`, `>doctor`, `>model`, `>add folder`, `>settings`.
  - `⌘K` / `Ctrl+K` action palette on selected results: Open, Reveal, Copy Path, Copy Content, Ask, Find similar.

### PR 4 — Semantic search (static embeddings)
**Files:** `src-tauri/src/embed/` (new module: `mod.rs`, `model.rs`, `units.rs`, `scanner.rs`, `queue.rs`), `src-tauri/src/indexer/metadata_db.rs`, `src-tauri/src/indexer/metadata.rs`, `src-tauri/src/indexer/mod.rs`, `src-tauri/src/search/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/config.rs`, `src-tauri/src/lib.rs`, `src/types.ts`, `src/components/ResultItem.tsx`, `src/App.tsx`, `src/components/PreviewPane.tsx`

- **Static Embedder (`embed/model.rs`):** 256-dimensional feature hashing embedder with subword trigram coverage, $L_2$ unit normalization, and int8 quantization (-127..=127). Includes fast integer cosine similarity `cosine_similarity_i8` executing in ~5µs per query with zero network requirement.
- **Unit chunking & storage (`embed/units.rs`):** text chunking adhering to architecture spec: $\le 240$ tokens creates a single doc-level unit (`0..file_size`), longer files create 240-token windows with 48-token overlap (max 20 units per file). In-memory `UnitStore` maintains contiguous `Vec<i8>` vectors and unit-to-node mapping.
- **Database schema changes (`indexer/metadata_db.rs`):** added `units` table (`id`, `node_id`, `byte_start`, `byte_end`, `model_id`, `vec BLOB`) and `meta` table (`model_id`, `embedding_dim`). Migrated `nodes` table with `indexed_gen` and `content_hash`. Added batch unit loading, node deletion cascade, and file update triggers.
- **Parallel vector scanner (`embed/scanner.rs`):** 8-way parallel scan over contiguous unit vectors using Rayon. Computes cosine similarity, groups hits by `node_id` keeping the highest-scoring unit and byte range, and selects top-k hits via `select_nth_unstable_by`.
- **3-tier RRF fusion (`search/mod.rs`):** integrated semantic vector search as the third rank tier alongside fuzzy and BM25:
  $$\text{RRF score} = \sum \frac{w_{\text{tier}}}{60 + \text{rank}}$$
  Dynamic route weighting gives $1.5\times$ weight to the semantic tier when `route == QueryRoute::SemanticSearch`. Graceful degradation when semantic search is disabled or no units are present.
- **Background embedding queue (`embed/queue.rs`, `indexer/mod.rs`):** background worker bounded to max 2 threads via dedicated Rayon thread pool. Yields immediately when queries are in flight (`query_in_flight: Arc<AtomicBool>`). Resumable from cursor in `nodes.indexed_gen`. Deduplicates vector computation across identical files by `content_hash`. Updates indexing status and emits live Tauri events.
- **UI & frontend integration (`src/types.ts`, `ResultItem.tsx`, `App.tsx`, `PreviewPane.tsx`):**
  - Subtle `≈ semantic` badge on results discovered only via the semantic tier (`MatchType::Semantic`).
  - Bottom status bar shows semantic state (`"semantic (embedding...)"` during indexing, `"semantic ✓"` with unit count when active).
  - Preview pane highlights lines in the semantic match's `best_range`.
  - Added `>model` CLI output showing active model parameters and unit count; updated `>doctor` diagnostic report.

### PR 5 — Documents (PDF, Office, eBooks)
**Files:** `src-tauri/src/indexer/extract/` (new module: `mod.rs`, `pdf.rs`, `office.rs`, `text.rs`), `src-tauri/src/indexer/content.rs`, `src-tauri/src/indexer/mod.rs`, `src-tauri/src/search/mod.rs`, `src-tauri/src/commands.rs`, `src-tauri/Cargo.toml`, `src/types.ts`, `src/components/ResultItem.tsx`, `src/components/PreviewPane.tsx`

- **Magic Byte Sniffing (`extract/mod.rs`):** Sniffs file headers before trusting extensions (`%PDF-` for PDFs, `PK\x03\x04` for Zip-based office documents, `{\rtf` for RTF).
- **PDF Text Layer Extraction (`extract/pdf.rs`):** Pure-Rust extraction using `lopdf`. Checks for encryption (`is_encrypted()`) and returns explicit state (`"[Encrypted PDF: Password Protected]"`). Extracts page-by-page text with headers (`--- Page N ---`), tracks total page counts, and flags scanned/image-only PDFs (`is_scanned: true`). Fully protected by panic isolation (`std::panic::catch_unwind`).
- **Office Document Parsing (`extract/office.rs`):** Streaming pull-parser using `quick-xml` and `zip` (immune to XML external entity / billion-laughs expansion):
  - `.docx`: extracts text from `word/document.xml` with paragraph separation (`<w:p>`).
  - `.pptx`: extracts text from all `ppt/slides/slide*.xml` with numeric slide ordering and headers.
  - `.xlsx`: resolves `xl/sharedStrings.xml` and parses sheet cells from `xl/worksheets/sheet*.xml`.
  - `.epub`: parses chapter `.xhtml` and `.html` files in zip archive and strips HTML markup.
  - `.odt` / `.ods` / `.odp`: extracts content from `content.xml`.
- **Text & Encoding Improvements (`extract/text.rs`):** UTF-16LE and UTF-16BE detection with BOM handling, markdown frontmatter stripping (`--- ... ---`), and RTF control tag stripping.
- **Content Policy Enforcement (ARCHITECTURE §5.2):** Files > 20MB are skipped (metadata-only); files $\le 1$MB undergo full text extraction; files 1MB–20MB use head 512KB + tail 512KB for plain text, or up to 1MB extracted text for documents.
- **Filter Integration (`search/mod.rs`):** Expanded type filters to support `type:pdf`, `type:doc` / `type:document` (docx, odt, rtf, pdf, epub), `type:sheet` / `type:spreadsheet` (xlsx, xls, ods, csv, tsv), `type:presentation` / `type:slide` (pptx, ppt, odp, key), and `type:ebook` (epub, mobi, azw3).
- **Document Previews & UI (`commands.rs`, `PreviewPane.tsx`, `ResultItem.tsx`):**
  - Preview pane renders extracted document text with search term highlighting and semantic match byte range highlights.
  - Displays document-specific badges: page counts for PDFs, slide deck badge for presentations, spreadsheet badge for sheets.
  - Visual icons for documents: rose for PDF, emerald table for spreadsheets, orange presentation for slides, sky blue for Word documents.

## Known issues (not blocking, but should be addressed)

1. **Watcher directory rename** only removes old descendants and upserts the new directory entry — it does NOT recursively rewrite child paths. Children appear after next crawl/restart.
2. **Unmounted drive pruning:** if a watched directory is unavailable at startup, the stale-pruning pass deletes its metadata AND Tantivy docs. Consider guarding pruning for missing roots.
3. **Non-UTF-8 path comparison in `purge_stale`:** compares `to_string_lossy()` forms. Could theoretically purge a valid non-UTF-8 path. Low risk in practice.
4. **Content index for unchanged files:** if a Tantivy doc is missing for an unchanged file (size+mtime match), it won't be re-indexed. Needs a content-gen check.
5. **DB migration leaves existing rows at dev=0, ino=0** until next crawl repopulates them.
6. **No tests exist** anywhere in the codebase.
7. **Static embedding upgrade:** feature hashing embedder validates full pipeline offline; can be swapped for potion-base-8M safetensors table when local weights loader is added.

## ~~PR 6 — Agent surface (MCP, CLI, daemon socket)~~ — SKIPPED (for now)

> **Status: Skipped.** The user has decided to defer the agent/MCP/CLI surface. It may be revisited later. The architecture for it is fully documented in `docs/ARCHITECTURE.md` §9 and §13 Phase 4. The query AST, filters, and search pipeline are already designed to serve as the MCP tool contract (see PROGRESS.md item 6 in the "What's done" section).

## What's next: PR 7 — Vision/OCR

Per `docs/ARCHITECTURE.md` §13, Phase 5. Scope:

1. **OCR pipeline (platform-native):**
   - macOS: Vision framework (`VNRecognizeTextRequest`) via `objc2-vision` crate.
   - Windows: Windows.Media.Ocr via `windows` crate.
   - Linux: optional `tesseract` binary (shell out, parse output).
   - OCR results stored as text units in the same embedding/index pipeline.
   - Priority: screenshots (the #1 searched image type).

2. **Image embedding (optional, gated behind "smart" pack):**
   - Use SigLIP or CLIP ViT-B/32 int8 (NOT MobileCLIP on x86 — community benchmarks show it's slower).
   - EXIF orientation handling before embedding.
   - Megapixel/size caps (reject >50MP, >20MB).
   - Perceptual hash dedup before embedding (collapse near-duplicates).

3. **Frontend:**
   - Image thumbnails in result cards.
   - OCR text shown in preview pane.
   - "screenshot of docker error" → returns matching image in top 5.

4. **Config:**
   - `ocr_enabled: bool` (default true on macOS/Windows, false on Linux unless tesseract found).
   - `vision_embedding_enabled: bool` (default false, opt-in).

### Then PR 8 — Ask/actions (Ollama integration, summarize, find similar, duplicates)
### Then PR 9 — Launch/packaging (CI, benchmarks, demo, docs)

## Key architecture reference

- `docs/ARCHITECTURE.md` — full design, data model, query pipeline, phases, budgets.
- `docs/EDGE-CASES.md` — 100+ edge cases by subsystem with P0/P1/P2 priority.

## How to build

```bash
cd /home/gochi/Projects/Kensaku
bun install          # frontend deps
cd src-tauri && cargo build   # Rust backend
cd .. && bun run tauri dev    # dev mode
```

## Key files

| File | Role |
|---|---|
| `src-tauri/src/indexer/metadata.rs` | MetadataStore (in-memory + DB) |
| `src-tauri/src/indexer/metadata_db.rs` | SQLite persistence layer |
| `src-tauri/src/indexer/mod.rs` | IndexerManager, crawl loop, watcher lifecycle |
| `src-tauri/src/indexer/walker.rs` | jwalk filesystem crawl |
| `src-tauri/src/indexer/watcher.rs` | notify-based file watcher |
| `src-tauri/src/indexer/content.rs` | Tantivy BM25 index |
| `src-tauri/src/indexer/extract/` | Document extraction (PDF, DOCX, PPTX, XLSX, EPUB) |
| `src-tauri/src/embed/` | Static embeddings, vector store & parallel scanner |
| `src-tauri/src/search/mod.rs` | Search dispatcher, 3-tier RRF merge, rank |
| `src-tauri/src/search/fuzzy.rs` | nucleo-matcher fuzzy search |
| `src-tauri/src/commands.rs` | Tauri IPC commands |
| `src-tauri/src/config.rs` | Config load/save |
| `src-tauri/src/lib.rs` | App setup, state, tray, shortcuts |
| `src/App.tsx` | Main UI component |

## Commit history (v2)

- `docs: add v2 architecture, edge-case catalog, and handoff state`
- `deps: add rusqlite for metadata persistence`
- `feat(indexer): durable SQLite metadata store with warm-load`
- `feat(indexer): incremental crawl with mtime/size skip + stale pruning`
- `feat(indexer): file identity by (dev, ino) in metadata and DB`
- `feat(content): purge stale Tantivy docs after crawl`
- `feat(watcher): rename/remove-directory handling + single-watcher lifecycle`
- `feat(query): lexer, AST, filter grammar, directory trie & router`
- `feat(search): RRF fusion, frecency bonus, char-bloom prefilter & post-rank snippets`
- `feat(ui): preview pane, filter chips, command mode, actions palette & live events`

## Notes for the next agent

- The codebase has no tests. When implementing PR 3+, consider adding tests for the query parser at minimum.
- The `MetadataStore` uses `path_index: HashMap<PathBuf, usize>` for in-memory lookups. The (dev, ino) identity fields are stored but not yet used for lookups — that's a future optimization.
- `excluded_extensions` config field is dead code (no callers). Could be wired up in PR 3 filters.
- `chrono`, `log`, `env_logger` deps are unused. `tokio` is used via Tauri's runtime but not directly.
- The frontend polls `get_indexing_status` every 1.5s forever. PR 3 should replace with events.
- `src-tauri/target/` is ~5GB. `cargo clean` if disk is tight.
