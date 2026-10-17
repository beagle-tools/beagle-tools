---
name: beagle-create-issue
description: Turn a feature or bug description into N precise, agent-ready GitHub issues for the beagle-tools monorepo — investigate the code, ask only blocking questions, split the work into one-PR-sized issues ordered by dependency, and create them with gh. Use when the user says "/beagle-create-issue", "spec this out", "write issues for X", "break this into tickets", or describes a feature they want built in tandem, rca, workflows or utils.
---

# Spec a feature into agent-ready issues

The user describes something they want (a feature, a fix, a refactor). Your
job is to turn it into GitHub issues so precise that `/beagle-complete-issue`
— or any engineer — can implement each one **without asking a question**:
exact files, exact symbols, exact behavior, exact tests, exact commands.

You do not write the code. You investigate, decide, and specify.

The deliverable is **issues on GitHub** plus a list of their URLs in chat.

## Step 0 — Preconditions

```sh
gh auth status                                  # must be logged in
gh repo view --json nameWithOwner -q .nameWithOwner
gh label list --limit 100 | grep -E '^(app:|release:|agent-ready)'
```

- Run from a checkout of the monorepo (root has `AGENTS.md` and `Makefile`).
- If the `app:*` / `release:*` / `agent-ready` labels are missing, tell the
  user to run `./scripts/github/labels.sh` and stop — don't create issues
  with missing labels.
- `git fetch origin main` and investigate against `origin/main`, not a stale
  local branch.

## Step 1 — Understand the request

Restate the request in one or two sentences: the user-visible outcome and
who it's for. If the description is a single line, that's fine — the
investigation fills in the detail. Note anything the user explicitly ruled
in or out.

## Step 2 — Investigate the code (before asking anything)

Read, in this order:

1. Root `AGENTS.md` — which apps exist and what lives where.
2. `docs/CODING_STANDARDS.md`, `docs/TESTING.md`, `docs/SDLC.md`.
3. The `AGENTS.md` of every app the request might touch
   (`tandem/`, `rca/`, `workflows/`, `utils/README.md`).

Then locate the code. Use `rg`/`grep` for the nouns in the request (UI
labels, command names, config keys, error strings, types), and read the
files that come up. For every place a change would land, record:

- the **app** (`tandem`, `rca`, `workflows`, `utils`) and package/crate,
- the **file path** relative to the repo root, with **line refs**
  (`rca/cli/src/skill.rs:40-46`),
- the **symbol** (function, type, component, Tauri command, CLI subcommand),
- the **boundary** it crosses (IPC command + its TS mirror type, on-disk
  format, CLI flag, config file) — boundaries are where specs go wrong.

Also find the **closest existing example** of the pattern being added (an
existing Tauri command, an existing settings field, an existing e2e spec) —
issues should say "follow the shape of X".

Check what already exists so you don't spec duplicate work:

```sh
gh issue list --state open --search "<key words> in:title,body" --limit 20
gh issue list --state closed --search "<key words>" --limit 10
gh pr list --state open --search "<key words>" --limit 10
```

If an open issue already covers part of this, link it instead of
re-creating it, and tell the user.

## Step 3 — Ask only blocking questions

Ask the user **only** what you cannot decide from the code and that changes
the spec materially: product behavior with two reasonable answers, a public
format change (on-disk/IPC — needs a migration note), a new runtime
dependency, or a breaking CLI change. Batch them into one message, each
with your recommended default, e.g.:

> 1. Should archived jobs appear in the list? (default: no, behind a toggle)

Everything else — naming, file placement, test layout — decide yourself by
following the standards and the existing code. Write your decisions into
the issues so a reviewer can disagree with them.

If the user doesn't answer a question, use the stated default and say so in
the issue's **Context** section.

## Step 4 — Decompose into one-PR issues

Split the work so **each issue is one small, single-purpose PR**
(CODING_STANDARDS §4):

- **One app per issue.** An issue touches exactly one of `tandem/`, `rca/`,
  `workflows/` — or `utils/` when it's genuinely shared code (used by ≥2
  apps). A change that needs a new util and its use in two apps is three
  issues: utils first, then one per app.
- **Split by layer when it's big:** data format/persistence → Rust domain
  logic + Tauri command → frontend. A PR that changes a data format, the
  store and the renderer is three PRs.
- **Each issue is shippable alone:** main stays green and releasable after
  each merge (each merge can cut a release of that app). Hide unfinished UI
  behind the absence of an entry point rather than shipping a broken one.
- **Order by dependency.** Number them in the order they must merge. Later
  issues say `Blocked by #N`. Avoid chains longer than necessary — parallel
  issues are better than a long serial chain.
- Aim for issues that are a few hundred lines of diff including tests.
  Keep any single file under the 400-line rule; if a file the issue touches
  is near 400 lines, the issue must say to split it first.

Pick the **release label** per issue (it drives that app's semver bump when
the PR merges — see `docs/RELEASING.md`):

| Label           | When                                                                                 |
| --------------- | ------------------------------------------------------------------------------------ |
| `release:major` | breaking change to a CLI flag, on-disk/IPC format without migration, removed feature |
| `release:minor` | new user-visible feature or command                                                  |
| `release:patch` | fix, refactor, perf, internal change, tests, docs inside an app                      |

`utils` issues still pick a label: a merge to `utils/` releases every app.

## Step 5 — Write each issue

Use this template **exactly** (it matches `.github/ISSUE_TEMPLATE/agent-task.yml`).
Every section is required; write "None" only where that is the true answer.

````markdown
## Summary

<One or two sentences: what changes, user-visibly.>

## Context / why

<Why this is needed, link to the parent request, decisions you made and
defaults you applied. Link related issues/PRs.>

## App + labels

- App: `<tandem|rca|workflows|utils>` (package/crate: `<name>`)
- Labels: `app:<app>`, `release:<patch|minor|major>`, `agent-ready`
- Blocked by: #<N> (or "None")

## Code paths

- `path/to/file.rs:120-168` — `fn existing_thing` — change: <what>
- `path/to/new_file.rs` (new) — add `struct NewThing` — <responsibility>
- `path/to/ipc.ts:30` — mirror the new field in `type Thing`
- Pattern to follow: `path/to/example.rs:40` (`fn similar_command`)

## Required changes

1. <Precise, imperative step naming the file and symbol.>
2. <Error handling: which typed error variant, what message the user sees.>
3. <Boundary updates: Rust type + TS mirror + type guard in the same PR.>
4. <Docs: update `<app>/AGENTS.md` if a format/command changed.>

## Acceptance criteria

- [ ] <Observable, testable statement — "Running `agentctl list --archived`
      prints archived jobs", not "archiving works".>
- [ ] <Error case behaves as specified.>
- [ ] `make check-<app>` and `make standards` pass.

## Tests required

Per `docs/TESTING.md`:

- Unit: `<file>` — <case 1>, <case 2>, <invalid-input case>
- Integration: `<crate>/tests/<name>.rs` — <case> (or "None")
- Frontend unit/render: `src/lib/<x>.test.ts` — <case> (or "None")
- e2e (Playwright): `e2e/<flow>.spec.ts` — <flow> (or "None")
- Regression test for: <the bug, if this is a fix>

## Out of scope

- <Things a reasonable implementer might also do, and must not.>

## Release impact

`release:<bump>` — <one line why>. Cuts `beagle-<app>-vX.Y.Z` on merge.

## Verification commands

```sh
make check-<app>
make standards
<app-specific run/e2e command, e.g. `cd tandem && corepack pnpm e2e`>
```
````

Quality bar for each issue — check before creating:

- Every path in **Code paths** exists on `origin/main` (or is marked
  `(new)`), and every line ref is current. Re-open the file to confirm.
- **Required changes** name a file and symbol in every step. No step says
  "update as needed", "handle appropriately", or "etc.".
- Every **acceptance criterion** can be checked by a test or a command.
- **Tests required** lists concrete cases, including at least one failure
  or invalid-input case for new logic, and a regression test for a fix.
- **Out of scope** names the tempting adjacent work explicitly.
- The issue obeys the coding standards (typed errors, no `unwrap`, IPC types
  mirrored by hand, one `invoke()` module, etc.) — never spec something the
  gate would reject.

## Step 6 — Show the plan, then create

Show the user the list before creating anything:

```
1. [rca] feat: add `archived` field to rca.toml manifest      release:minor
2. [rca] feat: `beagle list --archived`            (blocked by 1) release:minor
3. [tandem] fix: ...                                            release:patch
```

Ask "Create these N issues?" — proceed on yes, or adjust. If the user
already said to just create them, skip the confirmation.

Create them **in dependency order** so earlier numbers exist when later
issues reference them. Write each body to a temp file (never inline a long
body in the shell):

```sh
body=$(mktemp)
cat >"$body" <<'EOF'
...issue body...
EOF
gh issue create \
  --title "feat(rca): add archived field to rca.toml manifest" \
  --body-file "$body" \
  --label "app:rca" --label "release:minor" --label "agent-ready"
```

- Titles use Conventional Commits form scoped by app: `feat(rca): …`,
  `fix(tandem): …`, `chore(utils): …`. The completing agent reuses it as
  the PR title and squash-commit message.
- Capture each URL/number from `gh issue create`'s output and substitute it
  into later issues' `Blocked by #N` **before** creating them.
- A blocked issue gets the `blocked` label as well; `/beagle-complete-issue`
  skips it until its blockers are closed.
- For more than one issue, also create a short tracking issue only if the
  user asks for one.

## Step 7 — Report

Reply with a table, nothing else:

| #   | Title | App | Release | Blocked by | URL |
| --- | ----- | --- | ------- | ---------- | --- |

Then one line: which issue `/beagle-complete-issue` should take first (the
lowest-numbered unblocked one).

## Rules

- Never create an issue whose code paths you haven't opened.
- Never put secrets, tokens, internal hostnames or customer data in an issue.
- Never assign humans or set milestones unless asked.
- Don't edit code or open PRs from this skill.
