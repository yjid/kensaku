## What and why

<!-- One paragraph. Link the issue: Closes #NNN -->

## Changes

<!-- Bullet the user-visible changes, not the file list. -->

## How it was tested

- [ ] `cargo test --all-targets --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings`
- [ ] `npx tsc --noEmit`
- [ ] `bun run test`
- [ ] `bun run build`
- [ ] Manual run on: <!-- Windows / macOS / Linux, version -->

## Definition of Done

- [ ] Tests cover the new behaviour (or explain why not)
- [ ] Docs updated (`docs/USAGE.md`, `docs/FAQ.md`, `docs/TROUBLESHOOTING.md` as relevant)
- [ ] `CHANGELOG.md` has a line under the next version
- [ ] Screenshot or short clip for UI changes
- [ ] Performance number for index/search changes
- [ ] Security note for anything touching the agent surface, filesystem, or network
- [ ] `kensaku-cli --json` / MCP contract unchanged, or versioned and documented
- [ ] No telemetry, no new outbound request, no cloud dependency
- [ ] Upgrade from the previous release considered

## Notes for reviewers

<!-- Tradeoffs, alternatives rejected, follow-ups. -->
