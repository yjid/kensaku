# Kensaku — Edge Cases & Correctness Catalog

> Full detail was delivered in the planning conversation. This is the condensed, actionable version organized by priority. **P0** = will make the app wrong or slow in normal use. **P1** = bites at scale or on some platforms. **P2** = polish/corner.

---

## A. Silent killers (ranked by impact)

1. **Rename/delete divergence** — ghost results, duplicates, unbounded index growth. (P0, was present in v0.1, partially fixed in PR 2)
2. **Full re-crawl on every launch** — app is "slow" for minutes after every boot. (P0, fixed in PR 1)
3. **O(n) fuzzy scan with full sort per keystroke** — 50–200ms at 1M files. (P0, planned PR 3)
4. **Disk reads inside the query path** — snippet extraction per hit, locks stall indexing. (P0, planned PR 3)
5. **Content tokenization of generated files** — lockfiles, .min.js, source maps explode the term dictionary. (P0)
6. **CJK tokenization** — default Tantivy tokenizer fails on no-space languages. (P0, Japanese app name!)
7. **Cloud placeholder files** (OneDrive/Dropbox/iCloud) — reading hydrates them, silently downloading gigabytes. (P0 on Windows/macOS)
8. **Case/Unicode normalization mismatch** — macOS NFD vs Linux/Windows NFC, case-sensitive vs insensitive FS. (P0)
9. **inotify watch exhaustion (ENOSPC)** — recursive $HOME typically fails; v0.1 only eprintln'd. (P0 Linux, partially fixed PR 2)
10. **Watcher overflow** (IN_Q_OVERFLOW, FSEvents MustScanSubDirs, Windows buffer) — missed events = silently stale index. (P0)
11. **Unbounded query concurrency / out-of-order results** — fast typing races. (P0, planned PR 3)
12. **Index schema drift** — reuse meta.json with fresh schema → wrong fields. (P0)

## B. By subsystem (condensed)

### Path identity
- P0: Key by (dev, ino), not path string. Store display_path + folded_key separately.
- P0: Non-UTF-8 filenames (Linux OsStr bytes) — keep OsString, lossy-convert only for rendering.
- P1: Windows MAX_PATH 260, reserved names (CON, NUL), trailing dots/spaces.
- P1: macOS NFD normalization, case-insensitive-but-preserving APFS, case-only renames.
- P1: Hardlinks — dedupe content extraction by inode.
- P1: Symlink loops, broken links, bind mounts, pseudo-filesystems (/proc, /sys, /dev).
- P1: inode reuse after delete — pair with birth time/size/mtime.

### Filesystem / mounts
- P0: Network mounts (NFS/SMB/sshfs) — mmap unreliable, no events, 10-100ms stat. Detect and degrade.
- P0: Only index S_ISREG — character devices (/dev/zero) hang forever on read.
- P1: Sparse files, files >2GB, huge directories (readdir batching).
- P1: Permission denied, ACLs, SELinux → skip + count + surface.
- P2: Antivirus on-access locks (EBUSY).

### Watcher / events
- P0: Recursive watch limit — raise max_user_watches, always have polling fallback.
- P0: Overflow → forced rescan. Treat as first-class event.
- P0: New subdirs must be watched on create (git clone invisible otherwise).
- P0: Start watcher before/during crawl; v0.1 started after (fixed PR 2).
- P0: Atomic-save pattern (tmp → rename). Filter junk: .swp, 4913, ~$*.docx, .DS_Store.
- P1: Event storms (git checkout, npm install) — coalesce per path, adaptive debounce.
- P1: Metadata-only events (chmod/atime) must NOT trigger content re-read.
- P1: Sleep/resume — replay from cursor; invalid → full rescan.
- P2: Duplicate events, multiple app instances (single-instance lock).

### Durability / index lifecycle
- P0: Durable metadata + fast warm-load (done PR 1).
- P0: Interrupted crawl → resumable, never present half-indexed as complete.
- P0: Disk-full during commit → abort cleanly.
- P1: Atomic config writes; corrupt config must not reset setup.
- P1: Re-index must purge removed roots and stale docs (partially done PR 1/2).
- P0: Deleted docs linger until merge — governed merge + garbage_collect at idle.
- P1: Panic in indexer thread must reset state and report.
- P1: Clock jumps/DST → recency ranking wrong.

### Content extraction
- P0: Size policy: <256KB full, 256KB-20MB head+tail, >20MB metadata only.
- P0: Encoding detection: BOM → UTF-16 → NUL heuristic → UTF-8 → Latin-1 fallback.
- P0: Binary detection must not kill UTF-16.
- P0: Cap line length (minified files have 2MB single lines).
- P0: Deny/downweight generated artifacts: lockfiles, .map, dist/, *.min.*, SQL dumps, base64.
- P1: Content dedupe by BLAKE3.
- P1: Append-only logs: index tail only.
- P1: Code-aware tokenizer: split camelCase/snake_case/kebab-case.
- P1: HTML/XML/Markdown: strip markup.
- P2: Unicode: zero-width, combining marks, RTL, emoji.

### Tantivy specifics
- P0: Never feed raw user input to QueryParser. Build Query objects from AST.
- P0: No OR-by-default scoring of whole corpus.
- P0: Avoid leading-wildcard, regex, Levenshtein in hot path.
- P0: Commit batching (2-5s / 1-2k docs), writer 128MB, wait_merging_threads at idle.
- P1: Drop ultra-high-cardinality tokens (hashes, base64).
- P1: path as STRING for exact delete, separate prefix field for scoping.
- P1: Snippets: fetch after ranking, top-20, blocking pool, LRU cache.
- P1: Deterministic sort: score then path tiebreak.
- P2: MmapDirectory on local disk only.

### Fuzzy / filename matcher
- P0: Never full-scan+full-sort per keystroke. Prefilter → top-k heap.
- P0: Precompute folded name/path at ingestion.
- P1: Users expect substring/prefix, not pure subsequence. Rank: exact > prefix > word-boundary > substring > subsequence.
- P1: Multi-token queries must be AND-ed across name+path.
- P1: 1-char queries: don't scan everything; require 2+ or fall back to frecency.
- P1: Typo tolerance only as fallback when results < k.
- P1: CJK: char-level fuzzy needs ngrams.
- P2: Acronym matching as bonus, not primary.

### Query / ranking
- P0: RRF fusion (k=60) instead of magic constants.
- P0: Cancellation: sequence numbers, abort in-flight, drop stale responses.
- P1: Filter grammar edge cases: quoted paths, -negation, autocomplete, invalid → hint.
- P1: Recency: smooth decay, not cliffs.
- P1: Result diversity: dedupe by content hash, collapse vendor trees.
- P1: Empty query → recent/frequent files.
- P2: Explainability: chips, match highlighting.

### Semantic layer
- P0: Store model_id + dim + normalized per index. Model change → resumable re-embed.
- P0: Normalize query and docs identically.
- P0: Chunking: doc-level for short, 240-token windows for long, max 20/file.
- P1: First-call model load is 100-500ms: warm on idle at startup.
- P1: ONNX threadpool: 2-4 threads, low priority, stop on battery/fullscreen.
- P1: Deletes in HNSW: tombstones + scheduled rebuild.
- P1: Model download: resume, checksum, proxy, offline, disk-space precheck.
- P1: load-dynamic (not static) to avoid AVX2 SIGILL.
- P2: Multilingual gap: en-only model + Japanese query = confidently wrong.
- P2: Embeddings are sensitive data at rest → 0700 dir.

### Images / OCR
- P0: Decode bombs: cap megapixels and bytes; reject CMYK/animated/HEIC; corrupted files must not panic.
- P0: EXIF orientation before hashing/embedding.
- P1: OCR first (platform APIs), CLIP second.
- P1: Screenshots are the dominant searched image.
- P1: Perceptual hash before embeddings for dedup.
- P2: SVG, PDF thumbnails, RAW.

### Documents
- P0: Parser must be isolated (worker thread + timeout, panic boundary).
- P0: XML entity expansion (billion-laughs) — disable external entities/DTDs.
- P0: Encrypted/scanned PDFs → explicit state, not silent failure.
- P1: Magic-byte sniffing before extension trust.
- P1: Store page/section offsets for jump-to-location.
- P1: DOCX track-changes/comments/headers, PPTX notes, XLSX shared strings.
- P2: Multi-column, ligatures, bad ToUnicode, RTL, rotated text.

### Concurrency / runtime
- P0: All CPU/IO search work on bounded blocking pool.
- P0: Snapshot semantics: one Searcher per query, no write locks during reads.
- P0: Interactive queries preempt indexing.
- P0: Bounded queues everywhere with explicit drop policy.
- P1: Graceful shutdown: flush commits, stop watcher, join threads.
- P1: Single instance; second launch focuses first.
- P2: Deadlock ordering documented.

### Memory
- P0: Don't store PathBuf per entry. ~40 bytes/entry (name + parent_id + attrs).
- P0: Measure RSS as PSS/USS.
- P1: Zero allocation in query hot loop; intern strings; reuse buffers.
- P1: Cap result payloads (50 × 300-char snippet).
- P1: ONNX arena never shrinks — disable for long-lived daemon.

### UI / IPC / platform
- P0: Escape file content before rendering (XSS from downloaded files). CSP null + HTML snippets = vulnerability.
- P0: Global shortcut on Wayland often fails — detect and offer fallback.
- P0: Pre-warm hidden window; recreating webview costs 200-800ms.
- P1: Virtualize result list.
- P1: IME composition — don't fire search mid-composition.
- P1: macOS: accessory activation policy, secure-input fields, full-disk-access.
- P1: Windows: Alt+Space collision, focus-stealing, DPI scaling, Defender slowdown.
- P1: Tray on Linux needs libappindicator/GNOME extension.
- P2: Screen readers, transparent window artifacts.

### Security / privacy
- P0: Default-deny secrets: ~/.ssh, ~/.gnupg, browser profiles, password managers, *.pem, *.key, .env.
- P0: open/reveal via OS APIs, not shell strings; reject paths outside watched roots.
- P1: Index dir permissions 0700; optionally exclude from Time Machine.
- P1: MCP: 0600 socket, auth token, read-only, path scope, no traversal, rate limit, audit log.
- P1: Prompt-injection hygiene: file contents are data, never instructions.
- P2: Logs must not dump full paths/content.

### Agent surface
- P0: mode: ranked | exhaustive — agents need pagination + total count.
- P0: Token-budgeted snippets (max_tokens), never whole files.
- P0: Freshness token + per-root as_of; fresh=true to force reconcile.
- P0: Honor .gitignore/.cursorignore — agents assume parity.
- P1: Stable versioned JSON schema; unknown filter → error.
- P1: Backpressure/concurrency limits.

## C. Perceived-speed rules

- Instant first paint: render previous results during computation.
- No layout shift: fixed row heights, reserved thumbnails.
- No reordering while typing.
- Fire on Enter immediately (skip debounce), and on paste.
- Empty state = recent files, rendered before any query.
- Prefetch snippet/thumbnail for top 5 asynchronously.
- Show staleness honestly ("index 4s behind").

## D. Test matrix (minimum)

| Axis | Cases |
|---|---|
| FS type | ext4, APFS (case-insensitive), NTFS, exFAT, NFS/SMB, FUSE |
| Scale | 10k / 100k / 1M files; 100k in one dir |
| Names | non-UTF-8, NFC vs NFD, emoji, CJK, 255-byte, CON/trailing dot, newline |
| Content | UTF-16, Latin-1, minified 2MB line, 50MB log, binary-as-text, empty, zero-byte |
| Docs | encrypted PDF, scanned PDF, 2000-page PDF, malformed PDF, DOCX track changes, XLSX hidden sheets |
| Events | rename chain, dir rename, dir delete, git checkout, npm install, overflow, watch-limit, sleep/resume |
| Chaos | kill -9 mid-crawl, disk full, permission flip, clock jump, index dir deleted at runtime |
| Perf | cold vs warm cache, p50/p95/p99, keystroke→paint, 24h RSS drift |

## E. Acceptance gates

- recall@5 on frozen 50-query golden set ≥ 0.90
- Zero ghost results after 1,000 rename/delete ops
- Freshness p95 < 2s
- p50 keystroke→pixels < 15ms at 1M files
- Idle RSS < 60MB at 1M
- MCP search p95 < 50ms
- Indexing throughput > 2,000 files/s metadata, > 150 files/s text content (image
  content is OCR-bound; see [BENCHMARKS.md](BENCHMARKS.md#indexing-and-extraction-reference))
