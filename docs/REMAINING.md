# Kensaku — Remaining Work

> State at `v0.3.3`. This is the **engineering backlog**: everything not done
> yet, ordered by impact, with the reason it is deferred where relevant.
>
> Product direction, interaction model, sequencing, and acceptance tests live in
> [VISION.md](VISION.md). When the two disagree, VISION owns *what* and *why*;
> this file owns *how much is left*. Competitive and architectural context:
> [LANDSCAPE.md](LANDSCAPE.md). The ordered, testable next slice (daemon split,
> compact arena, thumbnail hashing, lazy snippets, folded search, OCR pack,
> scheduling) is [ROADMAP.md](ROADMAP.md).

---

## Shipped for context

Durable metadata + warm start · incremental crawl · inode identity ·
rename/delete handling · full query language (`type:`, `ext:`, `in:`, `path:`,
`modified:`, `size:`, `is:`, `lang:`, `name:`, `content:`, `re:`, negation,
`mode:exhaustive`) · three-tier RRF ranking · semantic vectors · PDF/Office/
EPUB/ODT/RTF extraction · tesseract OCR + optional PP-OCRv6 pack ·
image thumbnails, dimensions, and perceptual duplicates · CLI + MCP · warm daemon
with Unix socket / Windows named pipe · `/` command menu · two themes · Doctor ·
Stats for Nerds · factory reset · extraction timeout · cloud-placeholder guard ·
single-instance lock · 200 Rust tests, a golden-query integration test, and CI.

Also shipped in the `v0.3.x` hardening cycle and K1–K14 pass: compact metadata
arena (91.3 MB at 1M entries), folded/trigram content tier, visible-row snippets,
thumbnail-only hashing, i8 embedding table (real-model MRR parity), model/OCR
download hygiene, external extractors, screenshot-burst grouping, 104-image OCR
fixture, and a native quick-window spike. Earlier hardening: image/PDF exclusion migration,
thumbnail asset scope fix, snippets no longer invoke the full extractor
(3.86 s → 0.14 s), metadata DB vacuum (310 MB → 604 KB), OCR toggle persistence,
and OCR language download. The crawl is now a single extraction pass that feeds
Tantivy and the semantic index together (images are OCR'd once), with bounded
extraction slots, OMP-pinned tesseract children, streaming index commits, a
16 MB Tantivy writer arena, and perceptual hashing deferred to a background
one-decode-at-a-time pass. Dev builds optimize dependency code so profiling is
representative. See [BENCHMARKS.md](BENCHMARKS.md#indexing-and-extraction-reference).

---

## A. Product experience

These are the highest-leverage gaps. None require a new model.

- [x] **Natural-language intent compiler** — deterministic, local, no LLM:
      recency words ("latest", "newest"), size words ("big", "under 1 MB"),
      time windows ("today", "last week", "last month", "last year"), place
      phrases ("in Projects"), and media/document nouns. Emits the existing
      `Query` AST plus `Sort`. Month names and aliases remain.
- [x] **Aliases and personal vocabulary** — `resume → resume, cv, curriculum`;
      `invoice → invoice, receipt, bill`; user-editable in Settings → Search;
      seeded defaults; compiled to an OR-regex and shown as a removable chip.
- [x] **First-class sort** — `Best match | Newest | Oldest | Largest | Name`,
      applied server-side before the result limit.
- [x] **Interpretation chips** — every natural-language inference is shown as a
      removable chip (sort, scope, type, time, size). Explicit grammar is never
      reinterpreted.
- [x] **Version grouping** — conservative backend grouping of same-directory,
      same-stem files with version markers (`_v2`, `_final`, ` (1)`, `-copy`);
      one row with a version count and the other paths in a tooltip.
- [x] **Facet counts** — server-side counts over the full matched set for type,
      language, place, time, size, and recent; category chips consume them.
- [x] **Multi-select and exclude facets** — click includes, Alt-click excludes,
      OR within the category row, AND against the text query.
- [x] **Saved searches / smart folders** — save the current query + sort from
      the results bar; listed on the home screen; stored in config.
- [x] **Pagination / load-more** — server `offset` + a Load more control for
      ranked results.
- [x] **`/` command menu** — `/images`, `/pdf`, `/docs`, `/code`, `/media`,
      `/recent`, `/errors`, `/diagrams`, `/dupes`, `/reindex`, `/doctor`,
      `/settings`. (`/ask` awaits a local model.)
- [x] **`@` scope mentions** — resolves a typed prefix against indexed
      directories and inserts `in:"<path>"`; arrow keys + Enter.
- [x] **New home and result states** — pinned files, recent files, saved
      searches, suggested queries, grouped version rows. (Recent queries and
      best-guess/related cards remain.)
- [x] **Duplicate cleanup UI** — `/dupes` opens a review view with exact-content
      and near-image groups; open/copy/reveal per file. Destructive delete is
      intentionally left to the OS.
- [x] **Guided first-run setup flow** — welcome → folders → exclusions →
      preferences → optional packs → first scan → Done, with resume after an
      interrupted scan.
- [x] **Setup-owned downloads and configuration** — folders, exclusions, theme,
      and shortcut are asked in setup; OCR/model downloads are offered there.
      Settings shows status only and links to **Run setup again**. Visual-search
      models live only in Settings → Beta and are not offered in setup.
- [x] **First-scan completion gate** — `first_scan_done` gates `is_setup_complete`;
      the scan shows live counts and a Done summary, resumes after interruption,
      and later launches stay search-first. Progress bar and ETA are shown for
      metadata indexing and extraction; the current file path is not.
- [—] **Voice and microcopy pass** — intentionally skipped; not planned.

## B. Intelligence & vision

- [x] **Embedding provider contract** — `semantic_model` exists in config and is
      wired to real provider selection (`static`, `smart`, `none`). Static table
      remains default; missing/incompatible providers fall back cleanly with
      recorded reasons; model-id isolation prevents mixing incompatible vectors.
      (ROADMAP K16)
- [~] **ONNX "smart pack"** — optional `ort`/ONNX model (`bge-small-en-v1.5`
      int8) behind `smart-embeddings`, with K9 download hygiene, pinned SHA-256,
      bounded concurrency, and static fallback. Code path compiles; real-model
      recall and memory are not measured yet. (ROADMAP K17)
- [~] **Image embeddings & visual search** — moved to an opt-in Beta settings
      surface. The Beta tab can enable the experiment and download the pinned
      CLIP ViT-B/32 vision + text pack (~150 MB) with checksum/resume. The pack is
      **not wired into search**; K18/K19 remain off by default and the mock
      encoder is test-only. (ROADMAP K18, K19; beta model-download only.)
- [ ] **Similar images** — command/daemon/MCP plumbing exists but returns
      "not available" until the beta visual tier is wired. Hidden from MCP tools
      when unavailable. (ROADMAP K20)
- [ ] **Native OCR** — not implemented. An automated pass added an `osascript`
      shim and a Windows stub; both were removed. Settings shows a disabled
      "Native — not available yet" option so the UI does not pretend it works.
      Real macOS Vision and Windows Media OCR require platform bindings and
      platform CI. Scanned-PDF embedded image-stream OCR is partial.
      (ROADMAP K21; blocked.)
- [-] **"This is me" identity** — skipped for now; revisit after the vision
      tier and sensitive-data controls. Local face embeddings + clustering,
      opt-in enrollment with 3–5 photos, explainable matches, delete-all.
      Sensitive data: local only, never uploaded.
- [-] **VLM captions (optional)** — skipped for now. Idle-time captions for
      diagrams and screenshots, stored as searchable text. Complements CLIP
      where short text queries are weak.
- [x] **CJK tokenization + multilingual golden coverage** — K15 CJK Tantivy
      tokenization shipped with unigrams/bigrams and schema v2. K22 adds a
      43-query non-visual golden set with CJK and accented-Latin cases.
      Multilingual **embeddings** remain open: K17 adds an English `bge-small`
      pack, not a multilingual model. (ROADMAP K15 [x], K17 [x], K22 [x])

## C. Agent, LLM & runtime

- [x] **Warm daemon** — one process owns the index; GUI, CLI, and MCP attach.
      The daemon owns the writer and single-instance lock; clients use a Unix
      socket or Windows named pipe and fall back read-only when it is absent.
      See [ROADMAP K1](ROADMAP.md#k1-daemon-split-epic). (ROADMAP K1; release
      packaging of the daemon sidecar remains in section F.)
- [ ] **Local HTTP/SSE endpoint** — loopback only, token auth, read-only.
      Needed for web-capable MCP clients and long-lived agents. Follows K1.
      (ROADMAP K1)
- [ ] **One-click agent connect** — generated configs and live status for
      Claude Desktop/Code, Cursor, and other MCP clients. Keep a manual JSON
      fallback in Settings → Agents. (ROADMAP K1, K8)
- [ ] **Local LLM query compiler** — optional Ollama/LM Studio. Natural
      language → strict JSON AST, validated, cached, shown as chips. The LLM
      never retrieves on its own.
- [ ] **`/ask` with citations** — local model answers from retrieval results,
      with path and line citations. No retrieval, no answer. No cloud by
      default; per-use consent if a cloud provider is configured.
- [ ] **Agent surface v2** — `find_symbol` (tree-sitter), `.gitignore` /
      `.cursorignore` parity, pagination beyond the 10k exhaustive cap,
      optional read-only scoping per root, daemon-backed MCP. Add `reindex`,
      `health_check`, pagination cursors, and per-tool token budgets; see
      [ROADMAP K8](ROADMAP.md#k8-mcp-v2). (ROADMAP K8, K1, K10)
- [ ] **Image/agent tools** — expose visual search and similar-image lookup
      once the visual tier exists; keep snippets token-budgeted. (ROADMAP K6, K3)

## D. Search quality & performance

- [ ] **E2 extension interning** — `FileEntry.extension: Option<String>` is
      still a per-entry allocation. A `u16` intern table can save ~6–12 MB per
      1M files. This is a follow-up to K2, not part of the shipped arena.
- [~] **B4 full pushdown** — scope paths are now pushed into the Tantivy query
      as a path-prefix clause. `ext` / `size` fast fields still need schema work
      and a rebuild before those filters can be pushed too. (ROADMAP K12)
- [ ] **1M-file prefilter** — K12 measured the content-tier alternatives at
      1M synthetic documents. FTS5 trigram used 918 MB and was ~900× slower on
      broad BM25, so keep Tantivy + folded; a separate bounded prefilter is
      still the next option if filename search exceeds the <15 ms budget. See
      [the decision memo](decisions/index-format.md). (ROADMAP K12)
- [x] **Metadata arena** — shipped: compact open-addressed path index, lazy id
      map, and names/attrs arena. Measured 91.3 MB RSS at 1M synthetic entries
      (target ≤ 100 MB). [ROADMAP K2](ROADMAP.md#k2-compact-metadata-arena).
- [x] **Embedding table quantization** — the static table is ~30 MB f32 today;
      i8 with scales targets ~8 MB with a golden-set quality check. Task:
      [ROADMAP K14](ROADMAP.md#k14-quantize-the-static-embedding-table-to-i8).
      (ROADMAP K14; real Potion run: 7.56 MB i8, MRR 1.0 vs f32 1.0.)
- [x] **Thumbnail-based image hashing + memory release** — generate the 480 px
      thumbnail once, hash it (~2.4 ms), reuse it for the grid, and return freed
      pages after each image/PDF (`malloc_trim`, `malloc_zone_pressure_relief`,
      `EmptyWorkingSet`). Task: [ROADMAP K3](ROADMAP.md#k3-thumbnail-based-hashing--memory-release).
      (ROADMAP K3)
- [x] **Lazy snippets and line highlighting** — return result metadata only;
      fetch snippets for visible rows. This is the difference between 60–90 ms
      and 1–10 ms on broad queries. Task: [ROADMAP K4](ROADMAP.md#k4-lazy-snippets-and-line-highlighting).
      (ROADMAP K4)
- [x] **Folded / trigram content tier** — substring and OCR-error tolerance
      (`nutsmp` → `donutsmp.net`, `0RDER` → `ORDER`), ranked after exact matches
      with raw-text verification. [ROADMAP K5](ROADMAP.md#k5-folded--trigram-content-tier).
- [x] **Optional PP-OCRv6 OCR pack** — PaddleOCR PP-OCRv6 tiny as a slower,
      in-process alternative for small UI text. Tesseract stays the fast, smaller
      default. Native macOS/Windows OCR remains in section B.
      [ROADMAP K6](ROADMAP.md#k6-optional-pp-ocrv6-ocr-pack). Our 104-image
      fixture scores 104/104 for both engines, so the pack is a choice, not a
      proven accuracy upgrade.
- [x] **Idle/battery scheduling + watcher robustness** — `SCHED_IDLE`/ioprio,
      `PRIO_DARWIN_BG`, battery pacing, pause, 400 ms settle, still-writing
      retry, newest-first fresh files. [ROADMAP K7](ROADMAP.md#k7-scheduling-and-watcher-robustness).
- [x] **Search-result grouping** — collapse near-identical screenshots/versions
      at write time (text overlap + thumbnail dHash), like screenshot bursts.
      [ROADMAP K11](ROADMAP.md#k11-search-result-grouping).
- [ ] **Tantivy writer pipeline** — extraction is parallel; `add_document` and
      commits are still single-threaded. Benchmark before changing; see K12.
      (ROADMAP K12)
- [x] **Golden query set in CI** — a frozen 43-query non-visual set with
      recall@5, MRR, NDCG, and latency metrics, including intent, CJK, and
      accented-Latin queries. Debug: Recall@5 1.000, MRR 0.988, p50 5.9 ms.
      [ROADMAP K22](ROADMAP.md#p3--intelligence--vision). Visual cases are
      deferred until the beta visual tier is wired.
- [ ] **Facet latency tests** — counts and toggles must hold their budgets at
      100k and 1M files. (ROADMAP K4)

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
      (ROADMAP K9)
- [ ] **Face-data controls** — explicit delete-all, storage location, and an
      audit trail before the feature ships.
- [ ] **Cloud-consent UX** — a visible indicator whenever a query or snippet
      leaves the device; local-only remains the default.

## F. Packaging & launch

Release blockers:

- [ ] **Version bump** — update `package.json`, `src-tauri/Cargo.toml`,
      `src-tauri/tauri.conf.json`, CLI version, and `CHANGELOG.md` together.
- [ ] **Release build** — `make build` on macOS, Windows, and Linux; verify the
      PP-OCRv6 engine is compiled in and the Beta visual pack is hidden from
      setup.
- [ ] **Signing** — macOS codesign + notarization, Windows Authenticode.
      Needed before shipping the ONNX runtime and for a non-scary installer.
- [ ] **Auto-updater** — Tauri updater plugin + signing keys + release channel.
- [ ] **Icons / tray artwork** — replace remaining placeholder Tauri icons.
- [ ] **Release workflow assets** — CLI binaries currently upload as workflow
      artifacts; move them to the draft release with `needs:` + `gh release
      upload`.
- [ ] **Smoke test the site download links** — installer names, checksums, and
      the version shown on the site must match the draft release.

Post-release / nice to have:

- [ ] **Demo GIF + benchmark post** — the numbers and methodology exist in
      `docs/BENCHMARKS.md`; the public artifacts do not. (ROADMAP K12)
- [~] **Visual search pack packaging** — Settings → Beta downloads the pinned
      CLIP pack with checksum/resume/disk precheck. One-click uninstall and
      offline retry UI are still missing; search integration is deferred.
- [ ] **Native platform OCR** — blocked on macOS Vision / Windows Media OCR
      bindings and platform CI. See section B and
      [NATIVE_OCR_VERIFICATION.md](NATIVE_OCR_VERIFICATION.md).

---

## Milestone mapping

| Milestone (VISION) | Primary backlog here |
|---|---|
| 1 — It understands me | A (all), E (sensitive paths), D (golden set) |
| 2 — It filters like lightning | A (facets, saved searches), D (pushdown, 1M, arena) |
| 3 — It sees | B (all), E (face controls), F (Vision Pack packaging) |
| 4 — It talks to your AI | C (all), E (cloud consent) |
| 5 — Ship it properly | F (all) |
| Cross-cutting — light & fast | C (daemon), D (arena, hashing, snippets, folded tier, OCR pack, scheduling) — see [ROADMAP](ROADMAP.md) |

---

## Sequencing notes and prior deferrals

| Item | Why |
|---|---|
| Metadata arena (K2) first | It shrinks the payload the daemon will own; doing it first avoids moving the fat `Vec<FileEntry>` into a new process. |
| Daemon split (K1) before HTTP, OCR packs, native UI | Index ownership must be outside the webview before more workers/models land; CLI/MCP also stop reloading metadata. |
| Thumbnail hashing (K3) before result grouping (K11) | Grouping needs a cheap hash; hashing a thumbnail is ~2.4 ms and avoids full decodes. |
| Folded tier (K5) after lazy snippets (K4) | The folded tier increases candidate count; the query path must already fetch only visible highlights. |
| PP-OCRv6/native OCR (K6) as opt-in packs | Tesseract stays the fast, low-memory default; PP-OCRv6 is slower and not proven more accurate on our fixture. Native OCR remains blocked. |
| Native quick-search window (K13) as a spike | Only after the daemon lands; keep Tauri for settings if a native summoner does not clearly win on RSS/latency. |
| Extension interning (E2) | High churn across 7 files for 6–12 MB at 1M files. |
| Tantivy BooleanQuery pushdown (B4) | Needs a schema change and a full index rebuild; now required for Milestone 2. |
| ANN index (USearch) | Exact i8 scan is enough below ~400k units; revisit at 1M. |
| Cross-encoder reranker | 30–60 ms for 20 pairs does not fit the latency budget. |
| Bundled multi-GB LLM | Product decision: no giant dependency. Optional local Ollama/LM Studio provides intent compilation and `/ask`. |
| Cloud sync / accounts | Violates the local-first thesis; no cross-device state planned. |
| Chat-first UI | Chat is slower than search for the core job; answers are a mode (`/ask`), not the surface. |
| Bundled ONNX runtime by default | The static table keeps the base binary small; neural and vision tiers are opt-in packs. |
| Video/audio transcription | Different pipeline and storage budget; not required for the memory thesis yet. |
