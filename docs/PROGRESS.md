# Kensaku — Progress & Handoff

> **For the next agent/session:** this file is the single source of truth for what's done, what's next, and how to continue. Read `docs/ARCHITECTURE.md` for the full design, `docs/EDGE-CASES.md` for the correctness catalog.

---

## Current state

- **Branch:** `master`
- **Last commit before v2 work:** `d690ada` (initial release v0.1)
- **v2 work:** PR 1 and PR 2 implemented and committed (see below). Builds clean (`cargo build` passes). No tests exist.
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

## Known issues (not blocking, but should be addressed)

1. **Pre-existing clippy warnings** (10 total) in `commands.rs`, `config.rs`, `lib.rs`, `content.rs` (snippet code), `mod.rs` (crawl nested ifs), `search/fuzzy.rs`. None introduced by PR 1/2. Clippy `-- -D warnings` fails repo-wide.
2. **Watcher directory rename** only removes old descendants and upserts the new directory entry — it does NOT recursively rewrite child paths. Children appear after next crawl/restart.
3. **Unmounted drive pruning:** if a watched directory is unavailable at startup, the stale-pruning pass deletes its metadata AND Tantivy docs. Consider guarding pruning for missing roots.
4. **Non-UTF-8 path comparison in `purge_stale`:** compares `to_string_lossy()` forms. Could theoretically purge a valid non-UTF-8 path. Low risk in practice.
5. **Content index for unchanged files:** if a Tantivy doc is missing for an unchanged file (size+mtime match), it won't be re-indexed. Needs a content-gen check.
6. **DB migration leaves existing rows at dev=0, ino=0** until next crawl repopulates them.
7. **No tests exist** anywhere in the codebase.

## What's next: PR 3 — Query layer

Per `docs/ARCHITECTURE.md` §13, Phase 1. Scope:

1. **Lexer + AST** (`src-tauri/src/query/` new module): parse query string into `Query { terms, filters, scopes, mode, raw }`.
2. **Filter grammar:** `type:`, `ext:`, `in:`, `path:`, `modified:`, `size:`, `is:`, `lang:`, `-negation`, `"phrases"`, `re:regex`.
3. **Directory trie + DFS scope cache:** resolve `in:` and leading directory tokens against a trie of indexed directories. Cache scope candidate lists per `(dir_id, generation)`.
4. **Router:** zero-model intent classification (token count, camelCase/snake_case, NL stopwords, regex delimiters).
5. **RRF fusion:** replace magic `×2`/`×15` constants with RRF (k=60) over fuzzy + BM25 + (later) semantic. Add exact-substring/prefix bonuses, smooth recency decay, frecency, depth penalty.
6. **Deterministic tiebreak:** sort by score then path.
7. **Top-k heap:** replace full sort in fuzzy matcher with `select_nth_unstable`.
8. **Char-bloom prefilter:** 64-bit letter-bucket mask per name, AND against query char set before matcher runs.
9. **Snippets post-rank:** move snippet extraction out of query path; fetch after ranking, top-20, blocking pool, LRU cache.
10. **UI:** filter chips (click to remove), preview pane, `>` command mode, `⌘K` actions, events instead of 1.5s polling, request sequence guard.

### Then PR 4 — Semantic (static embeddings)
### Then PR 5 — Documents (PDF, Office)
### Then PR 6 — Agent surface (MCP, CLI, daemon socket)
### Then PR 7 — Vision/OCR
### Then PR 8 — Ask/actions
### Then PR 9 — Launch/packaging

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
| `src-tauri/src/search/mod.rs` | Search dispatcher, merge, rank |
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

## Notes for the next agent

- The codebase has no tests. When implementing PR 3+, consider adding tests for the query parser at minimum.
- The `MetadataStore` uses `path_index: HashMap<PathBuf, usize>` for in-memory lookups. The (dev, ino) identity fields are stored but not yet used for lookups — that's a future optimization.
- `excluded_extensions` config field is dead code (no callers). Could be wired up in PR 3 filters.
- `chrono`, `log`, `env_logger` deps are unused. `tokio` is used via Tauri's runtime but not directly.
- The frontend polls `get_indexing_status` every 1.5s forever. PR 3 should replace with events.
- `src-tauri/target/` is ~5GB. `cargo clean` if disk is tight.
