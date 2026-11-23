# Roadmap

Kensaku is the local search index for you and your agents: filename and content
search that stays on your machine, plus a read-only MCP server so Claude, Cursor
and other agents can search the same index.

This page is a direction, not a contract. Dates are target windows; a version
ships when its exit criteria are met. The live work for each version lives in
the matching [milestone](https://github.com/yjid/kensaku/milestones).

## Versions

| Version | Theme | The one outcome | Target |
|---|---|---|---|
| 1.0.x | **Trust** | A stranger on a 200k-file disk gets a useful result in two minutes and does not call it broken | Dec 2026 |
| 1.1 | **Agent** | An agent user prefers Kensaku to a filesystem MCP server, and we can prove it | Jan 2027 |
| 1.2 | **Reach** | Files that are not on this disk — NAS, USB, offline — are searchable, and agents can cite them | Feb 2027 |
| 1.3 | **Shell** | Kensaku is reachable from the launcher people already use; no new app to remember | Mar 2027 |
| 2.0 | **Platform** | Others build on the index; Kensaku is infrastructure, not just an app | Q2 2027, gated |

## 1.0.x — Trust

Make the first hour calm. Search already works during the first scan; the job is
to make the stages legible and the app unbreakable on large libraries.

- Visible stages (names → text → OCR), coverage, honest ETA, pause/resume.
- Large-corpus stability: no dead window, no silent retry queue.
- A coverage report and "why wasn't this found?" instead of warning-speak.
- Launcher clarity: hide-to-tray is not a crash; window placement; hotkey
  conflicts.
- Signed Windows builds; documented macOS signing status.
- One clear statement on how Kensaku is built and maintained.

**Exit:** 100k files searchable in under 60 seconds; two weeks with no
large-corpus freeze reports; SmartScreen clean.

## 1.1 — Agent

Turn "your agent can search your whole disk" into a demonstrated product.

- A public agent benchmark: task success, tokens, wall time, citation rate,
  against a filesystem MCP server and grep.
- A guided first agent session with a visible working transcript.
- Coverage/`stats` and "no results + why" for agents.
- One-click setup hardening and distribution in MCP directories.
- Per-root scoping ("the agent can only see these folders").

**Exit:** 200 weekly MCP connections; five independent write-ups with a real
workflow; a benchmark a stranger can rerun.

## 1.2 — Reach

The files that are not on this disk.

- NAS/SMB content indexing through OS-mounted paths, with schedules and
  throttling.
- Offline catalogs for USB and unmounted drives, marked as offline.
- OneDrive/SharePoint local-mirror mode (no OAuth, no sync).
- Per-root policy: names only / content / OCR / schedule.
- Enterprise packaging: MSI/Intune, no-admin, read-only agent policy, audit log.

**Exit:** 50k SMB files indexed in under 10 minutes on a LAN; offline search
works with stale markers; three design-partner pilots.

## 1.3 — Shell

Meet people where they already type.

- Flow Launcher plugin.
- PowerToys Command Palette extension (including a fallback command).
- Explorer type-to-search or a context-menu "Search in Kensaku".
- Jump to matching line / open at page.
- App search and launch.
- Hotkey conflict handling in setup.

**Exit:** plugin installs are measurable and retained; no Alt+Space collision
reports; a user can go a day without opening the Kensaku window.

## 2.0 — Platform (gated)

Only when the foundations are stable:

1. Versioned MCP/HTTP tool contract.
2. Three or more external integrations shipped by people other than us.
3. The agent benchmark refreshed at least twice.
4. macOS signed and notarised; Windows production-signed.
5. A sustainability answer that protects the local-first promise.

Scope at that point: public API, index export, plugin/extractor SDK, multi-root
administration for teams.

## Not doing

- Cloud search, sync, accounts, or telemetry.
- A bundled LLM or in-app chat. Agents bring their own model.
- Write/delete tools in the agent API.
- A mobile app.
- A plugin marketplace.
- Competing with the OS on default filename search.

## How to help

- Pick a [`good first issue`](https://github.com/yjid/kensaku/labels/good%20first%20issue)
  or a [`help wanted`](https://github.com/yjid/kensaku/labels/help%20wanted)
  item in the current milestone.
- Adopt an extractor, a translation, or an integration (Flow, Raycast,
  PowerToys). Each has a spec issue; the plugin can live in its own repo.
- Bring a real use case to [Discussions](https://github.com/yjid/kensaku/discussions).
  Ideas with a concrete workflow get scheduled first.
- Use it and break it. A reproducible bug report is the most valuable
  contribution right now, especially on macOS and Linux.

Read [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) before opening a PR, and
[docs/MAINTAINING.md](docs/MAINTAINING.md) if you want to help run a milestone.
