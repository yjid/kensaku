<p align="center">
  <img src="assets/icon.png" width="120" height="120" alt="Kensaku" />
</p>

<h1 align="center">Kensaku <span title="kensaku = search, in Japanese">検索</span></h1>

<p align="center">
  <b>Your computer's search is broken. This one isn't.</b><br/>
  <i>Filenames, file contents, PDFs, Office docs and image text — one local index, millisecond answers, zero cloud.</i><br/>
  <i>For you, and for every agent you use.</i>
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

---

## The 10-second pitch

You wrote a file two weeks ago. You need it *now*.

```
┌──────────────────────────────────────────────────────────────┐
│  ⌕  screenshot of docker error                          esc  │
├──────────────────────────────────────────────────────────────┤
│  All 12   Files   Images 4   Docs   Code   Media   Recent    │
├──────────────────────────────────────────────────────────────┤
│  ▣  docker-build-error.png         screenshot · 482 KB       │
│      Error: failed to solve: process "/bin/sh -c cargo ..."  │
│  ▣  compose-notes.md               content · edited 2h ago   │
│  ▣  deploy.log                     content · 1.2 MB          │
└──────────────────────────────────────────────────────────────┘
   3.2 ms · indexed 3 seconds ago · semantic ✓
```

No cloud. No account. No "we're still searching…". Just your machine, indexed
once, answered instantly.

---

## What's inside

| | |
|---|---|
| 🔍 **Search everything** | Filenames, code, configs, logs, Markdown — as you type. |
| 📄 **Documents** | PDF, DOCX/PPTX/XLSX, EPUB/ODT, RTF, with encoding detection. |
| 🖼️ **Images** | OCR for screenshots and photos, thumbnails, perceptual near-duplicates. |
| ⚡ **Fast by construction** | SQLite snapshot + Tantivy BM25 + static vectors, fused with RRF. |
| 🧠 **Semantic, quietly** | 256-d static embeddings bundled; no GPU, no ONNX at query time. |
| 🤖 **Agent-ready** | MCP server + CLI over the same read-only index. |
| 🎨 **Two themes** | `paper` and `void`. Hairlines, no gradients, no noise. |
| 🔒 **Local by default** | No telemetry. The only network call is an optional model download. |

<details>
<summary><b>Query language (click to expand)</b></summary>

Everything is optional. Plain typing always works; filters only narrow the set.

```text
type:pdf invoice                 PDFs only
in:~/Projects kensaku            scope to a folder
modified:<7d config              changed this week
size:>2mb is:image               large images
name:config content:timeout      field-scoped
-node_modules docker error       exclusion
"exact phrase"                   phrase
re:"fn\s+\w+"                    regex
mode:exhaustive                  every match, path-sorted
```

Filters compose. Unknown `key:value` is treated as text, never an empty result.

</details>

<details>
<summary><b>How ranking works (click to expand)</b></summary>

Three tiers run in parallel — fuzzy filename, BM25 content, and static-vector
semantic — then fuse with reciprocal rank fusion. On top: exact/prefix name
bonuses, smooth recency, frecency from files you actually open, and a bounded
depth penalty. Semantic-only hits below a cosine floor are dropped, so a keyword
query doesn't return vaguely related documents. Results are deterministic:
score, then path.

</details>

---

## Install

Grab the bundle for your platform from **[Releases](https://github.com/yjid/kensaku/releases)**:

| 🐧 Linux | 🍎 macOS | 🪟 Windows |
|---|---|---|
| `.AppImage` · `.deb` · `.rpm` | universal `.dmg` | `.msi` · `.exe` |

> **Unsigned builds.** macOS: right-click → **Open** the first time, or
> `xattr -dr com.apple.quarantine /Applications/Kensaku.app`. Windows: SmartScreen →
> **More info** → **Run anyway**. Details in [docs/INSTALL.md](docs/INSTALL.md).

Build it yourself:

```bash
git clone https://github.com/yjid/kensaku
cd kensaku
bun install
bun run tauri dev      # dev
bun run tauri build    # bundles for your OS
```

Rust (edition 2024), Bun/Node 18+, and the
[Tauri prerequisites](docs/INSTALL.md#build-prerequisites).

---

## Give it to your agents 🤖

Kensaku is a **read-only search backend** for AI agents. There is no LLM inside
the app — your agent brings its own.

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

```bash
kensaku-cli search "docker error" --json --limit 10
kensaku-cli search "type:pdf invoice" --path ~/Documents
kensaku-cli state --json
```

Tools: `search` (ranked or exhaustive, path/ext/lookback filters, 400-char
snippets), `read_snippet` (token-budgeted window with line numbers), and
`index_state` (counts, sizes, model ids, freshness). No writes, no shell, no
network. Full contract: [docs/AGENTS.md](docs/AGENTS.md).

---

## Numbers

100,000-file synthetic corpus, release build, warm cache
([methodology](docs/BENCHMARKS.md)):

|  |  |
|---|---|
| Filesystem walk | **~190 ms** (≈530k files/s) |
| Fuzzy filename query | **2.8 ms** avg · 0.4 ms best · 4.6 ms worst |
| Keystroke → first paint | **~8–18 ms** |
| Metadata | **128 B** per entry (~12 MB per 100k) |

1M-file figures are projected, not measured yet. Honest gaps are in
[docs/REMAINING.md](docs/REMAINING.md).

---

## Where this is going

The next level is not more query syntax. It is **less translation between a
thought and a file**:

1. **Understand** — "latest resume" is a newest-sort plus an alias, not two
   keywords. The app shows its interpretation as chips you can edit.
2. **Filter** — live facet counts, multi-select, exclude, and saved searches.
   Narrowing a million files should feel like toggling.
3. **See** — CLIP/SigLIP image search, similar images, diagrams, native OCR,
   and an optional local "this is me" identity. No image leaves the machine.
4. **Connect** — a warm daemon that serves the GUI, CLI, and any MCP-capable
   agent, plus an optional local `/ask` with path citations.

Plain language stays the default. `/` is for what you want, `@` is for where it
lives. The existing query grammar remains for power users and agents. Full
scope, acceptance tests, and open decisions: [docs/VISION.md](docs/VISION.md).

---

## Privacy

- No analytics, no telemetry, no accounts.
- Index lives in `~/.config/kensaku/` (or the platform equivalent).
- Hidden files and junk directories are skipped by default.
- **Settings → About → Reset everything** wipes the index, database, models, and
  config, then restarts.

---

## Docs

| | |
|---|---|
| [Vision & roadmap](docs/VISION.md) | where the product is going, interaction model, milestones |
| [Install & build](docs/INSTALL.md) | platforms, prerequisites, CLI, unsigned notes |
| [Usage](docs/USAGE.md) | query language, keys, settings, indexing behavior |
| [Agents](docs/AGENTS.md) | MCP tools and CLI contract |
| [Architecture](docs/ARCHITECTURE.md) | data model, index tiers, pipeline, budgets |
| [Benchmarks](docs/BENCHMARKS.md) | methodology and numbers |
| [Edge cases](docs/EDGE-CASES.md) | correctness catalogue for contributors |
| [Remaining work](docs/REMAINING.md) | known gaps, with rationale |
| [FAQ](docs/FAQ.md) | the questions people actually ask |
| [Security](SECURITY.md) | threat model and reporting |
| [Contributing](docs/CONTRIBUTING.md) | setup, checks, PR expectations |
| [Releasing](docs/RELEASING.md) | build artifacts and publish a version |

---

## Status

`v0.3.3` — usable daily. Working: durable incremental index, full query language,
hybrid ranking, documents + OCR, image thumbnails and near-duplicates, agent
CLI/MCP, two themes, Doctor, Stats for Nerds, factory reset, tests and CI.

Not yet: natural-language intent, `/` + `@` interaction, instant facet counts,
CLIP/SigLIP visual search, face identity, native platform OCR, a warm agent
daemon, a 1M-file prefilter, signing/notarization, auto-update, final artwork.
See [docs/REMAINING.md](docs/REMAINING.md).

**Where this is going:** a memory that understands a sentence, filters like a
toggle, sees your screenshots and photos, and plugs into any AI without
uploading your life. The plan is in [docs/VISION.md](docs/VISION.md).

## Contributing

Issues and PRs are welcome — read [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) and
make the checks pass:

```bash
cargo test --all-targets --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
npx tsc --noEmit && bun run build
```

## License

MIT © Yash Jangid — [LICENSE](LICENSE).

<p align="center"><sub>Built with Rust, Tantivy, Tauri 2, React and a stubborn dislike of slow search.</sub></p>
