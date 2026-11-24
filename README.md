<p align="center">
  <img src="assets/icon.png" width="112" height="112" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>Fast, light, and efficient. Find anything on your computer.</b>
</p>

<p align="center">
  Search 100,000 file names in milliseconds, with about 12 MB of metadata.<br/>
  Search inside documents, images, and code too. Nothing leaves your machine.
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
  <a href="#install">Install</a> ·
  <a href="#why-kensaku">Why Kensaku</a> ·
  <a href="#features">Features</a> ·
  <a href="#for-ai-agents">Agents</a> ·
  <a href="docs/README.md">Docs</a> ·
  <a href="#contributing">Contribute</a>
</p>

<p align="center">
  <img src="assets/hero.png" width="100%" alt="Kensaku — find anything on your computer" />
</p>

## Install

**Download for [Windows](https://github.com/yjid/kensaku/releases/latest) ·
[macOS](https://github.com/yjid/kensaku/releases/latest) ·
[Linux](https://github.com/yjid/kensaku/releases/latest)**

1. Download the installer for your computer.
2. Run it and open Kensaku.
3. Pick the folders you want searchable. Search works while the first scan runs.

| Your computer | Download |
|---|---|
| Windows 10/11 (Intel/AMD) | `kensaku_<version>_x64-setup.exe` |
| Windows on ARM (Snapdragon) | `kensaku_<version>_arm64-setup.exe` |
| macOS (Apple silicon or Intel) | `kensaku_<version>_universal.dmg` |
| Linux | `kensaku_<version>_amd64.AppImage` (or `.deb` / `.rpm`) |
| Portable / no installer | Windows `.zip`, Linux `.AppImage`, macOS `.dmg` |

**Windows:** per-user install, no admin prompt. The installer is Authenticode-signed. A brand-new release can still show a SmartScreen reputation warning; choose **More info → Run anyway** only if it came from this repository's Releases page.

**macOS:** builds are not notarized yet. If macOS blocks the first launch, open **System Settings → Privacy & Security → Open Anyway**. Folder-access prompts can return after an upgrade; see [Troubleshooting](docs/TROUBLESHOOTING.md#macos-keeps-asking-for-folder-access).

<details>
<summary><b>Terminal one-line install</b> (optional)</summary>

Linux and macOS:

```bash
curl -fsSL https://github.com/yjid/kensaku/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/yjid/kensaku/releases/latest/download/install.ps1 | iex"
```

The installers verify `SHA256SUMS`; Windows also verifies the Authenticode signature. Use `--system` on Linux for the `.deb`/`.rpm`, `-Silent` on Windows, or `--dry-run` / `-DryRun` to see what will happen.

</details>

Full steps, updates, and uninstall: [docs/INSTALL.md](docs/INSTALL.md).

## Why Kensaku

You don't remember the filename. You remember what was in it.

- The PDF about database indexing, saved as `notes-final-2.pdf`.
- The log line from the failed deploy, somewhere in a folder of logs.
- The screenshot with a CUDA error, which has no useful name at all.

Type what you remember. Kensaku searches names, text inside files, and meaning, and shows the match with the words highlighted. Open it with Enter.

| | Kensaku | Everything | Listary | PowerToys Run |
|---|---|---|---|---|
| Search inside PDF, Office, EPUB, email | ✅ | ❌ | ❌ | ❌ |
| Text inside screenshots and scans (OCR) | ✅ | ❌ | ❌ | ❌ |
| Search by meaning | ✅ | ❌ | ❌ | ❌ |
| Multilingual and CJK text | ✅ | ❌ | ❌ | ❌ |
| Your AI agents can search the same index | ✅ | ❌ | ❌ | ❌ |
| Free and open source (MIT) | ✅ | ❌ | ❌ | ✅ |

**Three things worth trying on day one:**

1. **Screenshot archaeology.** Search the error text you saw in a screenshot.
2. **Describe, don't recall.** “big videos from last month” is a valid query.
3. **Give your AI agent eyes.** Claude or Cursor can search this same local index.

<p align="center">
  <img src="assets/search-dashboard.png" width="100%" alt="Kensaku search results" />
</p>

## Features

**Finds more**
- Names, full text, fuzzy matches, and meaning in one search.
- PDF, DOCX/PPTX/XLSX, EPUB, ODT, RTF, CSV/TSV, email (`.eml`, `.msg`), and plain text.
- 50+ code and data formats: Rust, Python, TypeScript, Go, C/C++, SQL, JSON, YAML, TOML, Dockerfile, and more.
- Multilingual and CJK by default: Chinese, Japanese, and Korean, plus mixed-script boundaries.
- Images and OCR: PP-OCRv6 by default; optional Indra for deskew and multi-script recognition.
- A query language for power users: `type:pdf invoice`, `modified:7d`, `re:"fn \w+"`. [Full reference](docs/USAGE.md).

**Fast and calm**
- 100,000 filenames in milliseconds on a warm cache; about 12 MB of metadata. [Benchmarks and caveats](docs/BENCHMARKS.md).
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

## Docs

[Install](docs/INSTALL.md) ·
[Usage](docs/USAGE.md) ·
[FAQ](docs/FAQ.md) ·
[Troubleshooting](docs/TROUBLESHOOTING.md) ·
[Privacy](docs/PRIVACY.md) ·
[Agents](docs/AGENTS.md) ·
[Benchmarks](docs/BENCHMARKS.md) ·
[Architecture](docs/ARCHITECTURE.md) ·
[Contributing](docs/CONTRIBUTING.md) ·
[Security](SECURITY.md)

## Contributing

Kensaku is MIT-licensed and built by people who wanted a better launcher.

- **Use it and break it.** Bug reports with steps to reproduce are the most valuable contribution right now.
- **Pick a good first issue** in the [issue tracker](https://github.com/yjid/kensaku/issues).
- **Improve search quality** — ranking, typo tolerance, and new extractors.
- **Docs, translations, and screenshots** — [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).
- **Security** — report privately via [SECURITY.md](SECURITY.md).

If Kensaku saves you time, [star the repository](https://github.com/yjid/kensaku/stargazers). It is how the next contributor finds us.

## License

MIT. See [LICENSE](LICENSE).
