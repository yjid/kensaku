# Usage

> This page documents **shipped behavior** at `v0.3.3`. The planned interaction
> — plain-language intent, `/` commands, `@` scopes, facet counts, visual search
> — is described in [VISION.md](VISION.md). Do not assume it exists yet.

## The window

Press <kbd>Alt</kbd>+<kbd>Space</kbd> (configurable in Settings) to toggle the
floating search window. It hides on close and keeps the index warm.

The main page is intentionally just **search + category chips + files**. Every
statistic and setting lives in Settings, so the search surface stays quiet.

- **Search bar** — type anything. Plain words search names and content.
- **Category chips** — All, Files, Folders, Images, Docs, Code, Media, Recent.
  They map to `type:` / `modified:` filters.
- **Results** — 56px rows or 160px cards. Selected row has an accent rail.
- **Preview** — appears when a result is selected and a query is active.
- **Status** — indexing progress is a thin line under the bar; no numbers on the
  main page.

## Keyboard

| Key | Action |
|---|---|
| <kbd>Alt</kbd>+<kbd>Space</kbd> | show/hide the window |
| <kbd>/</kbd> | focus search |
| <kbd>↑</kbd> <kbd>↓</kbd> | move selection (grid: by row) |
| <kbd>←</kbd> <kbd>→</kbd> | move within a grid row |
| <kbd>Enter</kbd> | open |
| <kbd>Shift</kbd>+<kbd>Enter</kbd> | reveal in folder |
| <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>Enter</kbd> | open (line-aware when available) |
| <kbd>Tab</kbd> | toggle preview |
| <kbd>/</kbd> | open the command menu (type `/`) |
| <kbd>⌘,</kbd>/<kbd>Ctrl</kbd>+<kbd>,</kbd> | settings |
| <kbd>Esc</kbd> | clear, then dismiss |

## Query language

Everything is optional. Filters never replace the text; they narrow the set.

### Filters

| Syntax | Meaning |
|---|---|
| `type:pdf` | category: `image`, `doc`, `sheet`, `presentation`, `code`, `video`, `audio`, `archive`, `dir`, or an extension |
| `ext:rs` | exact extension |
| `in:~/Projects` | scope to a folder (quote paths with spaces) |
| `path:src-tauri` | substring match on the path |
| `modified:<7d` | changed within a window: `1h`, `7d`, `30d`, `7d..30d` |
| `size:>2mb` | size range: `>`, `<`, or `a..b` (`kb`, `mb`, `gb`) |
| `is:image`, `is:dir` | aliases for common categories |
| `lang:rust` | language family of the extension |
| `name:config` | match the filename only |
| `content:timeout` | match file contents only |
| `re:"fn \\w+"` | regular expression over content + filename |
| `-node_modules` | exclude term or folder |
| `"exact phrase"` | phrase |
| `mode:exhaustive` | return every match, path-sorted (no ranked tiers) |

Examples:

```
type:pdf invoice
in:~/Projects kensaku indexer
modified:<7d config
size:>5mb is:image sunset
name:Cargo content:workspace
-node_modules docker error
re:"TODO|FIXME" ext:rs
```

### Plain language

Kensaku compiles common phrasing locally, without a model. Every inference is
shown as a removable chip above the results; removing a chip removes the words
that produced it. If the query already contains an explicit `filter:`, the
compiler leaves it alone.

| You type | It becomes |
|---|---|
| `latest resume` | `resume` + sort newest |
| `screenshots from last week` | `type:image modified:<7d` |
| `big videos in Projects` | `size:>100mb type:media in:Projects` |
| `tiny images` | `size:<1mb type:image` |
| `oldest invoices` | `invoices` + sort oldest |
| `biggest files` | `files` + sort largest |

Time words: `today`, `yesterday`, `last week`, `last month`, `last year`.
Size words: `big`/`large`/`huge` (>100 MB), `small`/`tiny` (<1 MB), and
`under 5 MB` / `over 10 MB`.

### Scopes and sort

- Type `@` to pick a folder from the index; the choice inserts `in:"<path>"`.
- Type `/` for the command menu (filters, `/dupes`, `/reindex`, `/doctor`, `/settings`).
- When results are showing, the **Sort** control offers `Best match`, `Newest`,
  `Oldest`, `Largest`, and `Name`. Sorting is applied server-side before the
  result limit; `Best match` keeps the fused relevance order.

### How ranking works

Three tiers run in parallel — fuzzy filename, BM25 content, and static-vector
semantic — then fuse with reciprocal rank fusion. On top of that: exact/prefix
name bonuses, smooth recency, frecency from files you open, and a bounded depth
penalty. Semantic-only hits below a cosine floor are dropped, so a keyword query
doesn't return vaguely related documents.

Results are deterministic: score, then path.

## Settings

> **Setup (shipped):** the guided first run covers watched folders, exclusions,
> preferences (theme + shortcut), optional OCR/model downloads, and the first
> scan, ending with a Done summary before the main window opens. Settings shows
> current values and offers **About → Run setup again**; model/OCR downloads are
> acquired in setup, and Settings shows installed status only. Later launches
> stay search-first.

- **General** — theme (`paper` / `void`), global shortcut, max results, start at login.
- **Folders** — watched roots, add/remove, drag-and-drop.
- **Exclusions** — presets (Developer, Creative, All-round), directories, extensions.
- **Search** — semantic toggle, OCR, filter cheat sheet.
- **AI & Semantic** — active model, embed queue, re-embed, optional model download.
- **Agents** — MCP config and CLI examples for external agents.
- **Doctor** — nine checks: config writability, DB integrity, index size, semantic
  units, watched roots, watcher health, inotify limit, model files, crash log.
- **Stats for Nerds** — p50/p95/p99 + sparkline, index composition and on-disk
  sizes, ingestion phase/files-per-second/queue depth, uptime, RSS, model ids.
- **About** — version, thesis, and **Reset everything**.

## Indexing behavior

- **Hidden entries** are skipped by default, along with common junk directories.
- **Sensitive paths and credentials** are default-denied: `.ssh`, `.gnupg`, `.aws`,
  `.kube`, `.docker`, `.password-store`, password-manager folders, `.env*`, and
  credential extensions (`.pem`, `.key`, `.pfx`, …).
- **Incremental** — unchanged files (same size + mtime) are not re-read.
- **Renames** are tracked by `(device, inode)`, so moves do not duplicate entries.
- **Startup** is search-first: the snapshot loads, then a stat-only reconcile runs
  in the background. This applies **after the first completed scan**; the guided
  first run waits for the initial scan and ends with a Done screen. Until it
  finishes the app reports how old the index is.
- **Unavailable roots** (unplugged drive, unmounted share) are preserved, not pruned.
- **Documents** — PDF, DOCX/PPTX/XLSX, EPUB/ODT, RTF; malformed files are skipped
  after an 8-second timeout.
- **Images** — OCR via `tesseract` when the engine and language data are present.
  The setup flow offers `eng.traineddata` as an optional pack; Settings → Search
  shows status and links to **Run setup again**. The toggle controls whether image
  text is indexed, and saving re-scans existing images. Thumbnails and dimensions
  are cached; near-duplicates use a perceptual hash.
- **Cloud placeholders** (Windows OneDrive/iCloud) are indexed by name only, so
  reading doesn't hydrate the file.

## Agents

See [`AGENTS.md`](AGENTS.md). The short version:

```bash
kensaku-cli search "docker error" --json --limit 10
kensaku-cli state --json
kensaku-cli mcp
```

## Commands

Type `/` in the search box to open the command menu, then filter and press Enter:

| Command | Action |
|---|---|
| `/images` | Filter to images |
| `/pdf` | Filter to PDF files |
| `/docs` | Filter to documents |
| `/code` | Filter to source and config files |
| `/media` | Filter to audio and video |
| `/recent` | Files changed in the last 7 days |
| `/errors` | Search for error text |
| `/diagrams` | Search for diagrams |
| `/dupes` | Open the duplicate review view |
| `/reindex` | Re-scan watched folders |
| `/doctor` | Print an index health summary |
| `/settings` | Open Settings |
