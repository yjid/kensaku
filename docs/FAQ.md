# FAQ

**Is anything sent to the cloud?**
No. There is no telemetry and no account. The only network request is the optional
model-file download you trigger in Settings.

**Why is the first launch slow?**
It builds the index. After that, startup is search-first: the snapshot loads and a
background reconcile keeps it honest. Large corpora can take minutes on first run;
you can search while it works.

**Why does search return nothing for a word I know is in a file?**
Check: is the folder watched? Is the extension in the excluded list? Is the file
over the content cap (1 MB indexed, 20 MB extraction ceiling)? Is it binary? Run
Settings → Doctor. The Search syntax card shows filters that may be active.

**Does it index hidden files?**
No by default. `skip_hidden` is on; you can opt in via config.

**Does it index PDFs and Office documents?**
Yes — PDF, DOCX/PPTX/XLSX, EPUB/ODT, RTF. Scanned PDFs need OCR; encrypted PDFs are
flagged, not indexed.

**Does it search images?**
By filename, by OCR text when `tesseract` is installed, and by perceptual
near-duplicate. Semantic image search (CLIP) is not implemented yet.

**Can it search across multiple drives?**
Yes. Add each root in Settings → Folders. Unplugged drives are preserved rather
than pruned.

**How much memory does it use?**
Roughly 128 bytes per file entry plus the Tantivy index. 100k files ≈ 12 MB of
metadata; the OS page cache handles the rest.

**Does it conflict with Spotlight / Everything / Recoll?**
No. It builds its own index and does not read or write theirs.

**Can two instances run?**
No — a file lock makes a second launch exit cleanly.

**Is the app signed?**
Not yet. macOS needs right-click → Open (or `xattr -dr com.apple.quarantine`),
Windows shows SmartScreen. See [`INSTALL.md`](INSTALL.md#the-builds-are-unsigned).

**Where is my data?**
`~/.config/kensaku/` on Linux, `~/Library/Application Support/kensaku/` on macOS,
`%APPDATA%\kensaku\` on Windows. Settings → About → Reset everything wipes it.

**Can I use it from an AI agent?**
Yes — that's the point of `kensaku-cli` and the MCP server. See
[`AGENTS.md`](AGENTS.md). The app itself contains no LLM.

**Why not bundle an LLM / "ask my files"?**
A local LLM is a multi-gigabyte dependency and a different product. Kensaku stays
the fast, trustworthy index; agents bring their own model.

**How do I contribute?**
See [`CONTRIBUTING.md`](CONTRIBUTING.md). Tests and clippy must pass.

**Why is the binary not 18 MB like the early plan?**
That plan assumed static ONNX inside the app. The shipped base uses a bundled
static embedding table instead, keeping the app small; the neural tier is optional.
