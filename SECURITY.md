# Security

## Reporting

Please **do not** open a public issue for a vulnerability. Use GitHub's private
vulnerability reporting (Security → Advisories → Report a vulnerability) or email
the maintainer listed on the repository profile. You will get an acknowledgement
within a few days.

## Threat model

Kensaku is a local desktop app and a read-only search backend. It is designed to
be safe when pointed at untrusted files.

**In scope**

- Parsing untrusted documents and images (PDF, DOCX/PPTX/XLSX, EPUB, RTF, images)
  without code execution, unbounded memory, or hangs.
- Path traversal or symlink tricks escaping watched roots.
- Command injection through file paths or file contents.
- XSS in the UI from indexed text, filenames, or metadata.
- The agent surface leaking data or permitting writes.
- Secrets ending up in the index or in logs.

**Out of scope**

- An attacker who can already read/write your user account or the index directory.
- Malicious OS-level components (kernel, filesystem, WebView).
- Denial of service by indexing an enormous corpus on purpose.

## Controls

- **Extraction isolation.** Parsers run on a bounded worker pool with a per-file
  timeout and panic isolation. XML external entities/DTDs are not resolved.
- **Decode caps.** Images are limited to 50 MP; text extraction is capped at
  100 MB per file and 4 MB of indexed text, with 48 semantic units per document.
  Dense images and scanned PDFs get up to 45 s; other extraction timeouts scale
  with file size.
- **No shell.** Files are opened through OS APIs; the agent surface has no
  shell or network tools. The only write action is `reindex`, and it is opt-in.
- **Read-only agent access.** `kensaku-cli` and the MCP server open the index
  without the Tantivy writer lock and cannot mutate it.
- **UI escaping.** Indexed content (filenames, snippets, previews) is rendered as
  text, never as HTML or `dangerouslySetInnerHTML`. The webview ships with a
  restrictive Content-Security-Policy (`script-src 'self'`, no remote origins)
  and the app loads no remote content.
- **Default exclusions** skip hidden files and common junk directories.
- **Local only.** No telemetry and no data about your files or searches leaves
  the machine. Outbound requests are limited to a periodic GitHub release check,
  user-initiated update downloads, and model/OCR pack downloads. The default
  text model downloads after setup; OCR data for the enabled engine may
  download automatically while OCR is on; optional Indra, smart-embedding, and
  visual packs are user-initiated.

## Sensitive data

The index contains filenames and extracted text from your watched folders. Treat
`~/.config/kensaku/` as sensitive: it is created with user-only permissions where
the platform supports it, and should not be synced to a shared location.

Default-deny covers hidden entries, `.ssh`, `.gnupg`, `.aws`, `.kube`, `.docker`,
`.password-store`, common password-manager folders, `.env*`, and credential
extensions (`.pem`, `.key`, `.pfx`, `.p12`, `.crt`, `.cer`). Additional paths
(browser profiles outside hidden app-data directories, custom secret stores)
should still be added to exclusions.

## Agent surface

- **stdio by default.** No network listener unless you explicitly run
  `kensaku-cli serve`, which binds loopback only, requires the bearer token in
  `<config>/http.token` (0600), and checks the peer on every connection.
- **Read-only.** No shell and no write tools. `reindex` is listed only when
  `mcp_allow_reindex=true` and the daemon is running; the HTTP transport never
  enables it.
- **Scoped reads.** `agent_scoped_roots` restricts results and single-path reads.
  Paths are resolved before comparison, so `..` and symlink escapes are rejected.
  Credential files and excluded extensions are refused even inside a scope.
- Logs go to stderr, never stdout, so protocol streams stay clean.
- Tool responses are token-budgeted; file content is data, never instructions.
- No `.gitignore` bypass by default — agents see what the index contains.

## Release integrity

Windows release builds are Authenticode-signed through SignPath Foundation; the
release workflow fails rather than ship an unsigned Windows installer. macOS
builds are not signed or notarized yet, and Intel Macs have no bundled ONNX
Runtime (PP-OCR falls back to Tesseract there).

Every release also publishes Tauri updater artifacts signed with a minisign
key. The public key is compiled into the app (`plugins.updater.pubkey`); the
private key exists only in GitHub Actions secrets and in the maintainer's
offline backup. In-app updates require a valid signature and fail closed on a
missing or mismatched one. The manifest (`latest.json`) is not signed, but it
cannot make a client accept an artifact without a valid signature.

Verify manual downloads against `SHA256SUMS` attached to the GitHub release.

Windows in-app updates install the per-user NSIS package only. MSI installs
under `Program Files` and portable copies are excluded at runtime and must
update through their own package, so an update cannot silently create a second
install. `.deb`/`.rpm` users update through their package manager; the AppImage
updates in place. See `docs/UPDATER.md`.
