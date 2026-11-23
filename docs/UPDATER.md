# Updates

Kensaku uses Tauri's signed updater. An installed copy checks a small JSON
manifest and, when an update is started, downloads the new package, verifies its
minisign signature, and installs it in place. Windows no longer opens the NSIS
wizard for an update, and the user does not have to revisit the Releases page.

## How it works

1. On startup (and when the user presses **Check for updates**) the app fetches
   `https://github.com/yjid/kensaku/releases/latest/download/latest.json`.
2. `latest.json` names the artifact and carries its signature for the current
   platform.
3. The Tauri updater downloads that artifact and verifies the signature against
   the public key compiled into the app (`plugins.updater.pubkey` in
   `src-tauri/tauri.conf.json`). `requireSignedVersion` also binds the signature
   to the announced version, so a tampered manifest cannot pair a newer version
   with an older signed package.
4. A valid update is installed and the app relaunches:

   | Platform | Package the updater installs |
   |---|---|
   | Windows | The per-user NSIS `*-setup.exe` with `/UPDATE` |
   | macOS | The universal `kensaku.app.tar.gz` (the `.dmg` stays manual) |
   | Linux | The `.AppImage` (`.deb`/`.rpm` stay with the package manager) |

The manifest itself is not signed, but every artifact it points to is. A forged
manifest cannot produce a valid minisign signature, so the updater rejects the
download.

## Windows install kinds

| Install | Update path |
|---|---|
| NSIS, per-user (`currentUser`) | In-app, passive progress only, no UAC |
| MSI, per-machine | Manual: install the new `.msi` |
| Portable zip | Manual: unpack the new portable zip next to the old one |

Mixing them is avoided on purpose. An NSIS update on an MSI install would create
a second copy under `%LOCALAPPDATA%` while the MSI copy stayed under
`Program Files`. The `updater_supported` command therefore refuses in-app
updates for portable copies and for executables under `Program Files`; the
settings panel shows a manual-update message instead of the download button. An
MSI installed to a custom directory outside `Program Files` cannot be detected
without the Windows bundle marker, so keep MSI deployments in the default
location or update them manually.

On Linux, the same command refuses `.deb`/`.rpm` installs (their bundle marker
identifies them) because those belong to apt/dnf. AppImage installs, which carry
the AppImage marker, update in place. The AppImage must live in a user-writable
directory; replacing one under `/opt` or `/usr/bin` fails without root, so move
it somewhere writable or use the package manager.

`tauri bundle` runs with `--no-binary-patching` on Windows only, so the
Authenticode signature on `kensaku.exe` survives bundling. That also means
Windows builds have no per-bundle marker; `latest.json` publishes the NSIS
artifact under both `windows-x86_64` and `windows-x86_64-nsis` (and the ARM64
equivalents) so the lookup works either way. macOS and Linux keep the marker.

## Keys

The updater uses a minisign keypair, independent of Windows Authenticode
signing:

- **Public key**: `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. Safe
  to commit.
- **Private key**: the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret.
- **Password**: the `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub Actions secret.
- **Backup**: the maintainer keeps a copy outside the repository (and outside
  this checkout). Losing the private key means existing installs reject every
  future update; recovering requires a new key and one manual migration
  release.

Generate a new pair only when starting a new update channel:

```bash
bunx tauri signer generate -w ~/.tauri/kensaku-updater.key
```

Then put the contents of the `.pub` file in `tauri.conf.json`, and the private
key and its password in the two repository secrets. To rotate an existing
channel, first ship one release that carries the new public key but is signed
with the **old** private key; only after that release is installed should CI
switch to the new private key. Otherwise existing clients reject the new
signature.

## Release pipeline

`tauri bundle` creates the updater artifact and a `.sig` file for every bundle
when `bundle.createUpdaterArtifacts` is `true`. The release workflow then:

1. **Builds and Authenticode-signs** `kensaku.exe` before bundling.
2. **Bundles** the installers. Tauri writes minisign signatures now, but they
   cover the pre-SignPath installers.
3. **Signs the installers with SignPath**. This changes the bytes, so the
   Tauri-generated signatures are stale.
4. **Re-signs the final NSIS installer** with
   `bunx tauri signer sign --app-version <version>` and removes the MSI
   signature (the MSI is not an in-app update target). The explicit app version
   keeps `requireSignedVersion` working for the hand-signed artifact.
5. **Stages** the installers, the `.sig` files, `.app.tar.gz`, and the
   AppImage.
6. **Generates `latest.json`** with `tools/generate-latest-json.mjs`, which
   fails if an artifact or signature is missing, or if the tag does not match
   `src-tauri/tauri.conf.json`.
7. **Uploads** everything to the draft release. The
   `releases/latest/download/latest.json` URL starts working when the draft is
   published as the latest release.

Do not edit `latest.json` by hand. Regenerate it from the signed assets; a
signature that does not match its artifact makes every client reject the
update.

## Migration from 1.0.16–1.0.18

Those releases use the old updater that downloaded and launched a full
installer. They cannot transition themselves. Users need **one** manual install
of the first release that contains the new updater (1.0.19). From that release
on, updates are in-app.

Keep `SHA256SUMS` and the manual installers on every release for offline
installs and for MSI/portable users.

## Troubleshooting a release

- **`latest.json` is missing from the release**: the publish job failed before
  uploading, or the draft is still a draft. The `/releases/latest/` URL only
  resolves a published, non-prerelease release.
- **App says no update exists although a newer tag is published**: check that
  the manifest has an entry for the running platform and that its `version` is
  higher than the app version.
- **"signature verification failed"**: the `.sig` was generated before
  SignPath re-signed the installer, or the manifest points at a different file
  than the one signed. Re-run the re-sign and manifest steps. The manifest
  generator also rejects a signature bound to a different app version.
- **Update keeps reappearing**: the manifest version and the bundled
  `tauri.conf.json` version disagree. The tag build checks this and fails.
