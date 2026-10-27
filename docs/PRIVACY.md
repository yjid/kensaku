# Privacy

Kensaku is local-first. Everything below describes the shipped behavior.

## What stays on your machine

- The index (metadata, full-text, embeddings) lives in your OS config directory.
- No account, no sign-in, no cloud search.
- No telemetry and no update checks: the app never phones home.
- Network access is used only when you explicitly download an optional model
  pack or OCR language data.

## What leaves the machine

- Nothing during normal search.
- Model/OCR downloads fetch files from their configured source when you click
  Download. Those requests go to the model host, not to Kensaku servers (there
  are none).

## MCP server

- The MCP server is read-only by default. It can read the local index and file
  metadata/contents that Kensaku has already indexed.
- It cannot delete or modify files. The only write-capable tool (`reindex`) is
  disabled unless `mcp_allow_reindex = true` in config.
- `agent_scoped_roots` can restrict MCP/CLI results to specific roots.
- The MCP server does not expose arbitrary filesystem access beyond the index.
