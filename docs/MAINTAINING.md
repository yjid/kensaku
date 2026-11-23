# Maintaining Kensaku

This is the operating manual for the repository: how goals, versions and
contributions are managed. It is deliberately lightweight — one maintainer plus
contributors, not a process department.

## The model

| Layer | GitHub artifact | Holds | Changes |
|---|---|---|---|
| Direction | GitHub Discussions | Themes and outcomes, 2-3 quarters out | Quarterly |
| Version | **Milestone** | One outcome, scope, exit criteria, due window | At version start |
| Workstream | **Epic issue** | The spec and child issues for one part of a version | At version start |
| Work | **Issue** | One shippable change with acceptance criteria | Continuously |
| Change | **Pull request** | Code, tests, docs, changelog | Continuously |
| Idea | **Discussion** | Problem, use case, votes | Continuously |

Rules that keep this honest:

- **Issues are not a wish list.** An issue that is not accepted gets no
  milestone and no assignee. Ideas stay in Discussions until they have a use
  case and a shape.
- **Milestones are versions, not dates.** A version closes when its exit
  criteria are met. Patch releases float freely between milestones.
- **No dates on individual issues.** Only the milestone has a target window.
- **One outcome per version.** If a proposed issue does not serve the outcome,
  it goes to `Later`.
- **Every epic has an owner.** If nobody owns it, it is not in the milestone.

## Setup (one time)

### 1. Milestones

Create one per version. The description is the contract: outcome, scope,
out-of-scope, exit criteria.

```bash
gh api repos/yjid/kensaku/milestones -X POST \
  -f title="1.1 — Agent" \
  -f description="Outcome: an agent user prefers Kensaku to a filesystem MCP server. Scope: benchmark, guided session, stats/why-not-found, MCP distribution, per-root scoping. Out of scope: chat UI, write tools. Exit: 200 weekly MCP connections, 5 independent write-ups, reproducible benchmark." \
  -f due_on="2027-01-09T23:59:59Z"
```

Repeat for `1.0.x — Trust`, `1.2 — Reach`, `1.3 — Shell`, `2.0 — Platform`
(due date far out; it is gated). Use `Later` for accepted-but-unscheduled work.

### 2. Project

Create a GitHub Project (Projects v2) named **Kensaku roadmap**. One project,
several views.

Fields:

| Field | Type | Values |
|---|---|---|
| Status | Single select | Inbox, Triaged, Ready, In progress, In review, Done |
| Track | Single select | Product, Growth, Quality, Docs |
| Area | Single select | Indexer, Search, UI, Agent, OCR, Integrations, Release |
| Version | Single select | Trust, Agent, Reach, Shell, Platform, Later |
| Priority | Single select | P0, P1, P2 |
| Size | Single select | S (≤1 day), M (≤1 week), L (needs design) |
| Contributor-ready | Single select | Yes, No |

Views:

1. **Roadmap** — roadmap layout, group by Version. This is the public-facing
   picture.
2. **Board** — board layout by Status. Daily driver.
3. **This week** — table filtered to `Status: Ready, In progress`, sorted by
   Priority.
4. **Help wanted** — filtered to `label:help wanted` and
   `Contributor-ready: Yes`.
5. **By area** — table grouped by Area. Used in triage.
6. **Inbox** — new issues and PRs that have not been triaged.

Built-in workflows: auto-add new issues and PRs, set `Status: Inbox` on open,
`In review` when a PR opens, `Done` when it merges or closes. Do not automate
`Ready`; that is a human decision.

### 3. Labels

Labels are facets, not a second roadmap. Keep the set small; the version lives
in the milestone.

Existing: `bug`, `enhancement`, `documentation`, `good first issue`,
`help wanted`, `question`, `accessibility`, `dependencies`, plus Dependabot
labels.

Add:

```bash
for l in "area/indexer:Indexing, watcher, extraction" \
         "area/search:Ranking, query language, relevance" \
         "area/ui:Desktop and launcher UI" \
         "area/agent:MCP, CLI, agent integrations" \
         "area/ocr:OCR engines and model packs" \
         "area/integrations:Launcher and editor plugins" \
         "epic:Version workstream with child issues" \
         "needs-repro:Cannot reproduce yet" \
         "needs-design:Needs a decision before code" \
         "blocked:Waiting on something else" \
         "p0:Drop everything" \
         "p1:This version" \
         "p2:Nice to have"; do
  name="${l%%:*}"; desc="${l#*:}"
  gh label create "$name" --description "$desc" 2>/dev/null || true
done
```

Do **not** add `version/*` labels. The milestone is the version.

### 4. Templates

- `bug_report.yml` and `feature_request.yml` already exist. Keep them.
- Add `epic.yml` for version workstreams (outcome, scope, child issues, exit
  criteria, owner).
- Add a pull-request template with the Definition of Done.
- `config.yml` should link Discussions and the security policy.

### 5. Discussions

Categories:

- **Announcements** — releases, roadmap updates. Maintainer-only posting.
- **Ideas** — feature requests. Upvotes are a signal, not a queue.
- **Q&A** — usage questions. Answers become docs.
- **Show your setup** — user stories and screenshots. Feeds marketing.
- **Integrations** — plugin authors, launcher extensions, MCP clients.
- **Roadmap** — the monthly update thread.

Move an accepted Idea to an Issue with a milestone and a link back. Close the
Discussion with "tracked in #NNN". That keeps one canonical place per decision.

## Weekly rhythm

| When | Time | What |
|---|---|---|
| Monday | 20 min | Triage Inbox: label area, set priority/size, milestone or `Later`; close duplicates; nudge issues older than 14 days |
| Wednesday | 10 min | Changelog draft from merged PRs; check the current milestone's exit criteria |
| Friday | 10 min | Review open PRs; update `This week`; one public reply or Discussion post |
| First of month | 30 min | Public roadmap update: shipped, next, not doing, one ask for help |
| End of milestone | 1 hour | Release checklist, close milestone, retrospective post |

If a week is busy, skip the Friday review, never the Monday triage. An
untriaged inbox is how a project starts to feel abandoned.

## Contributor funnel

### Always stocked

- 5–10 `good first issue` items. Each must be self-contained, ≤1 day, and not
  require a design decision. If it needs a decision, it is not a good first
  issue.
- 2–3 `help wanted` epics with a spec: problem, files to touch, acceptance
  criteria, test plan, size.
- One non-code ask per release: translation, docs page, benchmark run, demo
  video, reproduction of a macOS bug.

### Programs

- **Adopt an extractor** — one file format per contributor, with a test corpus.
- **Adopt a platform** — a macOS or Linux user who runs the release candidate
  and files reproducible reports. Credit them in release notes.
- **Adopt an integration** — Flow, Raycast, PowerToys, Alfred. Each has a spec
  and its own repo; the CLI/JSON contract is the interface.
- **Version captain** — one volunteer per theme who triages that area and
  reviews PRs. The maintainer keeps architecture and releases. Captains are
  named in the milestone and in the release notes.

### Rules

- Comment "I'd like to work on this" → assign. Unassign after 7 days of
  silence, with a friendly note. No shame, no dead issues.
- Review external PRs within 3 business days, even if the answer is "not yet".
- A PR that changes behaviour needs a test and a changelog line. A UI PR needs
  a screenshot. An agent-surface PR needs a security note. A perf PR needs a
  number.
- Credit every contributor in the release notes. Use GitHub's generated list
  plus a sentence for the notable ones.
- Never label an architectural decision `good first issue`. Decisions are
  written down in `docs/decisions/NNNN-*.md` and implemented in the open.

## Definition of Done

A version is done when:

- [ ] Every issue in the milestone is closed or moved with a comment.
- [ ] Exit criteria in the milestone description are met and demonstrated.
- [ ] Changelog has a story, not a commit dump.
- [ ] Docs updated: usage, FAQ, troubleshooting, privacy if anything changed.
- [ ] Benchmarks refreshed if the change touches search or indexing.
- [ ] Signing/verification status is accurate for every platform.
- [ ] Upgrade from the previous release tested on Windows, macOS, Linux.
- [ ] Release notes name the contributors and the next theme.
- [ ] Retrospective posted: what worked, what slipped, what changes next time.

## Metrics to review monthly

Project health, not vanity:

| Metric | Healthy |
|---|---|
| Issues older than 14 days without a label or milestone | 0 |
| PRs waiting for review > 3 business days | 0 |
| `good first issue` items open | 5–10 |
| Milestone scope churn after start | < 20% |
| Contributors in the last 30 days | growing |
| First response to a new issue | < 48 h |

Stars and downloads are inputs to marketing, not project health.

## Anti-patterns

- A public roadmap with dates you cannot keep.
- A milestone that becomes a dumping ground.
- "Good first issue" as a decoration to attract contributors, then no review.
- Letting Discussions and Issues duplicate the same request.
- Arguing scope in a PR instead of in the issue.
- Adding a label for every dimension; use the project fields.
- Closing a milestone with open issues "because the release shipped".

## First ten days

1. Create the five milestones and the `Later` milestone.
2. Create the project, fields, and six views.
3. Add the label set and the epic/PR templates.
4. Move the current open issues into the right milestone or `Later`; close the
   ones that are already fixed.
5. Write the pinned "how Kensaku is built" statement and close #175/#148.
6. Open 8 `good first issue` items from the 1.0.x and 1.3 lists.
7. Post the first monthly roadmap update in Discussions.
