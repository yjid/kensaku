# Kensaku — Intelligence & Vision execution plan

> Companion to [`ROADMAP.md`](ROADMAP.md) and [`REMAINING.md`](REMAINING.md).
> This plan expands backlog section B into ordered, independently reviewable
> tasks. Tasks are numbered K15+ so they can be committed one at a time using the
> same definition of done as K1–K14. Identity ("this is me") and VLM captions
> are intentionally left out of this slice per the backlog.

## Baseline check

These tasks are improvements over the current tree, not renames of shipped work.
The baseline was verified against the code before this plan was written:

| Task | Current state | Gap the task closes |
|---|---|---|
| K15 | `filename`/`content` use Tantivy's default `SimpleTokenizer`; a no-space CJK run becomes one token. | CJK queries cannot match words inside a longer run. |
| K16 | `semantic_model = "static"` exists in config, but the provider is hard-wired to the static table. | No provider abstraction or fallback contract for a second model. |
| K17 | Only the Potion static table exists; there is no `ort`/ONNX inference path. | No higher-quality semantic tier. |
| K18 | Images feed OCR text and thumbnails only; no visual vectors are produced. | Text cannot describe image content. |
| K19 | Search fuses fuzzy, BM25, semantic, and folded tiers; there is no visual tier. | Text-to-image retrieval is impossible. |
| K20 | No image-to-image query path exists. | Users cannot ask "more like this image". |
| K21 | Tesseract is the default; PP-OCRv6 is optional. No macOS Vision or Windows OCR implementation exists. | Tesseract install friction remains. |
| K22 | No frozen general golden query set exists in the repo; K14 has a small synthetic/labelled semantic check only. | No regression gate for intent, CJK, or visual quality. |

If any of these assumptions changes before a task starts, re-check the code and
update this table rather than implementing duplicate behavior.

## Scope

In scope:

- CJK-aware lexical search.
- Multilingual embedding readiness.
- Optional ONNX smart embedding pack.
- Visual image embeddings and text-to-image search.
- Similar-image lookup.
- Native platform OCR, including scanned PDFs.

Out of scope for this slice:

- Face identity and VLM captions (deferred in `REMAINING.md` section B).
- Cloud inference or any default network call.
- Replacing Tesseract as the default OCR.
- Changing the MIT-compatible licensing posture.

## Sequencing principles

1. **Lexical correctness before models.** CJK tokenization is a pure Rust change
   with no new dependency and no download. It should land first.
2. **Model plumbing before visual search.** The ONNX smart pack and download
   hygiene must exist before image embeddings depend on them.
3. **Visual index before UI.** Search APIs and tests should land before result
   chips and result-row affordances.
4. **Platform OCR is independent.** It can run in parallel with embedding work,
   but its tests are platform-specific and must not gate Linux CI.
5. **One schema migration at a time.** CJK tokenizer and visual index changes
   must not share a migration commit.

## Tasks

### K15. CJK-aware Tantivy tokenization

**Why:** Tantivy's default simple tokenizer does not segment no-space CJK text,
so Chinese/Japanese/Korean queries and filenames are effectively unsearchable.

**Work**

- Register a CJK-aware tokenizer for `filename` and `content`.
- Use character-class segmentation plus 2-gram/3-gram fallback for CJK runs;
  keep the existing simple tokenizer behavior for Latin, code, and paths.
- Handle mixed scripts (`README最終.md`, `src/组件/index.ts`).
- Preserve path separators, camelCase, and `_`/`.` splitting for code search.
- Add tokenizer tests for exact, prefix, and substring cases.
- Add a schema/version marker so old indexes rebuild deterministically.

**Acceptance**

- CJK filenames and content are searchable by full token, prefix, and substring.
- Existing Latin/code/path tests still pass.
- Index rebuild is triggered once and recorded in Doctor/stats.
- No change to query AST or ranking.

**Files:** `indexer/content.rs`, tokenizer helpers, schema migration tests.

**Effort:** 1 week.

### K16. Embedding provider contract

**Why:** The static Potion table is English-centric, and `semantic_model` is a
config string that does not actually select an implementation. Before adding a
multilingual model, the pipeline needs a provider boundary and a fallback
contract.

**Work**

- Wire the existing `semantic_model` config value to a real provider enum
  (`static`, `smart`, `none`) instead of leaving it display-only.
- Define an embedding-provider trait or enum that owns model id, dimension,
  normalization, and text encoding.
- Keep the static table as the default and the `none` path as a lexical-only
  mode.
- Ensure a missing or invalid optional provider falls back to the static table
  and records why.
- Add tests for selection, persistence, and fallback.

**Acceptance**

- `semantic_model` changes actual behavior and is persisted.
- Missing optional models do not break indexing or search.
- Status surfaces provider, model id, and fallback reason in Doctor/stats.
- No new model download and no new dependency in this task.

**Files:** `embed/mod.rs`, `embed/model.rs`, `embed/table.rs`, `config.rs`, `models.rs`.

**Effort:** 3-5 days.

### K17. Optional ONNX smart embedding pack

**Why:** The static table is fast but small. A downloaded ONNX model can improve
semantic recall while remaining optional and local.

**Work**

- Add an optional `smart-embeddings` feature using `ort` with `load-dynamic`.
- Download a pinned multilingual model (bge-small or EmbeddingGemma-Q4) with
  SHA-256, resume, disk precheck, and mirror support.
- Run inference in the existing embedding queue with bounded concurrency.
- Add an i8 quantization path and golden-set quality comparison.
- Keep the static table as the default and fallback.

**Acceptance**

- Feature off: no ONNX dependency, no memory cost, current behavior.
- Feature on: model downloads once, verifies checksum, and indexes incrementally.
- Semantic MRR improves on the golden set or the pack is documented as not
  worth shipping.
- Peak RSS and per-worker memory recorded.

**Files:** `Cargo.toml`, `models.rs`, `embed/queue.rs`, setup/Settings UI.

**Effort:** 2 weeks.

### K18. Image embedding extraction pipeline

**Why:** Images are currently searchable only by OCR and filename. Visual
embeddings require a decode/resize/inference path that reuses thumbnails.

**Work**

- Decode images once, reuse the K3 thumbnail for embedding input.
- Add an ONNX vision encoder (SigLIP or CLIP ViT-B/32 int8).
- Store embedding vectors in the existing vector sidecar or a new visual table.
- Add megapixel/size caps and cancellation handling.
- Reuse the thumbnail hash to skip unchanged images.

**Acceptance**

- Images are embedded without a second full decode.
- Cancel leaves no partial vectors or orphaned files.
- Memory and time per image are recorded.

**Files:** `indexer/extract/image.rs`, `embed/queue.rs`, storage layer.

**Effort:** 2 weeks.

### K19. Visual search tier and RRF integration

**Why:** Text-to-image search needs a fourth retrieval tier fused with lexical,
folded, and semantic text results.

**Work**

- Add a visual query encoder for text queries.
- Search the visual vector index and fuse results with RRF.
- Add `visual:` interpretation and chips.
- Preserve token budgets and pagination.
- Add filters for image type, size, and date.

**Acceptance**

- Queries such as "red cap" or "system design diagram" return relevant images.
- Visual tier can miss its deadline without breaking the response.
- Golden-set recall and latency recorded.

**Files:** query planner, `embed/`, `commands.rs`, MCP/CLI.

**Effort:** 2 weeks.

### K20. Similar-image lookup

**Why:** Once a visual index exists, image-to-image search is a natural and
cheap extension.

**Work**

- Add an image-to-image query path from a selected result.
- Reuse the visual vector and related-file response shape.
- Add UI affordance and agent tool exposure.
- Add tests for identical, near-duplicate, and unrelated images.

**Acceptance**

- Similar images return in under the existing visual-search budget.
- Results include similarity scores and paths.
- Agent access remains read-only.

**Files:** `commands.rs`, frontend result actions, MCP tools.

**Effort:** 1 week.

### K21. Native platform OCR

**Why:** Tesseract is an external dependency. Native OCR removes install
friction and improves platform integration.

**Work**

- Add macOS Vision via `objc2-vision` behind a feature.
- Add Windows OCR via the `windows` crate behind a feature.
- Keep Tesseract as the default and fallback.
- Route scanned PDFs through the same OCR path.
- Add platform-specific tests or manual verification notes.

**Acceptance**

- On supported platforms, OCR works without Tesseract.
- Missing native OCR falls back to Tesseract.
- Memory and accuracy are recorded against the frozen fixture.

**Files:** `ocr.rs`, `indexer/extract/image.rs`, `indexer/extract/pdf.rs`, setup UI.

**Effort:** 1-2 weeks.

### K22. Extend the golden query set with CJK/multilingual cases

**Why:** New tokenization and embedding behavior needs regression coverage
before it can be considered shipped. This is an extension of the existing
"golden query set in CI" backlog item in `REMAINING.md` section D, not a second
competing set.

**Work**

- Start from the frozen 40-50 query set defined by the section D item.
- Add CJK and accented-query cases with expected paths.
- Measure recall@5, MRR, NDCG, and latency for the extended set.
- Add CI thresholds where deterministic.
- Record results in `BENCHMARKS.md`.

**Acceptance**

- One golden set covers intent, lexical, semantic, CJK, and visual queries.
- It runs in CI or documented manual verification.
- Regressions fail the build where feasible.

**Files:** tests, `docs/BENCHMARKS.md`.

**Effort:** 3-5 days, after the section D set exists.

## Dependency order

| Order | Task | Depends on |
|---|---|---|
| 1 | K15 CJK tokenization | — |
| 2 | K16 embedding provider trait | K15 recommended |
| 3 | K17 ONNX smart pack | K16 |
| 4 | K18 image embedding pipeline | K17 |
| 5 | K19 visual search tier | K18 |
| 6 | K20 similar images | K19 |
| 7 | K21 native OCR | — |
| 8 | K22 multilingual golden queries | K15, K16, K17, section D golden set |

K21 may run in parallel with K16-K20. K22 should land after the behavior it
measures.

## Risks and constraints

- **Licensing:** ONNX Runtime and model licenses must remain MIT-compatible or
  be documented as optional packs with their own terms.
- **Memory:** Visual search must not break the <500 MB at 1M entries target.
- **Disk:** Model downloads need resume, checksums, and disk prechecks.
- **Platform:** Native OCR cannot be fully tested on Linux; use manual
  verification notes and feature gates.
- **Schema churn:** CJK and visual index changes need one migration each.
- **No default network:** All model downloads remain opt-in and user-initiated.

## Recommended first commit

Start with K15. It is the smallest correctness improvement, has no new
dependency, and makes the rest of the plan testable for CJK users. The first
commit should add the tokenizer and tests only; schema migration and rebuild
reporting should follow in a second commit if needed.
