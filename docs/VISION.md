# Kensaku — Product Vision & Roadmap

> **Status:** living document. Last updated 2026-10-26, at `v0.3.3`.
>
> This is the product direction: what we are aiming to build and why. Shipped
> behavior lives in [USAGE.md](USAGE.md) and [AGENTS.md](AGENTS.md). Engineering
> gaps live in [REMAINING.md](REMAINING.md). Implementation design lives in
> [ARCHITECTURE.md](ARCHITECTURE.md).

---

## One sentence

**Kensaku is your computer's memory.** It finds files, documents, screenshots,
and photos from a plain sentence — locally, instantly — and plugs into any AI
without uploading your life.

## The bet

Search is broken because it makes people translate a thought into syntax. LLMs
made natural language the default interface, but they do not know your machine.
Kensaku should be the missing layer: a **local, multimodal memory** that
understands intent, filters instantly, and can be handed to any model.

We are **not** building a chat app. Chat is slow. We are building a search that
does not need syntax — something that feels less like querying a database and
more like asking a friend who already knows where things are.

## What "one of its kind" means

Three properties, together. Competitors have one or two of them.

1. **Local multimodal memory** — files, documents, screenshots, photos, and
   personal context. Private by construction, not by policy.
2. **Zero-syntax intent** — plain language is the default. `/` and `@` are
   accelerators, not requirements. The app always shows how it understood you.
3. **Agent-native** — the same warm index serves the human and any MCP-capable
   AI. No upload, no duplicate index, no per-call reload.

## What we have today

Honest inventory. "Shipped" means in the code at `v0.3.3` or landed on `master`
after it (guided first-run setup, first-scan gate, `/dupes`, default-deny
sensitive paths); "planned" means it lives in this document.

| Area | Shipped today | Still missing |
|---|---|---|
| **Engine** | Metadata + Tantivy BM25 + static vectors + OCR, fused with RRF; incremental crawl; inode identity; rename/delete handling; warm start | 1M-file prefilter; filter pushdown; daemon; visual tier |
| **Content** | Text, code, configs, logs; PDF, DOCX/PPTX/XLSX, EPUB/ODT/RTF; image OCR via tesseract | Native OCR; scanned-PDF OCR; video/audio content |
| **Images** | Filename, OCR text, dimensions, thumbnails, perceptual hashes, near-duplicate groups | CLIP/SigLIP visual search; similar images; face identity; captions |
| **Surfaces** | Desktop GUI; CLI; MCP over stdio | Warm shared daemon; local HTTP; one-click agent connect |
| **Interaction** | Search-as-you-type, category chips, list/grid, preview, `/` command menu, `@` scope picker, explicit sort, deterministic plain-language compiler with removable interpretation chips, full filter grammar, guided first-run setup with first-scan gate | Aliases; facets; saved searches; month-name windows |
| **Intelligence** | Semantic tier; frecency; recency boost | Explicit "latest" sort; version grouping; personal vocabulary; optional local LLM query compiler; `/ask` with citations |
| **Trust** | Local-only, no telemetry, atomic config, crash log, Doctor, default-deny sensitive paths and credential extensions | Face-data controls; cloud-consent UX |

Two facts worth stating plainly, because older design docs can read as if they
already shipped:

- The GUI backend is **in-process**. The daemon topology in
  [ARCHITECTURE.md](ARCHITECTURE.md) is the target, not the current state.
- `find_duplicates` is reachable through `/dupes`, which opens a review view
  (open/copy/reveal). Destructive deletion is intentionally not automated.

## What "friend, not search" means in practice

The feeling comes from four behaviors, not from conversation:

1. **It understands the sentence.** "latest resume" is a sort plus an alias,
   not two literal keywords.
2. **It shows its work.** Every interpretation becomes a chip the user can
   remove or edit. No invisible filters, no silent guessing.
3. **It remembers.** Recent queries, pinned files, learned aliases, version
   groups, and "continue where you left off".
4. **It gets out of the way.** No spinners, no layout shift, no chat thread,
   no "How can I help you today?"

## The interaction model

### Rules

| Input | Meaning | Examples |
|---|---|---|
| **Plain language** | Default search. Always works. | `latest resume`, `screenshots from last week`, `docker error in Projects` |
| **`/`** | What you want, or what to do | `/images`, `/pdf`, `/docs`, `/recent`, `/dupes`, `/errors`, `/diagrams`, `/reindex`, `/doctor`, `/settings` |
| **`@`** | Where it lives, or who it is about | `@Projects`, `@Downloads`, `@Work`, `@Photos`, `@me` |
| **Chips** | The visible, editable interpretation | `[Newest] [Docs] [resume OR cv]` |

Advanced grammar (`type:`, `modified:<7d`, `re:`, `mode:exhaustive`) stays
available for power users and agents. It is no longer the primary onboarding
path. Nothing is mandatory except typing what you want.

### Examples

```text
latest resume
→ [Newest] [Docs] [resume OR cv]
→ Resume_2026.pdf, then 3 older versions grouped underneath

/images red cap @Photos
→ [Images] [@Photos] [visual: red cap]

find me in a red cap
→ [Photos of you] [visual: red cap]        (after one-time "this is me")

system design diagram @Projects
→ [Images] [@Projects] [visual: system design diagram]

docker error @Projects /week
→ [Content] [@Projects] [Last 7 days]

/dupes
→ duplicate cleanup, grouped by content hash and near-identical images
```

### First run: setup owns everything

The first run is **one guided flow**, not a settings scavenger hunt. Downloads,
initial configuration, and model acquisition happen **only here**.

```text
Welcome
  → Folders to watch
  → Preferences (theme, shortcut, exclusions preset)
  → Optional packs (OCR language, semantic model, Vision Pack) with sizes
  → First scan: live counts, current path, ETA, resume if interrupted
  → Done: summary (files, folders, docs, images, vectors) + "Start searching"
```

Rules:

- **Downloads live in setup only.** OCR language data, semantic models, and
  Vision Packs are acquired here. Settings may show installed status and offer
  "Run setup again"; it must not duplicate download actions.
- **Initial configuration happens in setup.** Watched folders, exclusion
  presets, theme, shortcut, and optional packs are asked once, in context.
  Settings shows the current values and links to "Run setup again" instead of
  duplicating the flow.
- **The first scan completes before the main window opens.** Show real counts
  (files, folders, documents, images, OCR'd images, embedded units), the
  current path, and a credible ETA. Finish with a clear **Done** state.
- **No partial index is presented as complete.** If the scan is interrupted,
  it resumes from a cursor; the Done screen only appears when the first index
  is actually usable.
- **Optional packs are truly optional.** Declining them leaves keyword search
  working; the app never blocks on a download unless the user chose it.
- **Subsequent launches stay search-first.** Only the *first* index waits.
  Later boots load the snapshot instantly and reconcile in the background,
  exactly as today.

### Response shape

Every response should carry more than results:

- `interpretation` — the structured reading of the input, rendered as chips
- `sort` — best match, newest, oldest, largest, name
- `facets` — live counts for the current result set
- `total` — how many matched before the display limit
- `as_of` — freshness, as today
- `partial` — when a tier missed its deadline

## The five pillars

| Pillar | Question it answers | What it requires |
|---|---|---|
| **Understand** | What did the user mean? | Intent compiler, aliases, sort, chips, version grouping |
| **Filter** | How do I narrow this instantly? | Facet counts, multi-select/exclude, pushdown, saved searches |
| **See** | What is in this image? | Visual embeddings, native OCR, optional face identity, captions |
| **Connect** | How does my AI use this? | Daemon, HTTP/MCP, one-click connect, cited `/ask` |
| **Remember** | Why does it feel personal? | Recents, pins, learned aliases, smart folders, contextual home |

"Remember" is cross-cutting. It is the difference between a fast tool and a
friend.

---

## What to change

| Area | Today | Target |
|---|---|---|
| **Input** | Query grammar is the interface | Plain language first; `/` and `@` as accelerators; chips as the syntax |
| **Recency** | Ranking bonus | First-class sort; "latest" means newest |
| **Filters** | Post-filters in the query path; client-side counts | Server-side facet counts; multi-select; exclude; pushdown |
| **Results** | Flat list / grid | Best-guess result, grouped versions, facets, related files |
| **Commands** | `>` prefix in the search box | `/` command menu; same capabilities, discoverable |
| **Home** | Empty search box | Recent work, pins, suggestions, one input |
| **Setup** | Guided first-run flow (folders → exclusions → preferences → optional packs → first scan → Done); downloads live in setup; Settings shows status and **Run setup again** | Vision Packs; setup-owned initial configuration refinements |
| **Images** | OCR and filenames only | Visual search, similar images, diagrams, optional identity |
| **LLM** | MCP config snippet | One-click connect; optional local intent compiler; cited `/ask` |
| **Architecture** | In-process backend + per-call CLI | Warm daemon shared by GUI, CLI, and agents |
| **Voice** | Functional, editorial | Human, anticipatory, quiet; no chat assistant persona |

## What to add, prioritized

| Priority | Capability | Why it matters | Effort |
|---|---|---|---|
| **P0** | ~~Natural-language intent compiler (local rules)~~ ✅ shipped | Makes "latest resume" work without syntax | M |
| **P0** | Aliases and personal vocabulary | The app learns that resume = cv = curriculum | S |
| **P0** | ~~Explicit sort modes~~ ✅ shipped | "Latest", "biggest", "oldest" must actually sort | S |
| **P0** | ~~Interpretation chips~~ ✅ shipped | Trust and discoverability; no silent AI | S |
| **P0** | Version grouping | `resume_v1`, `resume_final`, `resume_final_final` become one row | S |
| **P0** | Facet bar with live counts | "Filter anything" feels instant | M |
| **P0** | Saved searches / smart folders | Personal memory; repeatable intent | M |
| **P0** | Duplicate cleanup UI | Backend already exists; high perceived value | S |
| **P0** | Guided first-run setup flow | One place for folders, preferences, downloads, and a completed first scan | M |
| **P0** | Default-deny sensitive files | Must precede face and vision features | S |
| **P0** | ~~`/` command menu and `@` scope mentions~~ ✅ shipped | Familiar LLM-style acceleration | S |
| **P0** | New home and result states | Makes it feel personal before the first keystroke | M |
| **P1** | Visual embeddings (SigLIP/CLIP int8) | "red cap", "system design diagram", similar images | L |
| **P1** | Native OCR (macOS Vision, Windows OCR) | Removes tesseract install friction | M |
| **P1** | "This is me" face identity | "find me in a red cap" | L |
| **P1** | Daemon + local HTTP/MCP | Instant agent access; shared warm index | L |
| **P1** | One-click agent connect | Turns MCP from config homework into a feature | M |
| **P1** | Optional local LLM `/ask` with citations | Answers "what was I working on?" without cloud | M |
| **P2** | VLM captions for diagrams/screenshots | Complements CLIP where text queries are weak | M |
| **P2** | Multilingual embeddings + CJK tokenization | Global product; already a known correctness gap | M |
| **P2** | `find_symbol`, `find_related`, pagination | Agent depth | M |
| **P2** | Signing, notarization, auto-update, icons | Adoption blockers, not differentiators | M |

---

## Roadmap

Each milestone ships something usable on its own. Nothing below requires the
next milestone to be useful.

### Start here (Milestone 1 work order)

If you are picking this up today, take these in order. Each is independently
reviewable and testable.

1. **Add `Sort` to the query AST and executor.** `BestMatch` keeps today's RRF
   order; `Newest`, `Oldest`, `Largest`, and `Name` sort after candidate
   collection. This alone makes "latest" honest.
2. **Add the alias table and deterministic intent rules** behind a config flag.
   Emit an `Interpretation`; do not change ranking yet.
3. **Extend the response** with `interpretation` and `sort`; render them as
   removable chips. Add the 40-query golden set before tuning the rules.
4. ~~**Add `/` and `@` to the search input.**~~ ✅ `/` opens the command menu
   and `@` resolves scopes through the directory index. The grammar remains the
   substrate; `Ctrl+K` is gone.
5. ~~**Build the guided setup flow.**~~ ✅ Folders, preferences, optional packs,
   and downloads live there; the main window is gated on the first scan.
6. **Ship the home and grouped-result states**, then build out the duplicate
   review view (the `>dupes` → `/dupes` review list already exists).
7. **Only then** consider the optional local LLM compiler. The deterministic
   path must be good without it.

### Milestone 1 — "It understands me"

**Goal:** a non-technical user gets the right file from a sentence without
learning syntax.

**Scope**
- Deterministic intent compiler: recency words, size words, time windows,
  "in <place>", media/document nouns, error/log nouns
- Aliases and personal vocabulary (seeded + user-editable)
- First-class sort modes and version grouping
- Interpretation chips and low-confidence clarification (chips, not chat)
- `/` command menu and `@` scope mentions
- New home state (recent, pins, suggestions) and grouped result states
- Duplicate cleanup view
- Guided first-run setup: folders, preferences, optional packs, first scan with
  live counts, and a Done summary
- Downloads and initial configuration only in setup; Settings shows status and
  can re-run setup

**Acceptance**
- A fresh install goes welcome → folders → preferences → optional packs → first
  scan → Done without opening Settings. No download is initiated anywhere else.
- The scan shows files, folders, documents, images, OCR'd images, and embedded
  units, plus the current path and a credible ETA. The Done screen only appears
  when the index is usable.
- An interrupted first scan resumes; a partial index is never presented as
  complete.
- After setup, later launches remain search-first and instant.
- A frozen 40-query golden set includes "latest resume", "screenshots from
  last week", "docker error in Projects", "my biggest videos", "the invoice
  from March". Top-1 or top-3 expectations are met.
- Every query shows its interpretation. No invisible filters.
- One-shot success (user opens a result without refining) ≥ 70% on the set.
- p50 keystroke → paint stays under the existing gates.

**Not in scope:** visual search, face identity, daemon, cloud LLM.

### Milestone 2 — "It filters like lightning"

**Goal:** narrowing a million files feels like toggling, not querying.

**Scope**
- Server-side facet counts for type, language, place, time, size
- Multi-select and exclude within facets
- Filter pushdown into Tantivy fast fields
- Saved searches and smart folders
- Pagination / load-more for ranked results
- 1M-file prefilter work needed to hold the latency gate

**Acceptance**
- Facet counts are accurate for the whole result set, not the current page.
- Toggling a facet updates results and counts in under 50 ms at 100k files.
- p50 < 15 ms and p95 < 40 ms at 1M files, or the milestone is not done.
- Filters can never silently produce zero results; invalid input becomes text
  plus a hint.

**Not in scope:** visual models, agent transport.

### Milestone 3 — "It sees"

**Goal:** images become first-class memory, not OCR attachments.

**Scope**
- Optional Vision Pack: SigLIP or CLIP ViT-B/32 int8, loaded dynamically
- Text → image, image → similar images, diagram and screenshot search
- Native OCR on macOS and Windows; tesseract fallback
- Optional "this is me" enrollment: local face embeddings, local clustering
- Explainable image matches ("face 0.82 · red cap 0.91")
- Optional idle-time VLM captions for diagrams and screenshots

**Acceptance**
- "find me in a red cap", "system design diagram", and "that whiteboard
  photo" return the right image in the top 5 on a 50k-image corpus.
- Enrollment is opt-in, local-only, reversible, and delete-all works.
- No image bytes leave the device in the default configuration.
- 50k-image scan stays within the interaction budget.

**Not in scope:** video/audio transcription, cloud vision.

### Milestone 4 — "It talks to your AI"

**Goal:** any MCP-capable AI can use the live index instantly, and a local
model can answer questions with citations.

**Scope**
- Warm daemon shared by GUI, CLI, and MCP
- Local HTTP/SSE endpoint with token auth
- One-click connect for Claude Desktop/Code, Cursor, and other MCP clients
- Optional local LLM (Ollama/LM Studio) as a **query compiler**, plus `/ask`
  with path and line citations
- Explicit cloud-LLM consent UX; local-only remains the default

**Acceptance**
- A warm MCP search round-trip completes in under 50 ms p95.
- Connecting a supported client takes under 60 seconds.
- `/ask` answers cite real paths and lines; no answer is invented without a
  retrieval step.
- Zero network calls when no cloud provider is configured.

**Not in scope:** write actions, autonomous file changes.

### Milestone 5 — "Ship it properly"

**Goal:** remove adoption friction.

**Scope**
- macOS signing/notarization, Windows Authenticode
- Tauri auto-updater with release channels
- Real icons and tray artwork
- CLI binaries attached to releases
- 60-second demo, benchmark post, golden-set results published

**Acceptance**
- Clean install on Linux, macOS, and Windows without unsigned-build warnings.
- Update path works from the previous release.
- The demo shows "latest resume", a facet narrow, an image query, and an
  agent query without editing a config file.

---

## Metrics

**North star:** time from thought to the correct file — and how often the user
gets there without refining.

| Metric | Target |
|---|---|
| One-shot success on the 40-query golden set | ≥ 80% by Milestone 3 |
| p50 keystroke → paint at 1M files | < 15 ms |
| p95 keystroke → paint at 1M files | < 40 ms |
| Interpretation visible | 100% of queries |
| Silent filters / silent zero-results | 0 |
| First-run path | One guided flow: folders → preferences → packs → scan → Done |
| Downloads outside setup | 0 |
| First scan completion | Counts shown; resumes after interruption; no partial index marked complete |
| First run decisions | Folders + optional packs, all inside the guided flow |
| MCP search p95, warm | < 50 ms |
| Network calls in default configuration | 0 |
| Facet toggle → repaint at 100k files | < 50 ms |

Measure the golden set in CI with recall@5, MRR, and NDCG@10, alongside the
existing latency gates. Product quality that is not measured will regress.

---

## Non-goals and guardrails

**Non-goals**

- A chat-first product with a conversation transcript as the primary surface.
- Accounts, cloud sync, telemetry, or cross-device state.
- A bundled multi-gigabyte LLM.
- Write actions, file deletion, or shell access through the agent API.
- Mandatory syntax or a manual users must read.
- Uploading the index or maintaining a second cloud index.

**Guardrails**

- Local-only by default. Any cloud call is explicit, per-use, and visible.
- Downloads and initial configuration happen in the guided setup flow only.
  Settings shows status and can re-run setup; it never adds a parallel download
  path.
- A partial first index is never presented as complete. The Done state appears
  only when the index is usable.
- Sensitive paths are default-deny before vision or face features ship:
  `~/.ssh`, `~/.gnupg`, browser profiles, password managers, `.env`, `*.pem`,
  `*.key`.
- Face embeddings are opt-in, stored locally, explainable, and deletable.
- The LLM interprets and explains; the index retrieves. The model never
  invents a result without a retrieval step.
- Every capability is reachable from the one box — a chip, a `/` command, or a
  result action — never a new primary screen.
- Results never reorder while the user is still typing.

---

## Open decisions

1. **Face identity scope.** Cluster-first with user labels, or enrollment-first
   with 3–5 photos? What is the false-positive threshold, and how is "delete
   my face data" verified?
2. **Cloud LLM boundary.** Query-only compilation, or snippets too? What does
   per-use consent look like without nagging?
3. **Daemon timing.** Build before vision (agent value sooner) or after
   Milestone 2 (less migration churn)? Current recommendation: Milestone 4.
4. **Vision Pack model.** SigLIP vs CLIP; size, license, and int8 quality on
   desktop CPUs. Native OCR order: macOS first or Windows first?
5. **Multilingual timing.** CJK tokenization is a correctness gap today; the
   multilingual embedding tier may need to precede broad launch.
6. **`/` and `@` vocabulary.** Fixed commands vs user-defined aliases; is
   `@me` special or just another alias?
7. **Saved searches.** User-created only, or auto-suggested from repeated
   queries? If auto, never create silently.
8. **Version grouping.** Heuristics by name + size + time, or explicit user
   merging? False grouping is worse than no grouping.
9. **First scan on very large drives.** Block the main window with progress and
   an ETA, or offer an explicit "continue in background" escape hatch? Default:
   block. The escape hatch, if any, must be visible and must not mark the index
   complete.

---

## How we work

- Each milestone ships usable; no milestone depends on the next.
- New capabilities arrive as chips, `/` commands, or result actions, not new
  primary screens.
- Update this document when scope changes. Update
  [REMAINING.md](REMAINING.md) when an engineering item moves.
- Every UX claim gets a test: golden-set query, latency gate, or privacy check.
- Keep [USAGE.md](USAGE.md) and [AGENTS.md](AGENTS.md) truthful about what is
  shipped. This file is allowed to describe the future; those files are not.

---

## Document map

| Document | Owns |
|---|---|
| **VISION.md** (this file) | Product direction, interaction model, roadmap, metrics |
| [USAGE.md](USAGE.md) | Shipped GUI behavior and query language |
| [AGENTS.md](AGENTS.md) | Shipped CLI/MCP contract |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System design and planned technical layers |
| [REMAINING.md](REMAINING.md) | Engineering gaps and deferred work |
| [EDGE-CASES.md](EDGE-CASES.md) | Correctness catalogue |
| [BENCHMARKS.md](BENCHMARKS.md) | Measurement methodology |
