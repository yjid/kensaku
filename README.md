<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">æ¤œç´¢</span></h1>

<p align="center">
  <b>Find anything on your computer. Privately.</b><br/>
  File names, the text inside documents, screenshots, and meaning â€” all offline.
</p>

<p align="center">
  <a href="https://upstar.dev/r/yjid/kensaku"><picture><source media="(prefers-color-scheme: dark)" srcset="https://upstar.dev/badge/yjid/kensaku/day/windows-dark.svg"><img alt="Upstar: #1 Riser Of The Day in Windows, October 29, 2026" src="https://upstar.dev/badge/yjid/kensaku/day/windows.svg" width="250" height="55"></picture></a>
</p>

<p align="center">
  <a href="https://github.com/yjid/kensaku/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/yjid/kensaku?style=for-the-badge" /></a>
  <a href="https://github.com/yjid/kensaku/releases/latest"><img alt="Downloads" src="https://img.shields.io/github/downloads/yjid/kensaku/total?style=for-the-badge" /></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-111111?style=for-the-badge" /></a>
  <a href="https://github.com/yjid/kensaku/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/yjid/kensaku/ci.yml?branch=master&style=for-the-badge" /></a>
</p>

<p align="center">
  <img src="assets/hero.png" width="100%" alt="Kensaku â€” find anything on your computer" />
</p>

## Install

Download the latest release for **[Windows](https://github.com/yjid/kensaku/releases/latest)**, **[macOS](https://github.com/yjid/kensaku/releases/latest)**, or **[Linux](https://github.com/yjid/kensaku/releases/latest)**. Or install from a terminal:

```bash
# Linux / macOS
curl -fsSL https://github.com/yjid/kensaku/releases/latest/download/install.sh | sh
```

```powershell
# Windows PowerShell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/yjid/kensaku/releases/latest/download/install.ps1 | iex"
```

1. Download the installer for your computer.
2. Run it and open Kensaku.
3. Pick the folders you want searchable. Search works while the first scan runs.

<details>
<summary><b>Not sure which file to download?</b></summary>

| Your computer | Download |
|---|---|
| Windows 10/11 (Intel/AMD) | `kensaku_<version>_x64-setup.exe` |
| Windows on ARM (Snapdragon) | `kensaku_<version>_arm64-setup.exe` |
| macOS (Apple silicon or Intel) | `kensaku_<version>_universal.dmg` |
| Linux | `kensaku_<version>_amd64.AppImage` (or `.deb` / `.rpm`) |
| Portable / no installer | Windows `.zip`, Linux `.AppImage`, macOS `.dmg` |

</details>

**Windows:** per-user install, no admin prompt. The installer is Authenticode-signed. A brand-new release can still show a SmartScreen reputation warning; choose **More info â†’ Run anyway** only if it came from this repository's Releases page.

**macOS:** builds are not notarized yet. If macOS blocks the first launch, open **System Settings â†’ Privacy & Security â†’ Open Anyway**. Folder-access prompts can return after an upgrade; see [Troubleshooting](docs/TROUBLESHOOTING.md#macos-keeps-asking-for-folder-access).

Full steps, options, updates, and uninstall: [docs/INSTALL.md](docs/INSTALL.md).

## Why Kensaku

You don't remember the filename. You remember what was in it.

- The PDF about database indexing, saved as `notes-final-2.pdf`.
- The log line from the failed deploy, somewhere in a folder of logs.
- The screenshot with a CUDA error, which has no useful name at all.

Type what you remember. Kensaku searches names, text inside files, and meaning, and highlights the match. Open it with Enter.

| | Kensaku | Everything | Listary | PowerToys Run |
|---|---|---|---|---|
| Search inside PDF, Office, EPUB, email | âœ… | âŒ | âŒ | âŒ |
| Text inside screenshots and scans (OCR) | âœ… | âŒ | âŒ | âŒ |
| Search by meaning | âœ… | âŒ | âŒ | âŒ |
| Multilingual and CJK text | âœ… | âŒ | âŒ | âŒ |
| Your AI agents can search the same index | âœ… | âŒ | âŒ | âŒ |
| Free and open source (MIT) | âœ… | âŒ | âŒ | âœ… |

**Three things worth trying on day one:**

1. **Screenshot archaeology.** Search the error text you saw in a screenshot.
2. **Describe, don't recall.** â€œbig videos from last monthâ€ is a valid query.
3. **Give your AI agent eyes.** Claude or Cursor can search this same local index.

<p align="center">
  <img src="assets/search-dashboard.png" width="100%" alt="Kensaku search results" />
</p>

## Features

**Finds more**
- Names, full text, fuzzy matches, and meaning in one search.
- PDF, DOCX/PPTX/XLSX, EPUB, ODT, RTF, CSV/TSV, email (`.eml`, `.msg`), and plain text.
- 50+ code and data formats, including Rust, Python, TypeScript, Go, C/C++, SQL, JSON, YAML, and TOML.
- Multilingual and CJK by default: Chinese, Japanese, and Korean, plus mixed-script boundaries.
- Images and OCR: PP-OCRv6 by default; optional Indra for deskew and multi-script recognition.
- A query language for power users: `type:pdf invoice`, `modified:7d`, `re:"fn \w+"`. [Reference](docs/USAGE.md).

**Fast and calm**
- 100,000 filenames in milliseconds on a warm cache; about 12 MB of metadata. [Benchmarks](docs/BENCHMARKS.md).
- Results update as you type. Changed files are picked up incrementally.
- Closing the window keeps the index warm; reopening is instant.
- Launcher or desktop-app mode, keyboard navigation, space previews, grid or list.

**Private**
- No account, no telemetry, no cloud search. Your index stays on your machine.
- The agent surface is read-only by default; `reindex` is opt-in.
- Hidden files, build folders, and credential files are excluded by default.

**Good to know**
- OCR packs download once (PP-OCRv6 is ~6.1 MB; Indra is ~27 MB). Intel Macs fall back to Tesseract.
- Visual search is a beta model download and is not part of search results yet.
- Windows is the most tested platform; macOS and Linux reports are especially valuable.

## Performance

| What | Result |
|---|---|
| Name search, 100k files (Windows 11, default threads) | **18 ms average** (best 8.3 ms) |
| Name search, 100k files (Windows 11, 4 threads) | **3.1 ms average** |
| Name search, 100k files (Linux reference, warm) | **1.3 ms average** (best 0.5 ms) |
| Metadata for 100k files | **12.2 MB** |
| First run over 101k mixed files (earlier Linux reference) | 184 s, peak 183 MB |
| Re-index with nothing changed | 0.56 s |

The Windows and Linux rows are the same workload on different hardware; NTFS metadata reads and antivirus make the walk slower on Windows. Method, corpus, and caveats: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## For AI agents

`kensaku-cli` ships with each release and shares the same local index:

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

Read-only by default, with scoped roots, cursor pagination, and tree-sitter symbol lookup. Full guide: [docs/AGENTS.md](docs/AGENTS.md).

## Build from source

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri build
```

You need Rust stable, Bun 1.1+, and your platform's webview libraries. Full prerequisites and build notes: [docs/INSTALL.md](docs/INSTALL.md). A `Makefile` wraps `make dev`, `make build`, `make test`, and `make reset`.

## Documentation

Start with [Install](docs/INSTALL.md), [Usage](docs/USAGE.md), [FAQ](docs/FAQ.md), or [Troubleshooting](docs/TROUBLESHOOTING.md). For agents, see [AGENTS](docs/AGENTS.md); for performance and internals, see [BENCHMARKS](docs/BENCHMARKS.md) and [ARCHITECTURE](docs/ARCHITECTURE.md). Contributing and security have their own pages: [CONTRIBUTING](docs/CONTRIBUTING.md), [SECURITY](SECURITY.md).

## Contributing

Kensaku is MIT-licensed and built by people who wanted a better launcher.

- **Use it and break it.** Bug reports with steps to reproduce are the most valuable contribution right now.
- **Pick a good first issue** in the [issue tracker](https://github.com/yjid/kensaku/issues).
- **Improve search quality** â€” ranking, typo tolerance, and new extractors.
- **Docs, translations, and screenshots** â€” [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).
- **Security** â€” report privately via [SECURITY.md](SECURITY.md).

If Kensaku saves you time, [star the repository](https://github.com/yjid/kensaku/stargazers). It is how the next contributor finds us.

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=yjid/kensaku&type=Date)](https://star-history.com/#yjid/kensaku&Date)

## License

MIT. See [LICENSE](LICENSE).

