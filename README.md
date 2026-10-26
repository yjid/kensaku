<p align="center">
  <img src="assets/icon.png" width="120" height="120" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>Find anything on your computer. Let your AI find it too.</b><br/>
  <i>Fast, local-first search for files, source code, documents, and images — semantic search, OCR, hybrid ranking, and MCP access for compatible AI agents.</i>
</p>

<p align="center">
  <i>Your computer's search is broken. This one isn't.</i>
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
  <a href="#installation-and-quick-start">Install</a> ·
  <a href="#ai-agent-integration">AI agents</a> ·
  <a href="docs/USAGE.md">Documentation</a> ·
  <a href="https://github.com/yjid/kensaku/releases">Releases</a>
</p>

---

## The problem and the solution

You wrote a file two weeks ago. You need it now. You cannot remember what it was called.

Ordinary filename search fails when the name is forgotten, the folder is wrong, or the content lives inside a PDF, a spreadsheet, or a screenshot. Kensaku builds a persistent local index of filenames, file contents, and extracted text, then answers in milliseconds through one search box.

- **Persistent index.** Walks watched folders once, keeps the index on disk, and reconciles changes in the background.
- **Content retrieval.** Reads supported documents and source files; OCR text from images when tesseract and language data are available.
- **Unified interface.** Desktop app, CLI, and MCP server all query the same index.

## Core capabilities

- **Find by meaning, not just words.** Semantic search via local embeddings finds conceptually related files even when exact terms don't match.
- **Full-text content search.** Across supported files, with snippets and line-level context.
- **Supported documents.** PDF, DOCX/PPTX/XLSX, EPUB/ODT/ODS/ODP, RTF, plain text and code.
- **Image understanding.** OCR text extraction (tesseract), thumbnails, dimensions, and perceptual near-duplicate grouping.
- **Hybrid ranking.** Lexical + semantic + recency + path signals fused via RRF.
- **Query language.** `type:`, `ext:`, `in:`, `path:`, `modified:`, `size:`, `is:`, `lang:`, `name:`, `content:`, `re:`, negation, `mode:exhaustive`.
- **Programmatic access.** `kensaku-cli search` with text or JSON output, filters, and exhaustive enumeration.
- **AI-agent access.** MCP server exposing `search`, `read_snippet`, and `index_state` over stdio.
- **Local-first.** Indexes and config stay on your machine; no telemetry.

## How it works

```
watched folders
   │
   ▼
 walker (jwalk) ──► metadata (SQLite) + Tantivy content index
   │                       │
   ▼                       ▼
 extraction (PDF/Office/OCR) ──► snippets + semantic units
   │
   ▼
 desktop app / CLI / MCP  ──► query ──► hybrid ranking ──► results
```

- **Metadata** (paths, size, mtime, inode identity) lives in SQLite.
- **Content** lives in Tantivy, populated by parallel extractors with an 8-second timeout per file.
- **Semantic units** feed a local embedding tier; the default model is a static table, with optional neural models.
- The desktop backend, CLI, and MCP all read the same on-disk index; the CLI/MCP open it read-only.

## Installation and quick start

Download prebuilt bundles from the [releases page](https://github.com/yjid/kensaku/releases). Builds are currently unsigned (see [docs/INSTALL.md](docs/INSTALL.md) for Gatekeeper/SmartScreen notes).

| Platform | Asset |
|---|---|
| Linux | `kensaku_<ver>_amd64.AppImage`, `.deb`, `.rpm` |
| macOS | `Kensaku_<ver>_universal.dmg` (Intel + Apple Silicon) |
| Windows | `Kensaku_<ver>_x64_en-US.msi`, `kensaku.exe` |

**Build from source** (all platforms):

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri build
```

Prerequisites: Rust stable, Bun 1.1+ (Node 18+ works), plus platform webview libraries. See [docs/INSTALL.md](docs/INSTALL.md).

**First run:** choose watched folders, pick an exclusion preset, start. The first scan completes before the main window opens, showing live counts; later launches are instant. You can search while indexing runs.

**Agent CLI:**

```bash
cargo build --release --manifest-path cli/Cargo.toml
install -m 755 cli/target/release/kensaku-cli ~/.local/bin/kensaku-cli
```

## Usage examples

```bash
# fuzzy filename search
kensaku-cli search "docker error"

# full-text-ish query with filters
kensaku-cli search "error" --ext log --since 7d --limit 20

# JSON for scripts/agents
kensaku-cli search "resume" --json

# index status
kensaku-cli state
```

Query syntax (full reference in [docs/USAGE.md](docs/USAGE.md)): `type:`, `ext:`, `in:`, `path:`, `modified:`, `size:`, `is:`, `lang:`, `name:`, `content:`, `re:`, negation, `mode:exhaustive`.

## AI agent integration

Kensaku is designed to be queried by compatible AI agents over MCP (stdio). The agent gets read-only search access to your local index — not unrestricted filesystem access.

**Tools:**

| Tool | Purpose |
|---|---|
| `search` | Ranked file/content search with filters |
| `read_snippet` | Bounded snippet from an indexed file, centered on a query |
| `index_state` | Index freshness and size |

**Minimal config** (Claude Desktop / Cursor style):

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

If `kensaku-cli` is not on `PATH`, use its absolute path. Tool responses are token-budgeted; file content is returned as data, never as instructions. See [docs/USAGE.md](docs/USAGE.md) for the full integration guide.

## Performance and benchmarks

Measured on Linux, release build, warm page cache, local SSD, ~100,000-file synthetic corpus (default exclusions applied):

| Phase | Result |
|---|---|
| Walk | ~190 ms per 100k files (~525k files/s) |
| Fuzzy filename search | ~2.8 ms average per query (p50) |
| Memory | 128 B per entry (~12.8 MB per 100k files) |

These are reference values for spotting regressions, not guarantees; hardware, filesystem, and query mix all move the numbers. The benchmark measures the keystroke hot path (walk + fuzzy search) and deliberately skips SQLite/Tantivy writes, so it is not end-to-end indexing latency. Full methodology: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Privacy and security

- **Local only.** No telemetry, no account. Indexes and config live in your OS config directory (`~/.config/kensaku/` on Linux, `~/Library/Application Support/kensaku/` on macOS, `%APPDATA%\kensaku\` on Windows).
- **What is indexed.** Filenames and extracted text from watched folders; thumbnails for images. Extraction caps content at 1 MB indexed / 20 MB extraction ceiling; images are capped at 50 MP.
- **Network.** The only outbound request is an optional model or OCR language download that you initiate. Default installs make zero network calls.
- **Agent access.** The CLI and MCP server open the index read-only (no Tantivy writer lock, no mutation tools). They bind to stdio only.
- **Secrets.** Default exclusions skip hidden files, common junk directories, and common credential file extensions (`.pem`, `.key`, `.pfx`, …). Other sensitive locations (`.ssh`, browser profiles, password managers, `.env`) are not auto-excluded; add them to exclusions. Treat the index directory as sensitive.

Full threat model and controls: [SECURITY.md](SECURITY.md).

## Project status

**Available:** durable metadata + warm start, incremental crawl, rename/delete handling, full query grammar, hybrid ranking, semantic vectors, PDF/Office/EPUB/ODT/RTF extraction, tesseract OCR, image thumbnails/dimensions/near-duplicates, CLI + MCP, two themes, Doctor, factory reset, 95+ Rust tests.

**In progress / planned:** natural-language intent, `/` and `@` interaction, facet counts, visual search (CLIP/SigLIP), face identity, native platform OCR, warm daemon, 1M-file prefilter, signing/notarization, auto-update. See [docs/REMAINING.md](docs/REMAINING.md) and [docs/VISION.md](docs/VISION.md).

## Contributing, security, and license

- Contributing: [CONTRIBUTING.md](docs/CONTRIBUTING.md)
- Security: [SECURITY.md](SECURITY.md) (private vulnerability reporting)
- Issues: https://github.com/yjid/kensaku/issues
- License: [MIT](LICENSE)

Useful contributions: reproducible bug reports, platform testing (especially unsigned-build install paths), search-quality improvements, agent integrations, and documentation.

---

**Detailed docs:** [USAGE.md](docs/USAGE.md) · [ARCHITECTURE.md](docs/ARCHITECTURE.md) · [FAQ.md](docs/FAQ.md) · [INSTALL.md](docs/INSTALL.md) · [BENCHMARKS.md](docs/BENCHMARKS.md)