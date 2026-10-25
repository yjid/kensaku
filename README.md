# Kensaku (検索)

**A warm, local search index for your machine — and for your AI agents.**

Kensaku indexes filenames, file contents, documents, and image text once, keeps the
index fresh with a filesystem watcher, and answers in milliseconds. It is a desktop
app for you, and a read-only search backend for agents (MCP + CLI) on the same index.

- **Local only.** No cloud, no accounts, no telemetry.
- **Hybrid ranking.** Fuzzy filename + BM25 content + static-vector semantic, fused
  with reciprocal rank fusion.
- **Documents included.** PDF, DOCX/PPTX/XLSX, EPUB/ODT, RTF, plus OCR for images.
- **Agent-ready.** `kensaku-cli search …` and an MCP stdio server with `search`,
  `read_snippet`, and `index_state`.
- **Two themes.** `paper` and `void`, both designed to stay out of the way.

---

## What it does

| | |
|---|---|
| **Find files** | Type a name; results stream as you type. |
| **Find content** | `docker compose error`, `type:pdf invoice`, `modified:<7d`, `re:"fn \w+"`. |
| **Search documents** | PDF page-level text, Office/EPUB extraction, encoding detection. |
| **Search images** | OCR text from screenshots and photos; thumbnails in results. |
| **Keep it fresh** | Native file watcher, incremental crawls, rename tracking by inode. |
| **Stay honest** | Doctor (9 checks), Stats for Nerds, visible index freshness. |
| **Give it to agents** | MCP server + CLI over the same read-only index. |

### Query language

```
type:pdf ext:rs in:~/Projects path:src-tauri
modified:<7d size:>2mb is:image lang:rust
name:config content:timeout -node_modules
"exact phrase"   re:"fn\s+\w+"   mode:exhaustive
```

Plain typing always works; filters only narrow the set.

---

## Agent usage

MCP (Claude Code, Cursor, any MCP client):

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

CLI:

```bash
kensaku-cli search "docker error" --json --limit 10
kensaku-cli search "type:pdf invoice" --path ~/Documents
kensaku-cli state --json
```

Tools: `search` (ranked or exhaustive, path/ext/lookback filters, 400-char snippets),
`read_snippet` (token-budgeted window with line numbers), `index_state`
(counts, sizes, model ids, freshest-from timestamp). Read-only, no shell, no network.

Full contract: [`docs/AGENTS.md`](docs/AGENTS.md).

---

## Performance

Measured on a 100,000-file synthetic corpus, release build, warm cache:

| Operation | Result |
|---|---|
| Filesystem walk | ~190 ms (≈530k files/s) |
| Fuzzy filename query | avg **2.8 ms** (best 0.4 ms, worst 4.6 ms) |
| Memory | 128 B per entry (~12 MB per 100k) |
| Query path | ~8–18 ms keystroke → first paint |

1M-file figures are projected, not yet measured; see
[`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) and [`docs/REMAINING.md`](docs/REMAINING.md).

---

## Install & build

Prerequisites: Rust (edition 2024), Bun (or Node), and the Tauri system dependencies
for your OS (`webkit2gtk-4.1`, `libappindicator`, `librsvg`, `patchelf` on Linux).

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install

# desktop app (dev)
bun run tauri dev

# desktop app (release bundles)
bun run tauri build

# agent CLI
cargo build --release --manifest-path src-tauri/Cargo.toml --bin kensaku-cli
```

Put `kensaku-cli` on your `PATH` (or reference its absolute path in the MCP config).

First launch asks for folders and builds the index. Everything after that is incremental.

---

## How it works

```
filesystem → watcher + crawl ──► metadata (SQLite WAL) ──► in-memory snapshot
                              └─► content (Tantivy BM25)
                              └─► embeddings (static table, 256-d i8)

query ──► parser/AST ──► scope + filters ──► fuzzy ∥ BM25 ∥ vector ──► RRF ──► rank
                                                                        └─► snippets
```

- **Identity** is `(device, inode)`, so renames don't duplicate or lose entries.
- **Startup** is search-first: load the snapshot, serve immediately, reconcile in the
  background with a stat-only walk.
- **Freshness** is explicit: results carry an `as_of` generation.
- **Embeddings** use a bundled static estimator by default. The optional
  potion-base-8M table can be downloaded in Settings and is used when present;
  a model-id change triggers a resumable re-embed.
- **Agents** open the same index read-only (no Tantivy writer lock), so the CLI and MCP
  work while the app is running.

More detail: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Privacy

- No analytics, no telemetry, no network requests except the optional model download.
- The index lives in `~/.config/kensaku/` (metadata DB, Tantivy index, thumbnails,
  optional model files).
- Hidden files and common junk directories are skipped by default.
- Settings → About → **Reset everything** deletes the index, database, models, and
  configuration, then restarts.

---

## Status

`v0.2.0`. Working: durable incremental index, query language, hybrid ranking,
documents + OCR, image thumbnails, agent CLI/MCP, two themes, Doctor, Stats for Nerds,
factory reset, CI.

Known gaps and the roadmap: [`docs/REMAINING.md`](docs/REMAINING.md). The biggest ones
are ONNX/CLIP embeddings, native platform OCR, a 1M-file prefilter, and signing /
auto-update for release builds.

---

## Contributing

Issues and PRs are welcome. Please run `cargo test --all-targets` and
`cargo clippy --all-targets -- -D warnings` before opening a PR; CI runs both plus
`tsc` and the frontend build.

## License

MIT © Yash Jangid
