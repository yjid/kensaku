<p align="center">
  <img src="assets/icon.png" width="120" height="120" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>Find anything on your computer. Let your AI find it too.</b>
</p>

<p align="center">
  <i>Semantic search, OCR, and document understanding — one local index for you, your terminal, and your agents.</i>
</p>

<p align="center">
  <a href="https://github.com/yjid/kensaku/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/yjid/kensaku?style=for-the-badge&color=2F7D6D" /></a>
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-2F7D6D?style=for-the-badge" /></a>
  <a href=".github/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/yjid/kensaku/ci.yml?style=for-the-badge" /></a>
</p>

<p align="center">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-AppImage%20%7C%20deb%20%7C%20rpm-111110?style=flat-square&logo=linux&logoColor=white" />
  <img alt="macOS" src="https://img.shields.io/badge/macOS-universal%20dmg-111110?style=flat-square&logo=apple&logoColor=white" />
  <img alt="Windows" src="https://img.shields.io/badge/Windows-msi%20%7C%20exe-111110?style=flat-square&logo=windows&logoColor=white" />
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-111110?style=flat-square&logo=rust&logoColor=white" />
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-111110?style=flat-square&logo=tauri&logoColor=white" />
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#why-kensaku">Why Kensaku</a> ·
  <a href="#ai-agents">AI agents</a> ·
  <a href="docs/USAGE.md">Docs</a> ·
  <a href="https://github.com/yjid/kensaku/releases">Releases</a>
</p>

---

## The search you actually wanted

You remember the file. Not its name.

You know it was a PDF about a failed deploy, or a screenshot of that database error, or the invoice from March. Your operating system's search asks you to remember the filename — and gives up when the answer lives inside the file.

Kensaku indexes what is *in* your files, not just what they are called. Then it answers in milliseconds, on your machine, with no account and no cloud.

- **Search by meaning.** Local embeddings match related ideas, not just exact words.
- **Search inside.** PDFs, Office documents, EPUB, RTF, source code, logs, and images.
- **Search from anywhere.** Desktop app, CLI, and AI agents share the same index.
- **Search privately.** Your index never leaves your device.

## Why Kensaku

### It understands more than filenames

Kensaku combines lexical full-text search with a semantic tier. Ask for "that docker build error" and it can surface the right log or screenshot even when the exact phrase isn't in the name. Ranking fuses text relevance, semantic similarity, recency, and path signals, so the file you touched last week tends to win.

### It reads your documents and images

- **Documents:** PDF, DOCX, PPTX, XLSX, EPUB, ODT/ODS/ODP, RTF, and plain text/code.
- **Images:** OCR text extracted with Tesseract by default (fast, smaller), or the optional PP-OCRv6 pack (slower, in-process; may read small UI text better). Falls back to Tesseract when the pack or the required ONNX runtime library is absent.
- **Images, beyond text:** thumbnails, dimensions, perceptual near-duplicate grouping, and screenshot bursts that collapse to one result row.

Most "fast search" tools stop at filenames. Kensaku opens the file.

### It gives the same power to your AI agents

This is the part that changes your workflow. Kensaku ships an MCP server over stdio, so compatible agents can search your local index — read-only — instead of guessing at paths or asking you to paste content into chat.

Your agent gets five read-only tools, plus an opt-in reindex:

| Tool | What it does |
|---|---|
| `health_check` | Daemon, database, index and model health |
| `search` | Ranked or exhaustive search with cursor pagination, path/extension/recency filters |
| `read_snippet` | A bounded snippet from an indexed file, centered on a query |
| `regex_search` | Bounded regular-expression search over content |
| `index_state` | Freshness, phase, model ids and size |
| `reindex` | Hidden unless `mcp_allow_reindex` is enabled; requires the daemon |

One index. You search it in the app; your agent searches it in a tool call. No re-indexing, no cloud sync, no API key.

### It stays on your machine

No telemetry. No account. No hosted search service. Indexes and configuration live in your OS config directory. The only outbound request is an optional model or OCR language download that you initiate.

### It is genuinely fast

Measured warm-cache on a ~100,000-file corpus, Kensaku's keystroke hot path (walk + fuzzy filename search) averages around **2.8 ms per query**. Indexing is a single extraction pass: each file is read once and feeds both the lexical and semantic indexes, images are OCR'd once, and the crawl keeps a bounded queue instead of holding every extracted document in memory. The benchmark harnesses ship in the repository, and the methodology is documented.

### It speaks your language

For power users and scripts, the query language is real: `type:`, `ext:`, `in:`, `path:`, `modified:`, `size:`, `is:`, `lang:`, `name:`, `content:`, `re:`, negation, and exhaustive mode. For everyone else, type what you remember.

## Install

Download a prebuilt bundle from the [releases page](https://github.com/yjid/kensaku/releases). Builds are currently unsigned; [docs/INSTALL.md](docs/INSTALL.md) covers Gatekeeper and SmartScreen.

| Platform | Asset |
|---|---|
| Linux | `kensaku_<ver>_amd64.AppImage`, `.deb`, `.rpm` |
| macOS | `Kensaku_<ver>_universal.dmg` (Intel + Apple Silicon) |
| Windows | `Kensaku_<ver>_x64_en-US.msi`, `kensaku.exe` |

Build from source:

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri build
```

Prerequisites: Rust stable, Bun 1.1+ (Node 18+ works), plus platform webview libraries — see [docs/INSTALL.md](docs/INSTALL.md). A `Makefile` wraps the common tasks: `make dev`, `make build`, `make test`, `make reset`.

**First run:** choose folders, pick an exclusion preset, optionally download OCR/model packs. The first scan finishes with a Done summary; later launches are instant. You can search while indexing runs.

**Get the CLI:**

```bash
cargo build --release --manifest-path cli/Cargo.toml
install -m 755 cli/target/release/kensaku-cli ~/.local/bin/kensaku-cli
```

## Try it

```bash
# filename and content search
kensaku-cli search "docker error"

# filter by extension and recency
kensaku-cli search "error" --ext log --since 7d --limit 20

# JSON for scripts and agents
kensaku-cli search "resume" --json

# index health
kensaku-cli state
```

Query syntax reference: [docs/USAGE.md](docs/USAGE.md).

## AI agents

Point a compatible MCP client at the CLI:

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

If `kensaku-cli` is not on `PATH`, use its absolute path. The server binds to stdio only. It forwards to the warm daemon when one is running and otherwise falls back to a read-only local index. Tools are read-only by default; `reindex` stays hidden unless `mcp_allow_reindex` is enabled in settings. Responses are token-budgeted, and file content is returned as data, never as instructions. Full guide: [docs/AGENTS.md](docs/AGENTS.md).

## Performance

Warm page cache, release build, Linux, local SSD, ~100,000-file synthetic corpus with default exclusions:

| Phase | Result |
|---|---|
| Walk | ~190 ms per 100k files (~525k files/s) |
| Fuzzy filename search | ~2.8 ms average per query (p50) |
| Metadata footprint | 128 B per entry (~12.8 MB per 100k files) |

Indexing reference on the development machine (in-tree `index_bench` and `crawl_bench` harnesses). These runs used a debug build, so they are **conservative — release builds are faster**; dependency code is now optimized in the dev profile, so `make dev` is close to release. OCR time itself is bound by the external tesseract process and is build-independent:

| Workload | Result |
|---|---|
| 200 screenshots, OCR | ~12 s (~17 files/s) |
| 400 screenshots, OCR | ~26 s (~15 files/s) |
| Fresh crawl, 703 mixed files (400 images + 300 text) | ~23–27 s |
| Warm re-crawl of the same tree | ~0.7 s |
| Indexer process peak RSS, text-only / mixed | ~73 MB / ~73 MB |

Perceptual image hashes are computed after the crawl in a low-priority
background pass (one decode at a time), so `/dupes` fills in progressively and
never blocks indexing. The watcher hashes and groups changed screenshots
immediately; the initial crawl reuses only already-fresh thumbnails, and the
idle pass computes missing hashes and groups older images 50 at a time. The
mixed crawl used the fallback embedder; a real install
adds the ~30 MB static embedding table when the semantic pack is installed.

Reference values, not guarantees. The query-path benchmark isolates the keystroke hot path and deliberately skips index writes, so it is not end-to-end indexing latency. Indexer RSS excludes the ~30 MB static embedding table downloaded by the semantic pack. Full methodology: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Privacy and security

- **Local by default.** No telemetry, no account, no cloud search.
- **You choose what is indexed.** Watched folders and exclusions are yours to set. Hidden files and common junk directories are skipped; common credential extensions (`.pem`, `.key`, `.pfx`, …) are excluded by default. Add `.ssh`, browser profiles, and password-manager folders to exclusions.
- **Bounded extraction.** 8-second timeout per file with a bounded pool of extraction slots, 1 MB indexed content cap, 20 MB extraction ceiling, 50 MP image cap.
- **Read-only agent access by default.** The CLI and MCP server expose no shell or network tools. The only write tool, `reindex`, is hidden and disabled unless explicitly enabled and a daemon is running.
- **Treat the index as sensitive.** It contains filenames and extracted text; don't sync it to a shared location.

Full threat model: [SECURITY.md](SECURITY.md).

## Project status

**Available today:** desktop app, daemon-backed GUI/CLI/MCP over Unix socket or Windows named pipe, loopback HTTP/SSE agent transport with bearer-token auth, semantic + lexical + folded/trigram hybrid search, CJK-aware tokenization, config-only external extractors, full query grammar, PDF/Office/EPUB/ODT/RTF extraction, tesseract OCR plus an optional PP-OCRv6 pack, image thumbnails, near-duplicate grouping and screenshot bursts, incremental indexing, rename/delete tracking, `.gitignore`/`.cursorignore` parity, agent root scoping and keyset pagination, tree-sitter `find_symbol`, one-click Claude/Cursor MCP connect, an optional local-LLM query compiler and cited `/ask`, two themes, Doctor, factory reset, guided first-run setup, and 270+ Rust tests plus the golden-query integration test.

**Known limitations:** builds are currently unsigned (see [INSTALL.md](docs/INSTALL.md)), the auto-updater is not wired yet, and native platform OCR is not implemented. Visual search remains a **Settings → Beta** model-download experiment and is not wired into search. Face identity/VLM captions are not included. Agent access is read-only unless `reindex` is explicitly enabled.

## Contributing

- [Contributing guide](docs/CONTRIBUTING.md)
- [Security policy](SECURITY.md) — please report vulnerabilities privately
- [Issue tracker](https://github.com/yjid/kensaku/issues)
- [MIT license](LICENSE)

Useful contributions: reproducible bug reports, platform testing (especially unsigned-build install paths), search-quality improvements, agent integrations, and documentation.

---

**More:** [USAGE.md](docs/USAGE.md) · [ARCHITECTURE.md](docs/ARCHITECTURE.md) · [AGENTS.md](docs/AGENTS.md) · [EXTRACTORS.md](docs/EXTRACTORS.md) · [FAQ.md](docs/FAQ.md) · [INSTALL.md](docs/INSTALL.md) · [BENCHMARKS.md](docs/BENCHMARKS.md) · [RELEASING.md](docs/RELEASING.md)