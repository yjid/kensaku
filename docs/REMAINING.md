# Kensaku — Remaining Work

> State at `v0.2.0`: durable incremental index, query language, 3-tier hybrid
> search, document extraction, OCR, full v2 UI with two themes, settings with
> Doctor / Stats for Nerds / model download / factory reset.
> Everything below is *not* done yet. Ordered by impact.

---

## A. Functional gaps (highest impact)

- [ ] **Wire the downloaded model into the ranker.** `models.rs` downloads the
      potion-base-8M table, but `embed/model.rs` still uses the built-in
      hashing estimator. Parse `model.safetensors` + `tokenizer.json` and use
      them when present (fallback to hashing otherwise). This is the single
      biggest gap between the AI story and the behavior.
- [ ] **ONNX "smart pack"** — `fastembed`/`ort` with `load-dynamic`, bge-small
      or EmbeddingGemma-Q4, first-run activation. The UI already has the
      download surface and the honest copy.
- [ ] **Image embeddings** (SigLIP or CLIP ViT-B/32 int8) for "search images by
      description". Today images are searchable only through OCR text.
- [ ] **Native OCR**: macOS Vision (`objc2-vision`) and Windows OCR (`windows`
      crate). Today it shells out to `tesseract`, which may not be installed.
- [ ] **Image thumbnails** in result rows/cards, image dimensions in metadata,
      perceptual-hash dedup before embedding.
- [x] **Agent surface** — shipped: `kensaku-cli` with `search`, `state`, and an
      MCP stdio server exposing `search`, `read_snippet`, `index_state`.
      See `docs/AGENTS.md`. No in-app LLM by design.
- [ ] **Agent surface v2** — daemon/local socket so one-off CLI calls avoid
      metadata load, `find_symbol` (tree-sitter), `.gitignore`/`.cursorignore`
      parity, pagination beyond the 10k exhaustive cap.

## B. Search quality & performance

- [ ] **E2 extension interning** — `FileEntry.extension: Option<String>` is
      128 B/entry. `u16` intern table saves ~6–12 MB per 1M files.
- [ ] **B4 full pushdown** — filters are post-filters in Tantivy today. Add
      `ext` / `size` fast fields and build a `BooleanQuery` so only matching
      docs are scored.
- [ ] **A4 remainder** — `name:` / `content:` / `path:` route correctly for
      fuzzy/BM25, but BM25 still receives name-only terms. Split the query
      per engine fully.
- [ ] **`re:` and `mode:exhaustive`** are parsed but not executed. Implement or
      remove from the cheat sheet.
- [ ] **Golden query set + CI gates** — 50 labelled queries, recall@5/MRR/NDCG,
      latency thresholds in CI. `src/bin/bench.rs` exists but is manual.
- [ ] **1M-file benchmark** — 100k numbers are good (p50 ~2.8 ms fuzzy); 1M is
      extrapolated (~28 ms). Needs a trigram/prefix prefilter or a sharded scan
      to hold the <15 ms promise.
- [ ] **Metadata arena** — still `Vec<FileEntry>` + `HashMap<PathBuf, usize>`.
      A compact arena (name bytes + parent id + attrs ≈ 40 B/entry) would cut
      memory ~3×.
- [ ] **Semantic scan prefilter** — `ExactScanner` scans all units, then filters
      by candidate set. Pass the scope mask in.
- [ ] **Tantivy writer pipeline** — extraction is parallel; `add_document` and
      commits are still single-threaded.

## C. Correctness & robustness

- [ ] **No tests anywhere.** Add: query lexer/parser unit tests, watcher
      rename/delete integration tests, extraction corpus (malformed PDF/DOCX),
      config migration test.
- [ ] **Extraction subprocess** — `panic = "abort"` was removed so `catch_unwind`
      works, but a parser that *hangs* still blocks a worker. Add a timeout or
      run parsers in a child process.
- [ ] **Directory rename** in the watcher removes old descendants and adds the
      new directory only; children reappear on next crawl. Rewrite subtrees.
- [ ] **Unmounted drives** — startup pruning deletes that root's entries and
      Tantivy docs. Guard pruning when a configured root is missing.
- [ ] **Single-instance guard** — two daemons can open the DB/Tantivy writer.
      Add a lock file / named mutex.
- [ ] **Atomic config save** — `fs::write` can truncate on crash; use
      temp + fsync + rename.
- [ ] **Cloud placeholders** — OneDrive/iCloud/Dropbox hydration on read is not
      detected. Check placeholder attributes and skip content extraction.
- [ ] **Platform indexers** — Windows USN journal and macOS FSEvents
      `sinceWhen` replay are not implemented; only generic notify events.
- [ ] **Non-UTF-8 paths** — `purge_stale` compares lossy strings.
- [ ] **Reset smoke test** — the marker/restart path is implemented but only
      verified by inspection.

## D. Packaging & launch (PR 9)

- [ ] **GitHub Actions matrix** — Linux AppImage/deb/AUR, macOS universal dmg,
      Windows msi/exe, with `bun run tauri build` verified per OS.
- [ ] **Signing** — macOS codesign + notarization (required once ONNX dylib is
      shipped), Windows Authenticode.
- [ ] **Auto-updater** + release channels.
- [ ] **Icons / tray artwork** — currently placeholder Tauri icons.
- [ ] **README rewrite** — the current README claims "preloaded index" and
      "<5 ms fuzzy / <50 ms content"; both are outdated. Publish the new
      numbers, the real bundle size, and the current feature set.
- [ ] **Benchmark publication + 15-second demo GIF.**
- [ ] **Crash reporting** — local ring buffer, opt-in export via Doctor.

## E. Cleanup

- [ ] Remove unused deps: `chrono`, `log`, `env_logger`, and direct `tokio`
      (Tauri provides its runtime).
- [ ] Settings placeholders that are disabled: start-at-login, smart pack
      activation, Agents/MCP.
- [ ] `find_symbol` (tree-sitter) for agents.
- [ ] Doctor could add disk-space and per-root permission depth checks.
- [ ] `excluded_extensions` now correctly excludes only non-indexable formats;
      re-check the docs/README wording.

---

## Deliberately deferred with rationale

| Item | Why |
|---|---|
| Extension interning (E2) | High churn across 7 files for 6–12 MB at 1M files. |
| Tantivy BooleanQuery pushdown (B4) | Needs schema change + full index rebuild. |
| ANN index (USearch) | Exact i8 scan is enough below ~400k units; revisit at 1M. |
| Cross-encoder reranker | 30–60 ms for 20 pairs does not fit the latency budget. |
| In-app LLM / "ask" panel | Product decision: Kensaku is the search backend for agents, not a chat app. The agent surface ships instead. |
