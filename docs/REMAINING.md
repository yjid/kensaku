# Kensaku — Remaining Work

> State at `v0.2.0` + the seven-PR hardening cycle. Everything in this file is
> **not done yet**, ordered by impact, with the reason it is deferred where
> relevant.

---

## Shipped in this cycle (for context)

Real potion embeddings · query semantics (`re:`, field scoping, exhaustive,
semantic prefilter) · robustness (single instance, atomic config, unavailable
roots, extraction timeout, directory rename, cloud placeholders) · tests + CI ·
agent surface (CLI + MCP) · image thumbnails/dimensions/perceptual duplicates ·
autostart · local crash log · unsigned release workflow · README rewrite.

---

## A. Functional gaps

- [ ] **ONNX "smart pack"** — `fastembed`/`ort` with `load-dynamic`, bge-small or
      EmbeddingGemma-Q4, activation after download. The 30 MB potion table is
      already downloaded and used; this is the quality tier above it.
- [ ] **Image embeddings** (SigLIP or CLIP ViT-B/32 int8) for "search images by
      description". Today images are searchable through OCR text and filename only.
- [ ] **Native OCR**: macOS Vision (`objc2-vision`) and Windows OCR (`windows`
      crate). Today it shells out to `tesseract`, which may not be installed.
- [ ] **Agent surface v2** — a local socket/daemon so one-off CLI calls skip the
      metadata load; `find_symbol` (tree-sitter); `.gitignore`/`.cursorignore`
      parity; pagination beyond the 10k exhaustive cap; optional read-only scoping
      per root.

## B. Search quality & performance

- [ ] **E2 extension interning** — `FileEntry.extension: Option<String>` is 128 B
      per entry. A `u16` intern table saves ~6–12 MB per 1M files.
- [ ] **B4 full pushdown** — filters are post-filters in Tantivy. Add `ext`/`size`
      fast fields and build a `BooleanQuery` so only matching docs are scored.
- [ ] **1M-file prefilter** — 100k is ~2.8 ms; 1M is projected ~28 ms. A
      trigram/prefix index or sharded scan is needed to hold <15 ms.
- [ ] **Metadata arena** — still `Vec<FileEntry>` + `HashMap<PathBuf, usize>`.
      A compact arena (name bytes + parent id + attrs ≈ 40 B/entry) cuts ~3×.
- [ ] **Tantivy writer pipeline** — extraction is parallel; `add_document` and
      commits are still single-threaded.
- [ ] **Golden query set in CI** — a frozen 50-query labelled set with recall@5,
      MRR, NDCG, and latency thresholds. Fuzzy relevance tests exist; the full
      harness does not.

## C. Correctness & robustness

- [ ] **Non-UTF-8 paths** — `purge_stale` compares lossy strings; a `bytes` path
      field or raw `OsStr` keying would make it exact.
- [ ] **Platform indexers** — Windows USN journal and macOS FSEvents `sinceWhen`
      replay. Today only generic notify events + reconcile.
- [ ] **Reset smoke test** — the marker/restart path is verified by inspection.
- [ ] **Doctor depth** — disk-space headroom and per-root permission checks.

## D. Packaging & launch

- [ ] **Signing** — macOS codesign + notarization, Windows Authenticode. Needed
      before shipping an ONNX dylib and for a non-scary installer.
- [ ] **Auto-updater** — Tauri updater plugin + signing keys + release channel.
- [ ] **Icons / tray artwork** — still placeholder Tauri icons.
- [ ] **Demo GIF + published benchmark post** — the numbers and methodology exist
      in `docs/BENCHMARKS.md`; the artifacts do not.
- [ ] **Release workflow assets** — CLI binaries currently upload as workflow
      artifacts; moving them to the draft release needs `needs:` + `gh release
      upload` plumbing.

---

## Deliberately deferred with rationale

| Item | Why |
|---|---|
| Extension interning (E2) | High churn across 7 files for 6–12 MB at 1M files. |
| Tantivy BooleanQuery pushdown (B4) | Needs a schema change and a full index rebuild. |
| ANN index (USearch) | Exact i8 scan is enough below ~400k units; revisit at 1M. |
| Cross-encoder reranker | 30–60 ms for 20 pairs does not fit the latency budget. |
| In-app LLM / "ask" panel | Product decision: Kensaku is the search backend for agents, not a chat app. |
| Bundled ONNX runtime by default | The static table keeps the base binary small; neural tier is opt-in. |
