# Kensaku launch kit (1.0.7)

Everything needed to announce the app. Tone rules: concrete over adjectives,
honest limits included, every post ends with a question or an ask for feedback.
Never crosspost the same day to overlapping audiences.

---

## 1. Reddit

### Titles (ranked)

1. `You can't Ctrl+F a screenshot. Now you can.`
2. `I stopped naming files and started searching what's inside them`
3. `Spotlight for Windows — and it reads inside your screenshots (free, MIT)`

Title 1 for r/SideProject and r/productivity (curiosity + familiar pain).
Title 3 for r/windowsapps (Mac-envy phrase converts there). Title 2 is the
safe personal-story fallback anywhere.

### Body (all subs except r/opensource)

```
I could never find my own screenshots. I knew the error said "CUDA out of
memory". I knew I screenshotted it. I did not know it was called
"Screenshot 2026-07-14 094231.png".

So I built Kensaku (検索 = search). It indexes your files and reads inside
them — PDFs, DOCX, code, logs, and with OCR, the actual text in your
screenshots. It also finds by meaning: "the PDF about database indexing"
locates notes-final-2.pdf. And it parses "screenshots from last week".

  • 100% local. No account, no telemetry, nothing to the cloud.
  • Alt+Space to summon. ~20 MB of RAM idling in the tray. 9 MB installer.
  • Free and open source, MIT: github.com/yjid/kensaku
  • Bonus: Claude or Cursor can search the same index over MCP, read-only.

Windows is the build I've beaten on most; macOS and Linux come straight
from CI and need real-world testing — that's exactly the feedback I'm
here for.

What's the first thing you'd search for?
```

### r/opensource variant

Same body, plus this line before the final question:

```
License: MIT. Stack: Rust + Tauri, SQLite (WAL) as durable truth, Tantivy
for full text, model2vec static embeddings for semantic search. Docs live
in /docs — ARCHITECTURE.md is the honest map. Extractors are a documented
extension point (docs/EXTRACTORS.md) if you want to teach it a new format.
```

### Attach

`assets/search-at-full-speed.png` to every Reddit post (visual ≈ 2× engagement).

### Per-sub first comment (post it yourself immediately after submitting)

- r/SideProject: "The single most requested thing so far was 'search my
  screenshots' — OCR runs with Tesseract by default, and Settings has an
  optional PP-OCRv6 pack for handwriting-ish and low-quality captures."
- r/productivity: "The workflow trick: don't organize. Search 'invoice
  modified:30d' beats the folder you were going to create."
- r/windowsapps: "Alt+Space is the default summon (rebindable in Settings).
  It starts hidden in the tray and trims to ~20 MB while idle."
- r/opensource: "Good first issues are labeled in the tracker; extractors
  and agent tooling are the two areas where outside PRs land fastest."

---

## 2. Show HN

### Title (mechanism-forward, zero hype — HN rule)

```
Show HN: Kensaku – local file search that reads inside your screenshots
```

### Body (~110 words)

```
Kensaku (検索) is local-first file search and a launcher for
Windows/macOS/Linux. It indexes file names (~1ms across 100k files,
benchmarks with methodology in the repo), full text in PDFs/DOCX/PPTX and
code, OCR'd text inside screenshots via Tesseract or an optional
PP-OCRv6 pack, and semantic matches — "the PDF about database indexing"
finds notes-final-2.pdf. No cloud, no account, MIT.

I built it because I kept losing screenshots of errors I could describe
but not name. Indexing is a single pass — each file feeds both the name
and text indexes once — with incremental updates on change.

Honest limits: builds are unsigned, no auto-updater yet, thumbnails are
images-only. Windows is the most-tested build; macOS/Linux feedback is
the thing I want most: github.com/yjid/kensaku
```

### Author first comment (post immediately — the technical deep-dive)

```
Author here. Architecture notes for the technically curious (full map in
docs/ARCHITECTURE.md):

- SQLite with WAL is the durable truth; an in-memory snapshot built at
  startup serves queries and is mutated incrementally. Chose this over a
  custom mmap format for crash safety and free migrations; the mmap
  Everything-style format is the documented next step if cold load ever
  exceeds 400ms at 1M files.
- Name search is served from the in-memory tier (2-4ms budget). Full text
  is Tantivy (batched). We benchmarked SQLite F5 trigram as a replacement
  and rejected it: 918 MB and 258ms broad BM25 vs Tantivy's numbers in
  docs/BENCHMARKS.md.
- Semantic tier: model2vec static embeddings (potion-base-8M, 256-dim,
  i8-quantized, ~7.7MB table). No ONNX and no neural graph at query time —
  tokenize, table lookup, weighted mean, normalize, ~50µs/query, ~92% of
  MiniLM's MTEB average. An optional 40MB pack (fastembed + bge-small) is
  available for higher quality.
- OCR: Tesseract by default; optional PP-OCRv6 via onnxruntime
  (load-dynamic — the static ORT build requires AVX2 and SIGILLs older
  CPUs).
- The launcher destroys its webview when hidden and rebuilds on summon;
  idle state is ~20MB private on Windows with a 24.5k-file index loaded.
- Agent access is read-only over MCP/loopback HTTP, budgeted in size,
  content returned as data never instructions.

Known tradeoffs we argued about: unsigned binaries (signing costs are
real for an independent app), no auto-updater yet, thumbnails image-only
for now. What would you want an extractor for next?
```

---

## 3. X / Twitter

### Main post

```
You can't Ctrl+F a screenshot.

So we made Kensaku — local file search that reads inside your files:
PDFs, code, logs, and the text in your screenshots.

Describe it: "the PDF about database indexing". It finds notes-final-2.pdf.

100% local. Free, MIT.
github.com/yjid/kensaku
```

### Follow-up (reply to own post)

```
Alt+Space to summon, type, Enter. 9 MB installer, ~20 MB idle in the
tray. Your AI agent can search the same index over MCP — read-only,
nothing leaves the machine.
```

---

## 4. Playbook

| When | Where | Notes |
|---|---|---|
| Day 1, 8–11am ET | r/SideProject | Title 1, hero image attached, own first comment |
| Day 1 or 2, 7–10am ET | Show HN | Tue–Thu only. First comment immediately, answer everything for 24h |
| Day 3 | r/opensource | Title 2 + license/stack line, contribution-focused first comment |
| Day 4 | r/productivity, r/windowsapps | Crosspost variants, own per-sub comments |
| Same days | X | Main post morning ET, follow-up reply after 4–6h |

Rules that matter more than copy:

1. **Reply speed is the growth engine.** A bug reported in comments and
   fixed with a pushed commit the same day outperforms any headline.
2. **Never argue feature gaps.** Quote the honest-limits paragraph.
3. **One metric to watch:** downloads on the release page per 1,000 views.
   If it's low, the README hero is the thing to fix, not the posts.
4. **If HN flags the domain** (new domain, low karma), submit the deep
   GitHub URL, not the marketing landing page.
5. Update the [latest release](https://github.com/yjid/kensaku/releases/latest)
   notes only via new releases — never edit a published release silently.
