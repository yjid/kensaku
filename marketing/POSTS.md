# Launch posts — ready to copy-paste

The one thing that decides whether any of this works: a **10-second demo GIF** (Alt+Space → type "cuda out of memory" → results → Enter opens the screenshot). Posts with a short clip get 2-3x the engagement of text-only. Record it before you post anything.

---

## 1. Reddit — r/SideProject (post first, day 1, 8am ET)

**Title:** You can't Ctrl+F a screenshot. Now you can.

**Body:**

I could never find my own screenshots. I knew the error said "CUDA out of memory". I knew I'd screenshotted it. I had no idea it was called "Screenshot 2026-07-14 094231.png".

So I built Kensaku (検索 = search). It indexes your files and reads inside them — PDFs, DOCX, code, logs, and with OCR, the actual text in your screenshots. It also finds by meaning: "the PDF about database indexing" locates notes-final-2.pdf. And it parses "screenshots from last week".

- 100% local. No account, no telemetry, nothing to the cloud.
- Alt+Space to summon. ~20 MB idle in the tray. 9 MB installer.
- Free, MIT, open source: github.com/yjid/kensaku
- Bonus: Claude or Cursor can search the same index over MCP, read-only.

Windows is the build I've beaten on most; macOS and Linux come from CI and need real-world testing — that's the feedback I'm here for.

What's the first thing you'd search for?

**First comment (post immediately after submitting):**

The single most requested thing so far was "search my screenshots" — OCR runs with Tesseract by default, and Settings has an optional PP-OCRv6 pack for handwriting-ish and low-quality captures. Extractors are a documented extension point if you want to teach it a new format (docs/EXTRACTORS.md).

---

## 2. Reddit — r/opensource (day 1 or 2)

**Title:** Kensaku — local file search that reads inside your files (MIT, Rust + Tauri)

**Body:**

I built Kensaku because I kept losing screenshots of errors I could describe but not name. It indexes your files and reads inside them — PDFs, DOCX, code, logs, and the text in screenshots via OCR. It also finds by meaning and parses natural-language time ("screenshots from last week").

License: MIT. Stack: Rust + Tauri 2, SQLite (WAL) as durable truth, Tantivy for full text, model2vec static embeddings for semantic search (~50 µs/query, no ONNX at query time). ~240 Rust tests. 9 MB installer.

- 100% local, no telemetry, no cloud.
- MCP server so AI agents can search the same index read-only.
- Good first issues are labeled; extractors and agent tooling are where outside PRs land fastest.

Docs live in /docs — ARCHITECTURE.md is the honest map. What format would you want an extractor for?

---

## 3. Reddit — r/windowsapps (day 4)

**Title:** Spotlight for Windows — and it reads inside your screenshots (free, MIT)

**Body:**

Alt+Space to summon, type what you remember, Enter. Kensaku searches file names, full text in documents, and the text inside your screenshots. Meaning search too: "the PDF about database indexing" finds notes-final-2.pdf.

- ~20 MB idle in the tray, 9 MB installer, no account, no telemetry.
- Free, MIT, open source.
- Windows is the most-tested platform — feedback welcome.

github.com/yjid/kensaku

What's the first thing you'd search for?

---

## 4. Reddit — r/productivity (day 4)

**Title:** I stopped organizing files and started searching what's inside them

**Body:**

I used to spend time naming files and building folder hierarchies. Then I realized: I never remember the name, I remember what was in it.

Kensaku searches names, full text, and meaning across your files — including the text in screenshots. "big videos from last month", "the invoice from Tuesday", "screenshots from last week" — all valid queries. No filters to click.

- 100% local, no cloud, no account.
- Alt+Space to summon. ~20 MB in the tray.
- Free, MIT.

The workflow trick: don't organize. Search `invoice modified:30d` beats the folder you were going to create.

github.com/yjid/kensaku — what's your most-used search query?

---

## 5. Hacker News — Show HN (day 1 or 2, 7–10am ET, Tue–Thu)

**Title:** Show HN: Kensaku – local file search that reads inside your screenshots

**Body:**

Kensaku (検索) is local-first file search and a launcher for Windows/macOS/Linux. It indexes file names (~1ms across 100k files, benchmarks with methodology in the repo), full text in PDFs/DOCX/PPTX and code, OCR'd text inside screenshots via Tesseract or an optional PP-OCRv6 pack, and semantic matches — "the PDF about database indexing" finds notes-final-2.pdf. No cloud, no account, MIT.

I built it because I kept losing screenshots of errors I could describe but not name. Indexing is a single pass — each file feeds both the name and text indexes once — with incremental updates on change.

Honest limits: builds are unsigned, no auto-updater yet, thumbnails are images-only. Windows is the most-tested build; macOS/Linux feedback is the thing I want most: github.com/yjid/kensaku

**First comment (post immediately — this is the deep-dive HN readers want):**

Author here. Architecture notes for the technically curious (full map in docs/ARCHITECTURE.md):

- SQLite with WAL is the durable truth; an in-memory snapshot built at startup serves queries and is mutated incrementally. Chose this over a custom mmap format for crash safety and free migrations; the mmap Everything-style format is the documented next step if cold load ever exceeds 400ms at 1M files.
- Name search is served from the in-memory tier (2-4ms budget). Full text is Tantivy (batched). We benchmarked SQLite FTS5 trigram as a replacement and rejected it: 918 MB and 258ms broad BM25 vs Tantivy's numbers.
- Semantic tier: model2vec static embeddings (potion-base-8M, 256-dim, i8-quantized, ~7.7MB table). No ONNX and no neural graph at query time — tokenize, table lookup, weighted mean, normalize, ~50µs/query, ~92% of MiniLM's MTEB average. An optional 40MB pack (fastembed + bge-small) is available for higher quality.
- OCR: Tesseract by default; optional PP-OCRv6 via onnxruntime (load-dynamic — the static ORT build requires AVX2 and SIGILLs older CPUs).
- The launcher destroys its webview when hidden and rebuilds on summon; idle state is ~20MB private on Windows with a 24.5k-file index loaded.
- Agent access is read-only over MCP/loopback HTTP, budgeted in size, content returned as data never instructions.

Known tradeoffs: unsigned binaries, no auto-updater yet, thumbnails image-only. What would you want an extractor for next?

---

## 6. Product Hunt

**Tagline:** Ctrl+F for your screenshots

**Description:** Kensaku indexes your files and reads inside them — PDFs, code, logs, and the text in your screenshots. Find by meaning ("the PDF about database indexing"), parse "screenshots from last week". 100% local, free, MIT. Your AI agent can search it too (MCP).

**Topics:** Productivity, Developer Tools, Open Source, Privacy, AI

**Maker first comment:**

Hey PH 👋 Maker here.

I built Kensaku because I could never find my own screenshots. I knew the error said "CUDA out of memory", I knew I'd screenshotted it, and I had no idea it was called "Screenshot 2026-07-14 094231.png".

So it reads the pixels. OCR, built in. Plus meaning-based search, natural language time filters, and MCP so your AI agent can search the same index read-only. Nothing leaves the machine. MIT.

Honest limits: unsigned builds, no auto-updater yet, Windows is the most-tested platform. macOS/Linux come from CI and need real-world testing — that's the feedback I'm here for. What would you search for first?

---

## 7. Indie Hackers

**Title:** I'm launching Kensaku — local file search that reads your screenshots

**Body:**

I'm launching Kensaku (検索 = search) today.

I built it because I kept losing screenshots of errors I could describe but not name. It indexes your files and reads inside them — PDFs, code, logs, and the text in screenshots via OCR. It also finds by meaning and parses things like "screenshots from last week".

Numbers that matter to me right now: ~20 MB idle in the tray, 9 MB installer, 1.3 ms name search across 100k files, 0 bytes to the cloud. MIT, free.

I'm not charging for it. The goal is to build the launcher people actually want to use, then figure out the business. macOS and Linux come from CI and need real-world testing — if you're on either, I'd love a test report.

What's the first thing you'd search for?

---

## 8. LinkedIn

I shipped Kensaku today — a local file search that reads inside your files.

The itch: I could never find my own screenshots. I knew the error said "CUDA out of memory", I knew I'd captured it, and I had no idea what I'd named the file.

So I built one that reads the pixels. OCR is built in, along with full text search across PDFs/DOCX/code, meaning-based matches, and natural language time filters ("screenshots from last week"). Everything stays on your machine — no account, no telemetry, no cloud.

Stack: Rust, Tauri 2, SQLite + Tantivy, model2vec for semantic search. MIT licensed, free. Windows/macOS/Linux.

If you try it and it breaks, tell me — that's the most useful thing right now: github.com/yjid/kensaku

---

## 9. X / Twitter

**Tweet 1:**
You can't Ctrl+F a screenshot.

So we made Kensaku — local file search that reads inside your files: PDFs, code, logs, and the text in your screenshots.

Describe it: "the PDF about database indexing". It finds notes-final-2.pdf.

100% local. Free, MIT.
github.com/yjid/kensaku

**Reply (4–6h later):**
Alt+Space to summon, type, Enter. 9 MB installer, ~20 MB idle in the tray. Your AI agent can search the same index over MCP — read-only, nothing leaves the machine.

---

## 10. Mastodon / Bluesky

You can't Ctrl+F a screenshot. So I built Kensaku — local file search that reads inside your files (PDFs, code, logs, and the text in your screenshots). Meaning search + natural language time. 100% local, free, MIT. github.com/yjid/kensaku

#opensource #rust #tauri #privacy #productivity

---

## 11. Dev.to / Hashnode

**Title:** How I built a local file search that reads inside your screenshots (Rust + Tauri)

**Outline:**

1. The itch — the screenshot problem, with the concrete example
2. Why "filename search" tools leave money on the table
3. Architecture: SQLite WAL as durable truth, in-memory snapshot, Tantivy for text, model2vec for meaning, Tesseract + PP-OCRv6 for pixels
4. The hard parts: webview destruction on hide, OCR budgeting, keeping memory flat, the theme bug that wouldn't die
5. Benchmarks with methodology (link docs/BENCHMARKS.md)
6. What's next — link the roadmap Discussions
7. CTA: try it, star it, tell me what breaks

---

## The one thing that decides whether any of this works

A **10-second demo GIF**: Alt+Space → type "cuda out of memory" → results appear → Enter opens the screenshot. No audio. Posts with a short clip get 2–3x the engagement of text-only. Record it before you post anything.

## Universal 2-line pitch (use anywhere, verbatim)

Kensaku is local file search that reads inside your files — documents, code, and even the text in your screenshots. 100% local, free, MIT. github.com/yjid/kensaku

## Order of operations (one day, ~3 hours)

| Time (ET) | Action |
|---|---|
| 07:30 | Final read-through. Fix anything stale. |
| 08:00 | r/SideProject (Title 1, hero image). Drop per-sub first comment. |
| 08:15 | r/opensource with the stack line. |
| 09:00 | Show HN. Author deep-dive as first comment immediately. |
| 09:15 | LinkedIn post. |
| 10:00 | Mastodon / Bluesky. |
| 12:00 | X main post + follow-up reply 4–6h later. |
| 14:00 | Product Hunt (separate day — 00:01 PT start). |
| All day | Reply to every comment. Ship one small fix mentioned in a comment. |

## Rules that matter more than copy

1. **Reply speed is the growth engine.** A bug reported in comments and fixed with a pushed commit the same day outperforms any headline.
2. **Never argue feature gaps.** Quote the honest-limits paragraph.
3. **One metric to watch:** downloads on the release page per 1,000 views. If it's low, the README hero is the thing to fix, not the posts.
4. **If HN flags the domain** (new domain, low karma), submit the deep GitHub URL, not the marketing landing page.
5. **Never edit a published release silently.** New release for every change.