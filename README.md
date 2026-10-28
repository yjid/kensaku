<p align="center">
  <img src="assets/icon.png" width="112" height="112" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>You can't Ctrl+F a screenshot. Now you can.</b>
</p>

<p align="center">
  <a href="https://github.com/yjid/kensaku/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/yjid/kensaku?style=for-the-badge" /></a>
  <a href="https://github.com/yjid/kensaku/releases/latest"><img alt="Downloads" src="https://img.shields.io/github/downloads/yjid/kensaku/total?style=for-the-badge" /></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-111111?style=for-the-badge" /></a>
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#what-it-does">What it does</a> ·
  <a href="#contributing">Contribute</a>
</p>

<p align="center">
  <img src="assets/hero.png" width="100%" alt="Kensaku" />
</p>

---

Kensaku (検索 = search) is a file search that reads inside your files. PDFs, code, logs, screenshots. All local. No cloud. No account. No telemetry. Free. MIT. Don't like it? Fork it.

You can't remember filenames. Nobody can. You remember what was *in* the file. So why the fuck does every search tool only search names?

Kensaku searches names, full text, and meaning. It reads inside screenshots. It parses "screenshots from last week" as a query. It doesn't send anything anywhere. It doesn't ask you to log in. It doesn't care about your data. It just finds your shit.

## What it does

- Reads text inside screenshots (OCR, built in, no extra setup)
- Finds files by meaning — "the PDF about database indexing" finds `notes-final-2.pdf`
- Parses "screenshots from last week" as a query. No filters to click.
- Your AI agent can search it (MCP, read-only)
- 1.3ms name search across 100k files. 9MB installer. ~20MB idle in tray.
- Alt+Space to summon. Type. Enter. Done.

## Install

Download from the [latest release](https://github.com/yjid/kensaku/releases/latest).

| Platform | Download |
|---|---|
| Windows | `Kensaku_<version>_x64-setup.exe` or `.msi` |
| macOS | `Kensaku_<version>_universal.dmg` |
| Linux | `.AppImage`, `.deb`, or `.rpm` |

Unsigned builds. Windows → SmartScreen → More info → Run anyway. macOS → System Settings → Privacy & Security → Open Anyway. [docs/INSTALL.md](docs/INSTALL.md).

First run: pick folders to index. Search works while it scans.

```bash
kensaku-cli search "docker error"
kensaku-cli search "invoice" --ext pdf --since 30d
kensaku-cli search "resume" --json
```

### Build from source

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri build
```

Rust, Bun, webview libs. `make dev`, `make build`, `make test`.

## Search

Names, full text, fuzzy, meaning — one search. PDF, DOCX, PPTX, XLSX, EPUB, ODT, RTF, text, code, logs, CJK. OCR for images.

```
type:pdf invoice          ext:md in:"~/Notes"       modified:7d
size:>10mb                path:"client/2026"        lang:rust
name:resume               content:"timeout"         re:"fn \w+_test"
-draft                    mode:exhaustive
```

Filters: Best match, Newest, Oldest, Largest, Name. Time travel: "last Tuesday", "3 days ago", "today", "yesterday".

## Performance

| What | Result |
|---|---|
| Name search, 100k files | 1.3 ms |
| Re-index, nothing changed | 0.56 s |
| Memory, 100k file names | ~12 MB |
| Windows installer | 9 MB |
| Idle in tray | ~20 MB |

Linux numbers. Windows/macOS not separately measured yet. [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Privacy

No telemetry. No account. No cloud search. Index stays in your OS config dir. You pick what's indexed. Hidden files and build folders skipped by default. `.pem`, `.key`, `.pfx` excluded. Credential files never indexed. Nothing leaves your machine. Period.

## MCP (AI agents)

Point an MCP client at the CLI:

```json
{ "mcpServers": { "kensaku": { "command": "kensaku-cli", "args": ["mcp"] } } }
```

Read-only by default. [docs/AGENTS.md](docs/AGENTS.md).

## Limitations (I know, I know)

- Unsigned builds. Auto-updater not wired (button in Settings → About opens the release page).
- OCR: Tesseract (default) + optional PP-OCRv6 pack.
- Thumbnails: images only.
- Visual search: beta, not in results yet.
- Windows is the most-tested platform. macOS/Linux come from CI.

## Contributing

MIT. Bug reports with steps to reproduce are the most useful thing right now. Good first issues labeled. Extractors and agent tooling are where PRs land fastest. [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md). `make dev`, `make build`, `make test` (240+ Rust tests on every push).

Feature ideas go in [Discussions](https://github.com/yjid/kensaku/discussions). Loudest real use case gets the next slot.

<p align="center">
  <img src="assets/search-dashboard.png" width="100%" alt="Kensaku search results" />
</p>