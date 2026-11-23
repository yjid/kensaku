# Releasing

The workflow builds, the maintainer publishes. This keeps release creation out of
GitHub Actions (no release-token permissions to fight) and makes the published
assets explicit.

## 1. Prepare

```bash
git checkout master && git pull
# bump the version in package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
# and src-tauri/Cargo.lock; bump cli/Cargo.toml when the CLI changes. The CLI
# depends on the app crate, so refresh cli/Cargo.lock after every app version
# bump too (the commands below do it).
# update CHANGELOG.md

# The CLI is a separate workspace with its own lockfile. A version bump in the
# library it depends on leaves cli/Cargo.lock stale, and `cargo --locked` then
# fails on a clean checkout. Refresh the app lock for the version bump too, then
# verify both:
cargo metadata --format-version 1 --manifest-path src-tauri/Cargo.toml > /dev/null
cargo metadata --format-version 1 --manifest-path cli/Cargo.toml > /dev/null
cargo metadata --locked --format-version 1 --manifest-path src-tauri/Cargo.toml > /dev/null
cargo metadata --locked --format-version 1 --manifest-path cli/Cargo.toml > /dev/null

bun install
cargo test --locked --all-targets --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --locked --manifest-path cli/Cargo.toml
npx tsc --noEmit && bun run test && bun run build
git add -A && git commit -m "chore(release): vX.Y.Z"
git push origin master
```

## 2. Build on CI

```bash
git tag -a vX.Y.Z -m "Kensaku vX.Y.Z"
git push origin vX.Y.Z
gh run watch   # or: gh run list --workflow=release.yml
```

`.github/workflows/release.yml` produces artifacts:

| Artifact | Contents |
|---|---|
| `kensaku-linux` | `.deb`, `.rpm`, `.AppImage`, `.AppImage.sig` |
| `kensaku-macos` | universal `.dmg` / `.app`, `.app.tar.gz`, `.app.tar.gz.sig` |
| `kensaku-windows` | `.msi`, NSIS `.exe` + `.sig`, portable `.zip` (x64 + ARM64) |
| `kensaku-cli-{linux,macos,windows}` | `kensaku-cli` binary (including Windows ARM64) |
| `latest.json` | Updater manifest (generated in the publish job) |

Builds are **Authenticode-signed on Windows** through SignPath Foundation; the
workflow fails instead of shipping an unsigned Windows installer. macOS builds
are still unsigned. See `SIGNING.md`.

### Updater signing

Every release must also be signed with the updater's minisign key. The public
key lives in `src-tauri/tauri.conf.json`; the private key and password are the
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository
secrets. A missing key fails the bundle job rather than publishing an update no
client can verify.

Tauri signs the installers before SignPath re-signs them, so the release job
re-signs the final NSIS installer and regenerates `latest.json` from the signed
assets. See `UPDATER.md` for the full flow, the backup requirement, and the
MSI/portable policy.

### ONNX Runtime

Release builds bundle the ONNX Runtime 1.31.0 shared library. Upstream removed
the standalone 1.31.0 archives from `microsoft/onnxruntime`, so
`.github/actions/fetch-ort` extracts the library from the **v1.0.18 release
bundles** (portable zips, `.deb`, `.dmg`) and verifies each bundle against the
SHA-256 from that release's `SHA256SUMS`. The pin is in the action; bump it when
a later release bundles a newer runtime. Do not publish a separate vendor
release: the Releases page is for Kensaku releases only.

## 3. Review and publish the draft

The tag push builds every platform and assembles a **draft** GitHub release with
all bundles, the CLI binaries, and `SHA256SUMS`.

```bash
# wait for the release build
gh run list --workflow=release.yml --limit 1

# review the draft
gh release view vX.Y.Z
gh release download vX.Y.Z -D /tmp/kensaku-release
(cd /tmp/kensaku-release && sha256sum -c SHA256SUMS)
```

Check that the expected assets are present:

| Asset | Platform |
|---|---|
| `kensaku_*_amd64.deb`, `kensaku-*.rpm`, `kensaku_*.AppImage`, `kensaku_*.AppImage.sig` | Linux |
| `kensaku_*_universal.dmg`, `kensaku.app.tar.gz`, `kensaku.app.tar.gz.sig` | macOS |
| `kensaku_*_x64-setup.exe`, `kensaku_*_x64-setup.exe.sig`, `kensaku_*_x64_en-US.msi` | Windows |
| `kensaku_*_arm64-setup.exe`, `kensaku_*_arm64-setup.exe.sig`, `kensaku_*_arm64_en-US.msi`, `kensaku-portable-*-{x64,arm64}.zip` | Windows (x64 and ARM64) |
| `kensaku-cli-linux-*`, `kensaku-cli-macos-*`, `kensaku-cli-windows-*` | Agent CLI |
| `latest.json` | Updater manifest |
| `SHA256SUMS` | All assets |

Before publishing, confirm the updater manifest is present and correct:

```bash
gh release download vX.Y.Z -p latest.json -O /tmp/latest.json
node -e 'const m=require("/tmp/latest.json"); console.log(m.version); console.log(Object.keys(m.platforms))'
```

The version must match the tag, and the platform keys must include
`windows-x86_64`, `darwin-aarch64`, and `linux-x86_64` (plus the ARM64 variants
when built). Each pointed-to asset and its `.sig` must be on the release.

Then publish the draft (or edit the notes first):

```bash
gh release edit vX.Y.Z --draft=false --latest
```

If the workflow could not create the draft, the manual fallback is:

```bash
RUN=$(gh run list --workflow=release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
rm -rf /tmp/kensaku-release && mkdir -p /tmp/kensaku-release
gh run download "$RUN" -D /tmp/kensaku-release
find /tmp/kensaku-release -type f -exec sha256sum {} + > /tmp/kensaku-release/SHA256SUMS
gh release create vX.Y.Z --title "Kensaku vX.Y.Z" --notes-file /tmp/release-notes.md \
  $(find /tmp/kensaku-release -type f)
```

## 4. Verify

- The release page lists all bundles, the CLI binaries, and `SHA256SUMS`.
- `gh release view vX.Y.Z`
- Download one bundle, verify its checksum, install it, and launch.
- Check `kensaku-cli state --json` on a machine with an index.

## Signing / notarization

Windows Authenticode signing is configured in `release.yml` and uses SignPath
Foundation; see `SIGNING.md` for the repository variables and the single API
token secret. A tag build fails if they are missing or if the returned signature
does not verify.

macOS notarization is not configured yet. It needs `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD`, and `APPLE_TEAM_ID`. Until then macOS builds are unsigned.
