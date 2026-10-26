# Install & build

## Prebuilt bundles

Download from [Releases](https://github.com/yjid/kensaku/releases).

| Platform | Asset | Notes |
|---|---|---|
| Linux | `kensaku_<ver>_amd64.AppImage` | chmod +x and run; no install needed |
| Linux | `kensaku_<ver>_amd64.deb` | `sudo apt install ./kensaku_*.deb` |
| Linux | `kensaku-<ver>-1.x86_64.rpm` | `sudo rpm -i kensaku-*.rpm` |
| macOS | `Kensaku_<ver>_universal.dmg` | Intel + Apple Silicon |
| Windows | `Kensaku_<ver>_x64_en-US.msi` | per-user install |
| Windows | `kensaku.exe` | portable |

### The builds are unsigned

This is the honest trade-off for a project without a paid signing identity yet.

**macOS.** Gatekeeper will block the first launch. Either right-click the app →
**Open** → **Open**, or clear the quarantine flag:

```bash
xattr -dr com.apple.quarantine /Applications/Kensaku.app
```

**Windows.** SmartScreen shows "Windows protected your PC". Choose **More info** →
**Run anyway**.

**Linux.** No signing step; AppImage may need `--no-sandbox` on hardened distros.

If your organization requires signed binaries, build from source or wait for
notarized releases (tracked in `REMAINING.md`).

---

## Build prerequisites

### All platforms

- Rust stable (edition 2024)
- [Bun](https://bun.sh) 1.1+ (Node 18+ also works)
- Git

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf libgtk-3-dev build-essential curl wget file
```

Fedora:

```bash
sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel \
  librsvg2-devel patchelf gtk3-devel
```

Arch:

```bash
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf gtk3 base-devel
```

### macOS

```bash
xcode-select --install
```

### Windows

- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with **Desktop development with C++**
- WebView2 runtime (preinstalled on Windows 10/11)

---

## Build

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install

bun run tauri dev        # dev app
bun run tauri build      # bundles into src-tauri/target/release/bundle/
```

Bundle outputs:

- Linux: `bundle/appimage/`, `bundle/deb/`, `bundle/rpm/`
- macOS: `bundle/dmg/`, `bundle/macos/`
- Windows: `bundle/msi/`, `bundle/nsis/`

The release workflow (`.github/workflows/release.yml`) builds all three platforms
on a `v*` tag and uploads a draft release.

---

## The agent CLI

`kensaku-cli` is a separate binary that shares the same index.

```bash
cargo build --release --manifest-path cli/Cargo.toml
```

Install it on your `PATH`:

```bash
# Linux / macOS
install -m 755 cli/target/release/kensaku-cli ~/.local/bin/kensaku-cli

# Windows (PowerShell)
Copy-Item cli\target\release\kensaku-cli.exe "$env:USERPROFILE\bin\"
```

Then point your MCP client at it:

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

If `kensaku-cli` is not on `PATH`, use the absolute path in `command`.

CLI binaries are published as **workflow artifacts** on the release run (the
desktop bundles are the release assets). Build from source, or download the
artifact from the Actions run.

---

## OCR (optional)

OCR needs the `tesseract` binary **and** language data. Kensaku can download
`eng.traineddata` (~4 MB) into its own directory from **Settings → Search**, so
you don't need root. If you prefer a system install:

```bash
# Linux (Debian/Ubuntu)
sudo apt install tesseract-ocr tesseract-ocr-eng

# macOS
brew install tesseract

# Windows
winget install UB-Mannheim.TesseractOCR
```

Settings → Search shows one of four states: ready (with languages), engine found
but no language data (download button), engine missing (platform install command),
or unknown. The OCR switch is disabled until OCR can actually work. Doctor repeats
the same check.

---

## First run

1. Pick folders (Home, Projects, Documents, Downloads).
2. Choose an exclusion preset (Developer / Creative / All-round).
3. Set preferences (theme, global shortcut).
4. Optionally download OCR language data and the semantic model.
5. Start. The first scan shows live counts and ends with a Done summary; later
   launches are search-first and instant. You can search while indexing runs.

The index lives at:

| OS | Path |
|---|---|
| Linux | `~/.config/kensaku/` |
| macOS | `~/Library/Application Support/kensaku/` (config via dirs) |
| Windows | `%APPDATA%\kensaku\` |

Contents: `config.json`, `metadata.db` (SQLite WAL), `index/` (Tantivy),
`thumbs/`, optional `models/`, `crash.log`.

---

## Uninstall / reset

- **Reset inside the app:** Settings → About → **Reset everything** (type `RESET`).
  Deletes the index, database, models, and config, then restarts.
- **Manual:** quit Kensaku, delete the config directory above.
- **Uninstall:** remove the app bundle/package; the data directory is separate.
