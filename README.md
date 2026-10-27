<p align="center">
  <img src="assets/icon.png" width="112" height="112" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>Find any file on your computer in milliseconds.</b>
</p>

<p align="center">
  Search filenames, file contents, and meaning in one fast, local index.<br/>
  Nothing leaves your machine.
</p>

<p align="center">
  <a href="https://github.com/yjid/kensaku/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/yjid/kensaku?style=for-the-badge" /></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-111111?style=for-the-badge" /></a>
  <a href="https://github.com/yjid/kensaku/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/yjid/kensaku/ci.yml?branch=master&style=for-the-badge" /></a>
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#features">Features</a> ·
  <a href="#performance">Performance</a> ·
  <a href="#connect-ai-agents-mcp">MCP</a> ·
  <a href="docs/USAGE.md">Docs</a> ·
  <a href="https://github.com/yjid/kensaku/releases">Releases</a>
</p>

<p align="center">
  <img src="assets/fastest-semantic-file-search.png" width="100%" alt="Kensaku search results" />
</p>

---

## Find it the way you remember it

You don't remember the filename. You remember what was in it.

- The PDF about database indexing, saved as `notes-final-2.pdf`.
- The log line from the failed deploy, somewhere in a folder of logs.
- The screenshot with a CUDA error, which has no useful name at all.

Type what you remember. Kensaku searches names, text inside files, and meaning, and shows the match with the words highlighted. Open it with Enter.

## Features

**Fast**
- Filename search across ~100,000 files in a few milliseconds on a warm cache. Method and numbers are in [Performance](#performance).
- Results update as you type. There is no separate search page.
- Incremental indexing: changed files are picked up without rescanning the whole disk.

**Finds more**
- Names, full text, fuzzy matches that tolerate typos, and meaning, in one search.
- Documents: PDF, DOCX, PPTX, XLSX, EPUB, ODT/ODS/ODP, RTF, plain text, code, and logs, including CJK text.
- Images: text is read with Tesseract. An optional, more accurate PP-OCRv6 engine is available in Settings.

**Precise**
- Filters and sort: Best match, Newest, Oldest, Largest, Name.
- A query language when you want it (below).

**Works with AI agents**
- Compatible agents can search the same local index over MCP. Read-only by default. See [Connect AI agents](#connect-ai-agents-mcp).

**Private**
- No account, no telemetry, no cloud search. Your index stays in your OS config directory.

**Pleasant to use**
- Grid or list layout. Quick Look with Space for PDFs, images, and text.
- Right-click for open, show in folder, copy path, and details.
- Light and dark themes.

### The query language

For scripts and precise searches:

```
type:pdf invoice          ext:md in:"~/Notes"       modified:7d
size:>10mb                path:"client/2026"        lang:rust
name:resume               content:"timeout"         re:"fn \w+_test"
-draft                    mode:exhaustive
```

Full reference: [docs/USAGE.md](docs/USAGE.md).

## Install

Download the bundle for your system from the [latest release](https://github.com/yjid/kensaku/releases/latest).

| Platform | Download |
|---|---|
| macOS (Apple silicon and Intel) | `Kensaku_<version>_universal.dmg` |
| Windows | `Kensaku_<version>_x64-setup.exe` or `.msi` |
| Linux | `.AppImage`, `.deb`, or `.rpm` |

Builds are not yet code-signed, so macOS and Windows show a warning the first time. That is expected for an independent app, not a sign of a problem.

- **macOS:** open the app, then go to System Settings → Privacy & Security and click **Open Anyway**.
- **Windows:** on the SmartScreen prompt, click **More info** → **Run anyway**.

[docs/INSTALL.md](docs/INSTALL.md) has the full steps.

**First run:** choose the folders you want indexed, pick exclusions, and optionally download the OCR or model packs. Search works while the first scan runs.

**Command line:** `kensaku-cli` ships with each release.

```bash
kensaku-cli search "docker error"
kensaku-cli search "invoice" --ext pdf --since 30d
kensaku-cli search "resume" --json
kensaku-cli state
```

### Build from source

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri build
```

You need Rust (stable), Bun 1.1 or later, and your platform's webview libraries. See [docs/INSTALL.md](docs/INSTALL.md). A `Makefile` covers the common tasks: `make dev`, `make build`, `make test`, and `make reset`.

## Connect AI agents (MCP)

Point an MCP client at the CLI:

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

For clients that connect over HTTP, run `kensaku-cli serve`. It listens on loopback only and requires a bearer token stored in your config directory.

Responses are budgeted in size, and file content is returned as data, never as instructions. Full guide: [docs/AGENTS.md](docs/AGENTS.md).

## Performance

Measured on a release build, Linux, local SSD, warm cache, with a ~100,000-file synthetic corpus and default exclusions:

| What | Result |
|---|---|
| Walk the tree | ~190 ms per 100k files |
| Filename search, keystroke path | ~2.8 ms average (p50) |
| Index memory per entry | ~128 bytes (~12.8 MB per 100k files) |

Indexing is a single pass: each file is read once and feeds both the text and meaning indexes. Images are read once for OCR. Work is queued in bounded batches, so memory stays flat on large folders.

Results are reference values, not guarantees. Full methodology and the benchmark harnesses are in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Privacy and security

- **Local by default.** No telemetry, no account, no cloud search.
- **You choose what is indexed.** Watched folders and exclusions are yours. Hidden files and common build folders are skipped by default, and credential files such as `.pem`, `.key`, and `.pfx` are excluded.
- **Bounded processing.** Each file has a time limit and a size limit, so one bad file can't stall the index.
- **Read-only agent access.** The agent tools cannot change files. `reindex` is off unless you enable it.
- **Treat the index as sensitive.** It contains file names and extracted text. Don't sync it to a shared location.

Full threat model: [SECURITY.md](SECURITY.md).

## Status

**Available now**

- Desktop app with the 1.0.1 interface: light and dark themes, grid results, infinite scroll, thumbnails (with a setting), hover details, and right-click actions
- Search by name, content, meaning, and typo-tolerant matches
- Documents, code, and images (with OCR), CJK-aware
- Incremental indexing that tracks renames and deletes
- Agent access over MCP and loopback HTTP, with one-click setup for Claude and Cursor
- Optional local model for query refinement and cited answers (`/ask`)
- `.gitignore` and `.cursorignore` respected by default

**Known limitations**

- Builds are not yet code-signed, and the auto-updater is not wired up yet.
- Native platform OCR is not implemented. Tesseract and the optional PP-OCRv6 pack are the engines.
- Thumbnails cover image files only. Documents and code show type icons.
- Visual search is a beta model download and is not part of search results yet.

## Contributing

- [Contributing guide](docs/CONTRIBUTING.md)
- [Security policy](SECURITY.md): please report vulnerabilities privately
- [Issues](https://github.com/yjid/kensaku/issues)
- [MIT license](LICENSE)

Useful contributions: bug reports with steps to reproduce, testing on each platform's install path, search-quality improvements, agent integrations, and documentation.
