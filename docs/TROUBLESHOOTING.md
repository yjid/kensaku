# Troubleshooting

The problems that come up most often on first install and after an upgrade. For
search behaviour — "why no results for a word I know is in a file?" — see
[FAQ.md](FAQ.md).

Start with **Settings → Health → Doctor**. It checks the config directory, the
metadata database, the full-text and semantic indexes, watched roots, the
watcher, model files, OCR, external extractors, the crash log and the version in
one pass, and each failing row names its own fix.

## macOS keeps asking for folder access

**Symptom:** you grant Desktop, Documents or Downloads, and macOS asks again —
sometimes within seconds, and again after every upgrade. Full Disk Access is
already enabled in System Settings.

**Why:** Kensaku's macOS builds are unsigned. The system identifies the app by an
ad-hoc signature whose hash changes whenever the binary changes, so macOS cannot
tie a stored permission to what it considers a different app and prompts again.
Full Disk Access does not help here: the grant is rejected on the code-requirement
check before it is consulted.

**Workaround until signed builds ship:**

1. Quit Kensaku from the tray.
2. Keep exactly one copy: `/Applications/kensaku.app`. Delete downloaded copies —
   a second bundle in `~/Downloads` produces its own prompts.
3. Reset the stored decisions:
   ```bash
   tccutil reset All com.gochi.kensaku
   ```
4. Relaunch and grant each folder once.
5. Don't move the bundle afterwards.

Expect the prompt to come back after an upgrade, because the signature changes
with the new binary. Grant once more and it settles until the next upgrade.

To help narrow the cause, run this right after a prompt appears:

```bash
log show --last 5m --predicate 'subsystem == "com.apple.TCC"' --style compact | grep -i kensaku
```

A `csreq` or cdhash mismatch confirms the explanation above; a helper process
asking on its own would be a different bug worth reporting.

**Status:** tracked in [#73](https://github.com/yjid/kensaku/issues/73). The
durable fix is Developer ID signing plus notarisation; there is no date for it
yet.

## macOS says the app is damaged, or Gatekeeper blocks the first launch

Right-click the app → **Open** → **Open**, or clear the quarantine flag:

```bash
xattr -dr com.apple.quarantine /Applications/kensaku.app
```

Then check the file against `SHA256SUMS` from the release page before trusting it.

## Windows shows "Unknown publisher"

The installer and app are Authenticode-signed, but a new release starts with no
SmartScreen reputation, and current builds use a test certificate while the
production certificate is pending. Choose **More info** → **Run anyway**, or
verify the download against `SHA256SUMS` first.

## OCR says "Indra read nothing in its self-test"

1.0.18 verifies the Indra pack by size rather than only checking that a file
exists, and no longer caches a failed self-test for the life of the process.

**Fix:** Settings → OCR → select **Indra** → **Download** again. A stale or
truncated pack is now reported as not installed, so the download replaces it. No
re-index is needed; only files that failed extraction are retried.

If it still fails, the message now says which half broke — detector found no
lines, or the recognizer dropped them. Report it with the `[Kensaku OCR]` lines
from a terminal launch and:

```bash
sha256sum ~/.config/kensaku/ocr-indra/*
```

PP-OCR is the default and does not need the Indra pack, so switching to it
restores image search immediately.

## A folder's contents are never found

Check **Settings → Folders**. A folder set to **Names only** is searchable by file
name and path but is deliberately never read, OCR'd or embedded, so content and
meaning queries will not match inside it. Switching it back to names and content
queues its files for extraction on the next scan.

## Linux: the AppImage does not start

On hardened distributions the AppImage may need `--no-sandbox`. See
[INSTALL.md](INSTALL.md) for per-distro notes.
