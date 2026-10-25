# Kensaku for AI agents

Kensaku exposes its index to external agents. It is **read-only, local-only, and
shares the same index as the desktop app** — no daemon, no API keys, no network.

Two surfaces, one binary:

- **MCP** over stdio — for Claude Code, Cursor, and any MCP client.
- **CLI** — for shell-capable agents and one-off queries.

Both open the existing `~/.config/kensaku` index. The MCP process loads metadata
and vectors **once per session**; per-query cost is the same as in the app.

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
| `limit` | 1–50, default 10 | |
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
  "as_of": 1761400000
}
```

Snippets are capped at 400 characters. `match_type` is `FileName`, `Content`,
`Semantic`, or `Both`.

#### `read_snippet`

```json
{ "path": "/abs/file.rs", "query": "timeout", "max_tokens": 256 }
```

Returns `{ path, line_start, line_end, text, truncated }`. Text is capped at
`max_tokens × 4` characters and centered on the first match. Never returns a
whole large file.

#### `index_state`

No arguments. Returns:

```json
{ "files": 1204553, "dirs": 84210, "content_docs": 186420, "semantic_units": 402118,
  "index_bytes": 759000000, "model_id": 2, "active_model_id": 2,
  "model_name": "potion-base-8M", "roots": ["/home/you"], "as_of": 1761400000 }
```

`model_id` is the model that produced the stored vectors (`null` if none);
`active_model_id` is what new queries use. If they differ, a re-embed is pending.

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

- **Read-only.** No writes, no shell, no network.
- Honors the watched folders and exclusions configured in the app.
- Ranked mode fuses fuzzy filename, BM25 content, and static-vector tiers with
  RRF; semantic-only hits below a model-dependent cosine floor are dropped.
- `exhaustive` skips the ranked tiers and enumerates by path, capped at 10,000.
- No index → tools return a clear error; they never create or modify one.
- Startup cost is once per MCP session. A one-shot CLI call pays metadata load,
  so prefer the MCP session for many queries.
