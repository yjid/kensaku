# Kensaku — Intelligence & Vision execution prompt

You are a senior Rust/Tauri engineer working in `/home/gochi/Projects/Kensaku`.
Implement the complete Intelligence & Vision slice end to end: tasks K15–K22 in
`docs/INTELLIGENCE_VISION_PLAN.md`. Do not stop at a plan. Write the code, tests,
docs, and local commits.

## 1. Mission

Finish these tasks, in order, one commit per task:

1. K15 — CJK-aware Tantivy tokenization.
2. K16 — embedding provider contract and model-id isolation.
3. K17 — optional ONNX smart embedding pack.
4. K18 — image embedding extraction pipeline.
5. K19 — visual search tier fused into RRF.
6. K20 — similar-image lookup.
7. K21 — native macOS/Windows OCR.
8. K22 — extend the golden query set with CJK/multilingual cases.

K21 may be implemented in parallel because it is platform-specific. K22 must
wait for K15–K17 and the existing section D golden-query-set item.

Read these first, in this order:

1. `docs/INTELLIGENCE_VISION_PLAN.md` — the task definitions and acceptance.
2. `docs/ROADMAP.md` — status table and task numbering.
3. `docs/REMAINING.md` — backlog section B and D.
4. `docs/ARCHITECTURE.md` — daemon, index tiers, budgets.
5. `docs/VISION.md` — product behavior and milestone 3.
6. `docs/AGENTS.md` — read-only agent contract.
7. `docs/BENCHMARKS.md` — measurement format.
8. `CHANGELOG.md` — unreleased section.

## 2. Repository facts

- Rust 2024, Tauri 2, React 19 + Vite, Tantivy 0.26, SQLite via rusqlite.
- Two Rust crates: `src-tauri/` and `cli/`. There is no root Cargo workspace.
- Daemon: `cli/src/bin/kensaku-daemon.rs`; client: `src-tauri/src/daemon.rs`.
- The daemon owns the writer. The GUI, CLI, and MCP are clients. Keep that.
- `IndexerManager` lives in `src-tauri/src/indexer/mod.rs`.
- Content index: `src-tauri/src/indexer/content.rs`.
- Metadata: `src-tauri/src/indexer/metadata_db.rs`.
- Embeddings: `src-tauri/src/embed/`.
- Search fusion: `src-tauri/src/search/mod.rs`.
- Commands: `src-tauri/src/commands.rs`.
- OCR: `src-tauri/src/ocr.rs`, image extraction:
  `src-tauri/src/indexer/extract/image.rs`, PDF:
  `src-tauri/src/indexer/extract/pdf.rs`.
- Model/download management: `src-tauri/src/models.rs`.
- Current static embedding table: `src-tauri/src/embed/table.rs`.
- Frontend: `src/App.tsx`, `src/components/`, `src/types.ts`.

Current expected history includes:

```text
6dcdd93 docs: plan the intelligence and vision slice
d62d676 docs: add intelligence and vision execution prompt
```

The branch is `k1-k14-completion`. An open PR may exist; do not push, merge, or
release unless the user explicitly asks. Create a new local branch for this work:

```sh
git switch -c intelligence-vision
```

If there are uncommitted user changes such as `src/components/SetupModal.tsx` or
`public/setup-icon-*.png`, preserve them. Never `git add -A` blindly. Stage only
files you changed for the current task.

## 3. Non-negotiable constraints

- Do not reset or delete real user data, configs, indexes, or model directories.
  All tests must use temp directories.
- No new network call by default at runtime. Model downloads remain explicit,
  user-initiated, opt-in, and checksum-verified.
- Do not copy GPL/AGPL code. Ideas only. Keep Kensaku MIT-compatible. Check the
  license of every model, ONNX runtime, and crate you add and record it.
- Do not mention or attribute the anonymized screenshot-search reference in
  docs, code comments, or commits.
- The agent surface stays read-only by default. No shell/network tools.
- One task per commit. Do not combine unrelated cleanup.
- Every task updates tests, docs, and `CHANGELOG.md`.
- Never invent benchmark numbers. If a measurement cannot be run, write
  `not measured` and explain why.
- If a platform-specific task cannot be tested on Linux, feature-gate it and
  document the manual verification steps. Do not fake a test result.
- Prefer small, reviewable changes over speculative architecture.
- Keep the daemon as the owner of index state. Do not create a second writer.

## 4. Execution protocol for every task

1. Inspect the current code and tests. Do not assume the plan is still accurate.
2. Write or update tests first where practical.
3. Implement the smallest complete version.
4. Run formatting, tests, clippy, and TypeScript checks.
5. Update docs and changelog.
6. Commit only that task:

```text
task K15: add CJK-aware content tokenization
task K16: wire the embedding provider contract
task K17: add optional ONNX smart embedding pack
task K18: add image embedding extraction
task K19: fuse visual search into RRF
task K20: add similar-image lookup
task K21: add native platform OCR
task K22: extend golden queries with CJK cases
docs: sync intelligence and vision status
```

7. Do not leave the tree dirty between tasks except for known user changes.
8. After each task, report what passed and what remains.

## 5. K15 — CJK-aware Tantivy tokenization

### Why this is an improvement

`filename` and `content` currently use Tantivy's default `SimpleTokenizer`.
`SimpleTokenizer` treats an unbroken CJK run as one token. A file named
`最終報告書.pdf` cannot be found by `報告`; content `東京都の天気` cannot be found
by `東京`. This is a correctness gap, not a tuning issue.

### Implementation

- Add a custom Tantivy tokenizer, preferably in a new
  `src-tauri/src/indexer/tokenizer.rs` module.
- Implement Tantivy's `Tokenizer` + `TokenStream` traits. Preserve byte
  offsets and increment token positions correctly.
- For non-CJK text, match the existing `SimpleTokenizer` behavior: split on
  non-alphanumeric characters. Do not change Latin/code/path behavior.
- For CJK runs, emit:
  - each character as a unigram, so single-character queries work;
  - each adjacent character pair as a bigram, so normal two-character words
    match.
  Emit both with correct offsets and sequential positions.
- Treat these ranges as CJK: Han (`\u{3400}-\u{4DBF}`,
  `\u{4E00}-\u{9FFF}`, `\u{F900}-\u{FAFF}`), Hiragana/Katakana
  (`\u{3040}-\u{30FF}`), Hangul (`\u{AC00}-\u{D7AF}`,
  `\u{1100}-\u{11FF}`), and CJK compatibility/supplementary ranges where the
  `char` is recognized. Handle mixed scripts such as `README最終.md` and
  `src/组件/index.ts` by flushing the current run at script boundaries.
- Register the tokenizer under a stable name such as `"kensaku"` in both
  `ContentIndex::new` and `ContentIndex::open_read_only`.
- Change the `filename` and `content` fields to use that tokenizer via
  `TextFieldIndexing`. Keep `path` as `STRING`. Keep `content_folded` as the
  existing trigram tokenizer.
- Add a schema marker. Simplest reliable approach: write a small
  `content.schema` file in the content index directory containing a version
  number. In `ContentIndex::new`, if the marker is missing or old, rebuild the
  index the same way the K5 migration does. Do not silently open an index built
  with a different tokenizer.
- `open_read_only` must still register the tokenizer so queries analyze
  correctly. It must not write or migrate.
- Add tests that build a temp index and verify:
  - CJK filename exact word and substring queries;
  - CJK content exact word and substring queries;
  - mixed-script filename;
  - single-character CJK query;
  - Latin/code/path regression tests still pass;
  - an old index without the marker triggers exactly one rebuild.
- Record index-size impact on a small CJK fixture in the test output or
  benchmark notes; do not invent a number.

### Acceptance

- CJK filenames and content are searchable by full word and substring.
- Existing Latin/code/path tests pass.
- Old indexes rebuild once and the reason is surfaced in stats/Doctor.
- No query AST or ranking changes.
- Clippy clean.

## 6. K16 — Embedding provider contract

### Why this is an improvement

`semantic_model` is currently a config string used for display. It does not
select an implementation. Before adding another model, make provider selection
real and prevent vectors from different models or dimensions from mixing.

### Implementation

- Introduce a small provider abstraction in `embed/model.rs` or a new
  `embed/provider.rs`:
  - `Static` — existing Potion table.
  - `None` — lexical-only; no semantic tier.
  - Keep a `Smart` variant reserved for K17 behind a feature if that reduces
    churn.
- `ModelRegistry::global()` must read `KensakuConfig.semantic_model`,
  `semantic_enabled`, and `semantic_f32_table`, and choose the provider.
- Unknown or unavailable values fall back to `Static` with a recorded reason.
- `semantic_enabled = false` means `None`; search must skip the semantic scan.
- Add model-id filtering to semantic search. `Unit` already has `model_id`.
  `ExactScanner::scan` currently scores every unit. Change it to accept the
  active model id, or filter units before scoring, so vectors from an old model
  can never be compared to a new query vector.
- If the active model id or dimension changes, the semantic store must be
  rebuilt or filtered. Do not mix dimensions.
- Surface provider name, model id, dimension, and fallback reason in
  Doctor/stats.
- Add tests:
  - config round-trip for `static`, `none`, unknown fallback;
  - `semantic_enabled=false` skips semantic;
  - model-id isolation: old-model units are ignored;
  - missing artifacts fall back to the hashing estimator or static table as
    documented.
- No new dependency, no download in this task.

### Acceptance

- `semantic_model` changes actual behavior and persists.
- Missing optional models do not break indexing or search.
- Different model ids never mix vectors.
- Status surfaces provider and fallback reason.
- Existing 177+ tests still pass.

## 7. K17 — Optional ONNX smart embedding pack

### Why this is an improvement

The static table is small and English-centric. A downloaded ONNX model is an
optional quality tier. It must remain optional and local.

### Implementation

- Add a Cargo feature, e.g. `smart-embeddings`, in `src-tauri/Cargo.toml`.
  Feature off must add no dependency and no memory cost.
- Add an ONNX text embedding provider. Use `ort` directly or a small wrapper
  crate. If you use `fastembed`, verify it supports `load-dynamic` and does not
  download models implicitly. Do not guess APIs; read the crate docs in the
  local registry and compile.
- Choose a model with a clear MIT/Apache-2.0-compatible license. If the license
  is unclear, stop and report rather than shipping it. Record the license in
  `docs/ARCHITECTURE.md` or a new model-license note.
- Store the smart pack under a distinct directory such as
  `models/smart/`, leaving the existing static artifacts in `models/`.
- Reuse the download hygiene in `models.rs`: `.part` files, HTTP Range resume,
  disk-space precheck, SHA-256 verification, mirrors, and re-verification of
  existing files. Add the new artifacts to a manifest. Compute the real sizes
  and SHA-256 values from the actual downloaded files.
- Pin the tokenizer and model files. Do not download at query time.
- Run inference in the existing embedding queue with bounded concurrency
  (max 2 workers). Keep memory bounded; one session, lazy load, clear failure
  state.
- Prefer an already-quantized int8 ONNX artifact. If only f32 exists, quantize
  output vectors with per-vector scales. Validate that query and document
  vectors use the same scale convention.
- Add setup/Settings UI only if the existing OCR pack UI makes it cheap. Reuse
  patterns from `src/components/setup/ocr.tsx` and `SettingsModal.tsx`. If the
  UI is too large, add backend status and a minimal "Smart pack" row; do not
  block the backend on a redesign.
- Tests:
  - feature off: build and tests still pass without the dependency;
  - manifest, checksum, resume, and disk-precheck unit tests;
  - provider selection and fallback;
  - preprocessing/tokenization unit tests;
  - ignored real-model test gated by `KENSAKU_SMART_MODEL_DIR`.
- Record real quality and latency only if you can run the real model. Otherwise
  write `not measured`.

### Acceptance

- Feature off preserves current behavior exactly.
- Feature on downloads and verifies once, then indexes incrementally.
- Missing/corrupt model falls back safely.
- Model-id isolation still holds.
- No default network call.

## 8. K18 — Image embedding extraction pipeline

### Why this is an improvement

Images currently contribute OCR text, filenames, thumbnails, and perceptual
hashes only. There is no vector that describes the visual content.

### Implementation

- Define a visual-embedding abstraction so tests can use a fake encoder. The real
  encoder can be ONNX CLIP ViT-B/32 or SigLIP. Use one model for both image and
  text encoders so their vectors share a space.
- Reuse the K3 thumbnail path. Do not perform a second full decode when a
  thumbnail exists. If the thumbnail is missing, decode once, write the
  thumbnail, and use it.
- Preprocess to the model's expected input (for CLIP ViT-B/32: RGB, 224×224,
  CLIP mean/std, NCHW). Verify against the model metadata rather than guessing.
- Produce a normalized 512-dim (CLIP) or model-specific vector. Store it as i8
  with a per-vector scale. Add a model id and dimension.
- Add a separate `VisualStore` rather than mixing 512-dim visual vectors into
  the 256-dim text `UnitStore`. Persist it beside the semantic store and load it
  with bounds checks.
- Store one vector per image node. Use the thumbnail hash or node hash to skip
  unchanged images.
- Add cancellation checks around decode and inference. Never leave partial
  vectors or orphaned temp files.
- Add a `visual_enabled`/status field. Missing visual model means the visual tier
  is simply absent; indexing must continue.
- Tests:
  - fake encoder stores one vector per image;
  - identical thumbnail hash skips re-embedding;
  - cancellation leaves store unchanged;
  - corrupt image is skipped without panic;
  - temp directory cleanup;
  - ignored real-model test gated by env vars.

### Acceptance

- No second full decode when a thumbnail exists.
- Memory and per-image time recorded, or `not measured`.
- Missing model does not break indexing.
- No partial state on cancel.

## 9. K19 — Visual search tier and RRF integration

### Why this is an improvement

Search currently fuses fuzzy, BM25, semantic-text, and folded tiers. There is no
visual tier, so “red cap” cannot retrieve an image whose OCR text is empty.

### Implementation

- Add a text query encoder for the same visual model. Use the matching CLIP/SigLIP
  text encoder and tokenizer. Normalize and quantize exactly like image vectors.
- Add a visual scanner over `VisualStore`, with a minimum cosine floor.
- Restrict visual candidates to image nodes. Do not return a text file because
  its path happened to be near an image vector.
- Add visual candidates to search fusion:
  - add `in_visual` to the internal candidate struct;
  - choose a conservative RRF weight, e.g. `0.8` of the semantic weight;
  - require the visual floor for visual-only candidates;
  - merge into existing candidates when the same path is already present;
  - add a `MatchType::Visual` or equivalent badge when only visual matched.
- Decide when to run the visual tier. At minimum run it when the query has image
  intent or a `type:image`/image extension filter. Optionally run it for all
  non-empty natural-language queries if latency stays within budget. Do not
  regress keystroke latency.
- Add `visual:` interpretation only if it fits the existing interpretation
  model; otherwise keep the backend tier invisible and use the existing image
  filter chip.
- Preserve token budgets, pagination, scope, and `as_of`.
- Add tests with synthetic vectors:
  - visual-only image is returned;
  - text file with a nearby vector is filtered out;
  - visual + BM25 merge into one candidate;
  - floor rejects weak matches;
  - disabled visual tier changes nothing.
- Add an ignored real-model test for “red cap”/“diagram” style queries.
- Record recall and latency on a small frozen set, or `not measured`.

### Acceptance

- Text-to-image queries return relevant images.
- Visual tier failure does not break the response.
- No text-file false positives.
- Latency budget documented.

## 10. K20 — Similar-image lookup

### Why this is an improvement

Once visual vectors exist, image-to-image search is a cheap, high-value
extension. There is no such path today.

### Implementation

- Add a read-only command, e.g. `find_similar_images(path, limit)`.
- Resolve the selected image to its stored visual vector. If missing, return a
  clear state; optionally embed it on demand, but do not block the UI.
- Scan the visual store for nearest neighbors, excluding the source path.
- Return the existing result shape with paths, scores, thumbnails, and match
  type. Add a related/similar response shape only if the existing shape cannot
  carry it.
- Wire through the daemon:
  - add a JSON-RPC method in `cli/src/bin/kensaku-daemon.rs`;
  - add a `DaemonClient` helper in `src-tauri/src/daemon.rs`;
  - keep protocol version 1 if the addition is backward-compatible; bump it if
  you change existing shapes.
- Add a minimal GUI affordance on image results (“Find similar”) and a tooltip
  or empty state. Do not redesign the result row.
- Expose it through MCP only if it is read-only and token-budgeted.
- Tests:
  - identical image is excluded and near-duplicate ranks first;
  - unrelated image is below floor;
  - missing vector returns a clear error;
  - daemon method round-trips with a temp socket/token if the existing test
    harness supports it.

### Acceptance

- Similar images return within the existing visual-search budget.
- Agent access remains read-only.
- Missing visual model degrades gracefully.

## 11. K21 — Native platform OCR

### Why this is an improvement

Tesseract is an external dependency. PP-OCRv6 is optional and still ONNX. Native
macOS Vision and Windows OCR remove install friction and improve platform
integration.

### Implementation

- Add feature-gated modules:
  - `native-ocr-macos` using `objc2-vision`;
  - `native-ocr-windows` using the `windows` crate’s `Media::Ocr`.
- Do not add these dependencies on Linux by default. Linux builds and tests must
  pass without them.
- Extend the OCR abstraction in `src-tauri/src/ocr.rs` with a `Native` backend.
  Keep Tesseract as the default and fallback.
- On macOS/Windows, use native OCR when the feature is enabled and the platform
  API is available. Otherwise fall back to Tesseract, then to the accurate pack
  if selected.
- Route scanned PDFs through the same OCR path. Today `extract_pdf` returns
  `[Scanned PDF: Image only]`; replace that marker with page rendering + OCR.
  Choose a renderer with a compatible license. If rendering cannot be added
  safely, implement native image OCR first and document scanned-PDF rendering as
  a follow-up rather than faking it.
- Add platform-specific tests as `#[cfg(target_os = ...)]`; on Linux, test only
  dispatch/fallback logic.
- Document manual verification commands and expected output in
  `docs/BENCHMARKS.md` or `docs/ARCHITECTURE.md`.
- Do not claim measured accuracy on a platform you did not run.

### Acceptance

- On supported platforms, OCR works without Tesseract.
- Missing native OCR falls back to Tesseract.
- Linux default build is unchanged.
- Scanned PDFs either work or are explicitly documented as blocked.

## 12. K22 — Multilingual golden query set

### Why this is an improvement

There is no frozen general golden query set in the repo. K14 has a small
semantic check only. K22 extends the existing section D “golden query set in CI”
item; it is not a second competing set.

### Implementation

- First implement or locate the section D golden set. If it does not exist yet,
  create the minimal 40–50 query fixture with expected paths in a temp test
  corpus. Keep it deterministic and offline.
- Add CJK and accented-query cases:
  - exact CJK filename;
  - CJK content substring;
  - mixed-script filename;
  - Japanese/Korean samples;
  - accented Latin query.
- Compute recall@5, MRR, and NDCG where practical. Add latency thresholds only
  if the measurement is stable in CI.
- Run the set in a normal test or an ignored benchmark with a documented command.
  Do not make CI depend on a downloaded model.
- Record results in `docs/BENCHMARKS.md`; use `not measured` where honest.

### Acceptance

- One golden set covers intent, lexical, semantic, CJK, and visual queries.
- It runs offline in CI or a documented manual command.
- Regressions fail where deterministic.

## 13. Documentation updates

For every task, update the relevant docs:

- `docs/ROADMAP.md`: status table and task notes.
- `docs/REMAINING.md`: check off or annotate section B/D items.
- `docs/INTELLIGENCE_VISION_PLAN.md`: mark task status, add measured numbers.
- `docs/ARCHITECTURE.md`: provider, visual store, OCR backend, daemon methods.
- `docs/BENCHMARKS.md`: real numbers only, with methodology.
- `README.md`: only if user-facing behavior changes.
- `CHANGELOG.md`: under `[Unreleased]`.

Do not add attribution or links for the anonymized screenshot-search reference.
Do not claim a model is shipped if only the download path exists.

## 14. Verification commands

Run at minimum after each task:

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path cli/Cargo.toml

cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path cli/Cargo.toml

cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path cli/Cargo.toml --all-targets -- -D warnings

npx tsc --noEmit
git diff --check
```

Feature checks when relevant:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib --features accurate-ocr
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features accurate-ocr -- -D warnings

cargo test --manifest-path src-tauri/Cargo.toml --lib --features smart-embeddings
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features smart-embeddings -- -D warnings
```

If disk is tight, remove only build outputs:

```sh
rm -rf src-tauri/target cli/target
```

Never remove `~/.config/kensaku` or the real index.

Run real-model tests only when artifacts are present; otherwise leave them
ignored and document the env vars.

## 15. Final report

When finished, report:

- commits created, one per task;
- files changed per task;
- tests and clippy results, including feature sets;
- real measurements or `not measured` with reasons;
- anything blocked (platform, model license, missing artifact);
- docs updated;
- whether the tree is clean;
- confirmation that nothing was pushed, released, or tagged.

If you cannot complete a task safely, stop that task, leave the tree in a
reviewable state, and explain exactly what is blocked. Do not invent completion.
