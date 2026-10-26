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
- **Images:** OCR text extracted with tesseract when the engine and language data are available.
- **Images, beyond text:** thumbnails, dimensions, and perceptual near-duplicate grouping for cleanup.

Most "fast search" tools stop at filenames. Kensaku opens the file.

### It gives the same power to your AI agents

This is the part that changes your workflow. Kensaku ships an MCP server over stdio, so compatible agents can search your local index — read-only — instead of guessing at paths or asking you to paste content into chat.

Your agent gets three tools:

| Tool | What it does |
|---|---|
| `search` | Ranked file/content search with path, extension, and recency filters |
| `read_snippet` | A bounded snippet from an indexed file, centered on a query |
| `index_state` | Index freshness and size |

One index. You search it in the app; your agent searches it in a tool call. No re-indexing, no cloud sync, no API key.

### It stays on your machine

No telemetry. No account. No hosted search service. Indexes and configuration live in your OS config directory. The only outbound request is an optional model or OCR language download that you initiate.

### It is genuinely fast

Measured warm-cache on a ~100,000-file corpus, Kensaku's keystroke hot path (walk + fuzzy filename search) averages around **2.8 ms per query**. That is not a marketing number — the benchmark harness ships in the repository, and the methodology is documented.

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

If `kensaku-cli` is not on `PATH`, use its absolute path. The server binds to stdio only; tools are read-only and responses are token-budgeted. File content is returned as data, never as instructions. Full guide: [docs/USAGE.md](docs/USAGE.md).

## Performance

Warm page cache, release build, Linux, local SSD, ~100,000-file synthetic corpus with default exclusions:

| Phase | Result |
|---|---|
| Walk | ~190 ms per 100k files (~525k files/s) |
| Fuzzy filename search | ~2.8 ms average per query (p50) |
| Memory | 128 B per entry (~12.8 MB per 100k files) |

Reference values, not guarantees. The benchmark isolates the keystroke hot path and deliberately skips index writes, so it is not end-to-end indexing latency. Full methodology: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Privacy and security

- **Local by default.** No telemetry, no account, no cloud search.
- **You choose what is indexed.** Watched folders and exclusions are yours to set. Hidden files and common junk directories are skipped; common credential extensions (`.pem`, `.key`, `.pfx`, …) are excluded by default. Add `.ssh`, browser profiles, and password-manager folders to exclusions.
- **Bounded extraction.** 8-second timeout per file, 1 MB indexed content cap, 20 MB extraction ceiling, 50 MP image cap.
- **Read-only agent access.** The CLI and MCP server open the index without the writer lock and expose no write, shell, or network tools.
- **Treat the index as sensitive.** It contains filenames and extracted text; don't sync it to a shared location.

Full threat model: [SECURITY.md](SECURITY.md).

## Project status

**Available today:** desktop app, CLI, MCP, semantic + lexical hybrid search, full query grammar, PDF/Office/EPUB/ODT/RTF extraction, tesseract OCR, image thumbnails and near-duplicate grouping, incremental indexing, rename/delete tracking, two themes, Doctor, factory reset, guided first-run setup, and 95+ Rust tests.

**Planned:** natural-language intent without syntax, `/` commands and `@` scopes, facet counts, saved searches, CLIP/SigLIP visual search, face identity, native platform OCR, warm shared daemon, 1M-file prefilter, signing/notarization, auto-update. See [docs/REMAINING.md](docs/REMAINING.md) and [docs/VISION.md](docs/VISION.md).

## Contributing

- [Contributing guide](docs/CONTRIBUTING.md)
- [Security policy](SECURITY.md) — please report vulnerabilities privately
- [Issue tracker](https://github.com/yjid/kensaku/issues)
- [MIT license](LICENSE)

Useful contributions: reproducible bug reports, platform testing (especially unsigned-build install paths), search-quality improvements, agent integrations, and documentation.

---

**More:** [USAGE.md](docs/USAGE.md) · [ARCHITECTURE.md](docs/ARCHITECTURE.md) · [FAQ.md](docs/FAQ.md) · [INSTALL.md](docs/INSTALL.md) · [BENCHMARKS.md](docs/BENCHMARKS.md)