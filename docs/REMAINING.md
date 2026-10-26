# Kensaku — Remaining Work

> State at `v0.3.3`. This is the **engineering backlog**: everything not done
> yet, ordered by impact, with the reason it is deferred where relevant.
>
> Product direction, interaction model, sequencing, and acceptance tests live in
> [VISION.md](VISION.md). When the two disagree, VISION owns *what* and *why*;
> this file owns *how much is left*.

---

## Shipped for context

Durable metadata + warm start · incremental crawl · inode identity ·
rename/delete handling · full query language (`type:`, `ext:`, `in:`, `path:`,
`modified:`, `size:`, `is:`, `lang:`, `name:`, `content:`, `re:`, negation,
`mode:exhaustive`) · three-tier RRF ranking · semantic vectors · PDF/Office/
EPUB/ODT/RTF extraction · tesseract OCR · image thumbnails, dimensions, and
perceptual duplicates · CLI + MCP · command palette · two themes · Doctor ·
Stats for Nerds · factory reset · extraction timeout · cloud-placeholder guard ·
single-instance lock · 77+ Rust tests and CI.

Also shipped in the `v0.3.x` hardening cycle: image/PDF exclusion migration,
thumbnail asset scope fix, snippets no longer invoke the full extractor
(3.86 s → 0.14 s), metadata DB vacuum (310 MB → 604 KB), OCR toggle persistence,
and OCR language download.

---

## A. Product experience

These are the highest-leverage gaps. None require a new model.

- [ ] **Natural-language intent compiler** — deterministic, local, no LLM:
      recency words ("latest", "newest", "old download"), size words ("big",
      "huge", "under 1 MB"), time windows ("last week", "in March"), place
      phrases ("in Projects"), media/document nouns, error/log nouns. Emits the
      existing `Query` AST plus `Sort`.
- [ ] **Aliases and personal vocabulary** — `resume → resume, cv, curriculum`;
      `invoice → invoice, receipt, bill`; user-editable; seeded by use-case
      preset. Suggestions from behavior, never silent rewrites.
- [ ] **First-class sort** — `Best match | Newest | Oldest | Largest | Name`.
      Recency is currently a ranking bonus, so "latest" does not actually sort.
- [ ] **Interpretation chips** — expose the parsed intent as removable chips,
      including sort, scope, type, and aliases. Low confidence asks with chips,
      not a chat message.
- [ ] **Version grouping** — collapse `resume_v1`, `resume_final`,
      `resume_final_final` into one row with a version count. False grouping is
      worse than none; ship conservative heuristics.
- [ ] **Facet counts** — server-side counts for type, language, place, time,
      and size. The current `categoryCounts()` is client-side over the returned
      page only.
- [ ] **Multi-select and exclude facets** — OR within a facet, AND across
      facets, alt-click to exclude. Filters must never silently zero results.
- [ ] **Saved searches / smart folders** — "My screenshots", "Work invoices",
      "This project's logs". User-created first; auto-suggestion optional and
      never silent.
- [ ] **`/` command menu** — `/images`, `/pdf`, `/docs`, `/recent`, `/dupes`,
      `/errors`, `/diagrams`, `/reindex`, `/doctor`, `/settings`, `/ask`.
- [ ] **`@` scope mentions** — `@Projects`, `@Downloads`, `@Work`, `@Photos`,
      `@me`; recent places first; resolved against the existing directory trie.
- [ ] **New home and result states** — recent files, recent queries, pins,
      suggestions, best-guess card, grouped versions, related files.
- [~] **Duplicate cleanup UI** — `>dupes` now prints grouped exact-content and
      near-image duplicates. A dedicated review/delete view is still missing.
- [x] **Guided first-run setup flow** — welcome → folders → exclusions →
      preferences → optional packs → first scan → Done, with resume after an
      interrupted scan.
- [~] **Setup-owned downloads and configuration** — folders, exclusions, theme,
      and shortcut are asked in setup; OCR/model downloads are offered there but
      still also live in Settings. Vision Packs and a Settings status-only view
      remain.
- [x] **First-scan completion gate** — `first_scan_done` gates `is_setup_complete`;
      the scan shows live counts and a Done summary, resumes after interruption,
      and later launches stay search-first. (Current path and ETA are not yet
      shown.)
- [ ] **Voice and microcopy pass** — human, anticipatory, quiet. No chat
      persona, no "How can I help you today?".

## B. Intelligence & vision

- [ ] **ONNX "smart pack"** — `fastembed`/`ort` with `load-dynamic`,
      bge-small or EmbeddingGemma-Q4, activation after download. The potion
      table is already used; this is the quality tier above it.
- [ ] **Image embeddings** — SigLIP or CLIP ViT-B/32 int8 for "red cap",
      "system design diagram", "whiteboard". Today images are searchable through
      OCR text and filename only. Wire as a fourth retrieval tier into RRF.
- [ ] **Similar images** — image-to-image query from a selected result; reuse
      the same visual index and the related-file response shape.
- [ ] **Native OCR** — macOS Vision (`objc2-vision`) and Windows OCR (`windows`
      crate). Today it shells out to `tesseract`, which may not be installed.
      Scanned PDFs need the same path.
- [ ] **"This is me" identity** — local face embeddings + clustering, opt-in
      enrollment with 3–5 photos, explainable matches, delete-all. Sensitive
      data: local only, never uploaded; ship after the sensitive-path policy.
- [ ] **VLM captions (optional)** — idle-time captions for diagrams and
      screenshots, stored as searchable text. Complements CLIP where short text
      queries are weak.
- [ ] **Multilingual embeddings + CJK tokenization** — the default model is
      English-centric and Tantivy's default tokenizer fails on no-space
      languages. Both are correctness gaps for a global product.

## C. Agent, LLM & runtime

- [ ] **Warm daemon** — one process owns the index; GUI, CLI, and MCP attach.
      Today the GUI backend is in-process and a one-shot CLI pays metadata load.
- [ ] **Local HTTP/SSE endpoint** — loopback only, token auth, read-only.
      Needed for web-capable MCP clients and long-lived agents.
- [ ] **One-click agent connect** — generated configs and live status for
      Claude Desktop/Code, Cursor, and other MCP clients. Keep a manual JSON
      fallback in Settings → Agents.
- [ ] **Local LLM query compiler** — optional Ollama/LM Studio. Natural
      language → strict JSON AST, validated, cached, shown as chips. The LLM
      never retrieves on its own.
- [ ] **`/ask` with citations** — local model answers from retrieval results,
      with path and line citations. No retrieval, no answer. No cloud by
      default; per-use consent if a cloud provider is configured.
- [ ] **Agent surface v2** — `find_symbol` (tree-sitter), `.gitignore` /
      `.cursorignore` parity, pagination beyond the 10k exhaustive cap,
      optional read-only scoping per root, daemon-backed MCP.
- [ ] **Image/agent tools** — expose visual search and similar-image lookup
      once the visual tier exists; keep snippets token-budgeted.

## D. Search quality & performance

- [ ] **E2 extension interning** — `FileEntry.extension: Option<String>` is
      128 B per entry. A `u16` intern table saves ~6–12 MB per 1M files.
- [ ] **B4 full pushdown** — filters are post-filters in Tantivy. Add `ext` /
      `size` fast fields and build a `BooleanQuery` so only matching docs are
      scored. Required for instant facets and the 1M gate.
- [ ] **1M-file prefilter** — 100k is ~2.8 ms; 1M is projected ~28 ms. A
      trigram/prefix index or sharded scan is needed to hold <15 ms.
- [ ] **Metadata arena** — still `Vec<FileEntry>` + `HashMap<PathBuf, usize>`.
      A compact arena (name bytes + parent id + attrs ≈ 40 B/entry) cuts ~3×.
- [ ] **Tantivy writer pipeline** — extraction is parallel; `add_document` and
      commits are still single-threaded.
- [ ] **Golden query set in CI** — a frozen set of 40–50 labelled queries with
      recall@5, MRR, NDCG, and latency thresholds. Include intent queries
      ("latest resume", "screenshots from last week", "docker error in
      Projects") alongside lexical and semantic ones.
- [ ] **Facet latency tests** — counts and toggles must hold their budgets at
      100k and 1M files.

## E. Correctness, privacy & robustness

- [x] **Default-deny sensitive paths** — hidden entries are skipped; `.ssh`,
      `.gnupg`, `.aws`, `.kube`, `.docker`, `.password-store`, password-manager
      folders, `.env*`, and credential extensions (`.pem`, `.key`, `.pfx`, …)
      are denied by default in the walker/config. Browser profiles under hidden
      app-data directories are covered by the hidden-entry skip.
- [ ] **Non-UTF-8 paths** — `purge_stale` compares lossy strings; a `bytes`
      path field or raw `OsStr` keying would make it exact.
- [ ] **Platform indexers** — Windows USN journal and macOS FSEvents
      `sinceWhen` replay. Today only generic notify events + reconcile.
- [ ] **Reset smoke test** — the marker/restart path is verified by inspection.
- [ ] **Doctor depth** — disk-space headroom and per-root permission checks.
- [ ] **Face-data controls** — explicit delete-all, storage location, and an
      audit trail before the feature ships.
- [ ] **Cloud-consent UX** — a visible indicator whenever a query or snippet
      leaves the device; local-only remains the default.

## F. Packaging & launch

- [ ] **Signing** — macOS codesign + notarization, Windows Authenticode.
      Needed before shipping an ONNX dylib and for a non-scary installer.
- [ ] **Auto-updater** — Tauri updater plugin + signing keys + release channel.
- [ ] **Icons / tray artwork** — still placeholder Tauri icons.
- [ ] **Demo GIF + published benchmark post** — the numbers and methodology
      exist in `docs/BENCHMARKS.md`; the artifacts do not.
- [ ] **Release workflow assets** — CLI binaries currently upload as workflow
      artifacts; moving them to the draft release needs `needs:` + `gh release
      upload` plumbing.
- [ ] **Vision Pack packaging** — model download, checksum, resume, disk-space
      precheck, offline handling, and a one-click uninstall.

---

## Milestone mapping

| Milestone (VISION) | Primary backlog here |
|---|---|
| 1 — It understands me | A (all), E (sensitive paths), D (golden set) |
| 2 — It filters like lightning | A (facets, saved searches), D (pushdown, 1M, arena) |
| 3 — It sees | B (all), E (face controls), F (Vision Pack packaging) |
| 4 — It talks to your AI | C (all), E (cloud consent) |
| 5 — Ship it properly | F (all) |

---

## Sequencing notes and prior deferrals

| Item | Why |
|---|---|
| Extension interning (E2) | High churn across 7 files for 6–12 MB at 1M files. |
| Tantivy BooleanQuery pushdown (B4) | Needs a schema change and a full index rebuild; now required for Milestone 2. |
| ANN index (USearch) | Exact i8 scan is enough below ~400k units; revisit at 1M. |
| Cross-encoder reranker | 30–60 ms for 20 pairs does not fit the latency budget. |
| Bundled multi-GB LLM | Product decision: no giant dependency. Optional local Ollama/LM Studio provides intent compilation and `/ask`. |
| Cloud sync / accounts | Violates the local-first thesis; no cross-device state planned. |
| Chat-first UI | Chat is slower than search for the core job; answers are a mode (`/ask`), not the surface. |
| Bundled ONNX runtime by default | The static table keeps the base binary small; neural and vision tiers are opt-in packs. |
| Video/audio transcription | Different pipeline and storage budget; not required for the memory thesis yet. |
