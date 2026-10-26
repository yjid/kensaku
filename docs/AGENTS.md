# Kensaku for AI agents

Kensaku exposes its index to external agents. It is **read-only by default,
local-only, and shares the same index as the desktop app** — no API keys, no
network. An opt-in `reindex` tool is the only write action.

Two surfaces, one binary:

- **MCP** over stdio — for Claude Code, Cursor, and any MCP client.
- **CLI** — for shell-capable agents and one-off queries.

Both use the existing `~/.config/kensaku` index. The MCP server pings the
daemon and forwards searches/state to it when available; with no daemon it
opens the index read-only and loads metadata/vectors once per session.

---

## MCP setup

Use the absolute path if `kensaku-cli` is not on your `PATH`.

```json
{
  "mcpServers": {
    "kensaku": { "command": "kensaku-cli", "args": ["mcp"] }
  }
}
```

Protocol: JSON-RPC 2.0, newline-delimited, over stdin/stdout. Only protocol
messages are written to stdout; logs go to stderr.

### Tools

#### `search`

```json
{
  "query": "docker compose error",
  "limit": 10,
  "mode": "ranked",
  "path": "~/Projects",
  "ext": "rs",
  "modified_after": "7d"
}
```

| field | type | notes |
|---|---|---|
| `query` | string, required | full query language: `type:`, `ext:`, `in:`, `modified:`, `size:`, `re:"..."`, `-negation` |
| `page_size` | 1–50, default 10 | results per page (`limit` remains a deprecated alias) |
| `cursor` | string | opaque `next_cursor` from the previous page |
| `mode` | `ranked` \| `exhaustive` | ranked for relevance; exhaustive for completeness (path-sorted, cap 10k) |
| `path` | string | becomes `in:"<path>"` |
| `ext` | string | becomes `ext:<ext>` |
| `modified_after` | string | lookback window, becomes `modified:<window>` (e.g. `7d`, `30d`, `1h`) |

Returns JSON in `content[0].text`:

```json
{
  "results": [
    { "path": "...", "score": 0.91, "snippet": "...", "match_type": "Both",
      "size": 1024, "modified": 1761400000 }
  ],
  "total": 12,
  "mode": "ranked",
  "returned": 10,
  "page_size": 10,
  "next_cursor": "k1-a",
  "has_more": true,
  "token_budget": 5000,
  "as_of": 1761400000
}
```

Snippets are capped at 100 characters and flagged with `snippet_truncated`.
`match_type` is `FileName`, `Content`, `ContentFolded`, `Semantic`, or `Both`.
Use `next_cursor` until `has_more` is false. Offsets are capped at 10,000; a page
at the cap reports `cursor_limit_reached: true` instead of materialising more.

#### `read_snippet`

```json
{ "path": "/abs/file.rs", "query": "timeout", "max_tokens": 256 }
```

Returns `{ path, line_start, line_end, text, truncated }`. Text is capped at
`max_tokens × 4` characters and centered on the first match. Never returns a
whole large file.

#### `health_check`

No arguments. Returns `{ ok, daemon, db, index, model, model_real,
protocol_version, watcher_ok, files, content_docs, semantic_units, phase,
is_indexing, paused, error, as_of }`. An unhealthy index returns `ok: false`
with a reason, not a tool error.

#### `index_state`

No arguments. Returns:

```json
{ "files": 1204553, "dirs": 84210, "content_docs": 186420, "semantic_units": 402118,
  "index_bytes": 759000000, "model_id": 2, "active_model_id": 2,
  "model_name": "potion-base-8M", "roots": ["/home/you"], "as_of": 1761400000 }
```

`model_id` is the model that produced the stored vectors (`null` if none);
`active_model_id` is what new queries use. If they differ, a re-embed is pending.
The payload also carries `phase`, `is_indexing`, `paused`, extraction progress,
`model_real` and `last_reconcile`.

#### `regex_search`

`{ "pattern": "fn \\w+", "page_size": 20 }`. Patterns are capped at 512 bytes,
compiled before execution, and return the same pagination/snippet shape as
`search`.

#### `reindex` (opt-in write)

No arguments. Hidden from `tools/list` and disabled by default. To enable it,
set `"mcp_allow_reindex": true` in config (Settings → Agents) and restart the MCP
client. The daemon re-checks the setting on every call; without a daemon the
tool is unavailable.

---

## CLI

```bash
kensaku-cli search "docker error" --json --limit 10
kensaku-cli search "type:pdf invoice" --path ~/Documents
kensaku-cli search 're:"fn \w+"' --ext rs --since 30d
kensaku-cli search "config" --exhaustive --json
kensaku-cli state --json
kensaku-cli mcp
```

Default output is `path<TAB>score<TAB>snippet`; `--json` prints the full
response. Exit codes: `0` ok, `2` usage, `3` no index (launch Kensaku once).

---

## Behavior and limits

- **Read-only by default.** No shell and no network. `reindex` is the only
  write tool and requires the explicit config opt-in plus the daemon.
- Honors the watched folders and exclusions configured in the app.
- Ranked mode fuses fuzzy filename, BM25 content, and static-vector tiers with
  RRF; semantic-only hits below a model-dependent cosine floor are dropped.
- `exhaustive` skips the ranked tiers and enumerates by path, capped at 10,000.
- No index → tools return a clear error; they never create one.
- Daemon-first sessions avoid opening SQLite, Tantivy or the embedding table in
  the MCP process. Without a daemon, startup cost is once per session; a
  one-shot CLI call pays metadata load, so prefer the MCP session.

---

## Planned

These are designed but not shipped. The contract above remains the truth today.

- **Loopback HTTP/SSE** transport with token auth for clients that need it.
- **`find_symbol`** (tree-sitter), **`find_related`**, and
  `.gitignore` / `.cursorignore` parity; pagination beyond the 10k result cap.
- **Visual search** once the vision tier lands: text-to-image, similar-image,
  and optional "this is me" lookups. Read-only and local like everything else.
- **One-click connect** for supported MCP clients, with live status in
  Settings → Agents.
- **Optional local LLM** as a query compiler and `/ask` with path citations.
  The model never retrieves on its own and is never required.

Scope and acceptance tests: [VISION.md](VISION.md). Technical shape:
[ARCHITECTURE.md](ARCHITECTURE.md) §18.
