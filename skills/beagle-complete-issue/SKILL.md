---
name: beagle-complete-issue
description: Take one agent-ready GitHub issue in the beagle-tools monorepo from spec to merged release — branch from main, implement, write the required tests, run the gate until green, open a labeled PR, wait for CI's gate check, squash-merge, and watch the per-app release. Use when the user says "/beagle-complete-issue", "complete issue 42", "work the next issue", "pick up an agent-ready issue", or pastes an issue URL to implement.
---

# Complete an issue: spec → PR → merge → release

You take **one** issue written by `/beagle-create-issue` (or a human, in the
same template) and drive it all the way to a merged PR. A merge to `main`
cuts a release of the affected app automatically, so treat the merge as a
deploy: the gate is the contract, and you never weaken it.

The deliverable is a **merged PR** and a report of the release it cut — or a
clear stop report explaining exactly what blocked you.

## Inputs

- An issue number (`42`) or URL. Resolve the repo from the cwd:
  `gh repo view --json nameWithOwner -q .nameWithOwner`.
- No issue given → pick the **oldest unblocked** `agent-ready` issue:

  ```sh
  gh issue list --state open --label agent-ready \
    --search "-label:in-progress -label:blocked sort:created-asc" \
    --json number,title,labels,body --limit 20
  ```

  Skip any whose body says `Blocked by #N` while #N is still open
  (`gh issue view N --json state -q .state`). Tell the user which you picked.

## Stop conditions — check continuously

Stop, leave the branch/PR as-is, comment on the issue, and report to the
user when any of these happen:

1. **Ambiguity:** the issue leaves a decision open that changes behavior,
   or the code contradicts the issue (a path or symbol doesn't exist and the
   intent isn't obvious). Ask; don't guess.
2. **Scope:** finishing requires changing files or behavior the issue lists
   as out of scope, touching a second app, changing a public format the
   issue didn't mention, or adding a runtime dependency it didn't approve.
3. **Red you can't fix:** a required check stays red after **3** fix
   attempts, or the only way to green is weakening a check (lint `allow`
   without justification, removing a test, raising a limit, skipping CI).
4. **Unrelated breakage:** `main` itself is red, or a failure is clearly in
   code your diff doesn't touch (flaky/infra). Re-run once with
   `gh run rerun <id> --failed`; if it fails again, stop.

Stop comment format (`gh issue comment N --body-file …`): what you did,
what's blocking, the exact error/output, and the decision you need. Remove
`in-progress`, add `blocked` if it needs a human.

## Step 0 — Preconditions

```sh
git status --porcelain            # must be empty — never stash the user's work
gh auth status
gh issue view N --json number,title,state,labels,body,assignees
```

- Issue must be open, labeled `agent-ready`, not `in-progress` by someone
  else, and every `Blocked by #M` closed. Otherwise stop and say why.
- It must have exactly one `app:*` label and one `release:*` label. If the
  release label is missing, stop and ask (it decides the version bump).

## Step 1 — Claim it

```sh
gh issue edit N --add-assignee @me --add-label in-progress
```

## Step 2 — Branch from fresh main

```sh
git fetch origin
git switch main && git pull --ff-only origin main
git switch -c <type>/N-<slug>
```

- `<type>` from the issue title's Conventional Commit type: `feat`, `fix`,
  `perf`, `docs`, `chore` (refactors and tests use `chore`).
- `<slug>`: 3–6 lowercase hyphenated words from the title,
  e.g. `feat/42-beagle-list-archived`.
- If the branch already exists (a previous attempt), check it out and read
  its commits and any open PR before continuing.

## Step 3 — Load the rules

Read, fully, before writing code:

1. Root `AGENTS.md`
2. The app's `AGENTS.md` (`tandem/`, `rca/`, `workflows/`; `utils/README.md`)
3. `docs/CODING_STANDARDS.md`
4. `docs/TESTING.md`
5. Every file listed under the issue's **Code paths**, plus its tests.

Map each acceptance criterion to the code and test that will satisfy it.
If a listed path or line ref is stale, find where it moved; if you can't
tell, that's stop condition 1.

## Step 4 — Implement

- Do exactly the **Required changes**, in order. Match surrounding style.
- Follow the standards as you write, not after: typed errors, no
  `unwrap`/`expect`/`panic`, no `any`/`!`/unsafe casts, Rust and TS types
  mirrored in the same PR, one `invoke()` module, 400-line files (split by
  responsibility if a file would grow past it), docs on every public item.
- A public format/IPC change needs the app's `AGENTS.md` updated and a
  migration note for the PR.
- Never edit shared configs (`standards/`, root `rustfmt.toml`, toolchain,
  tsconfig base, eslint base) to make code pass.

## Step 5 — Write the tests the issue requires

Every case under **Tests required** gets a test, placed per
`docs/TESTING.md` (Rust unit tests beside the code, integration tests in
`tests/`, frontend unit/render tests with vitest, Playwright specs in
`e2e/`). Fixes always get a regression test that fails without the fix —
verify that by running it before applying the fix when practical.

## Step 6 — Run the gate locally until green

From the repo root:

```sh
cargo fmt --all            # in each touched cargo workspace
corepack pnpm format       # in each touched frontend
make check-<app>           # exactly what CI runs for the app (incl. e2e)
make standards
```

Also run any extra **Verification commands** from the issue. For a
behavior change in a desktop app or CLI, run it (`scripts/dev/` or the
app's `AGENTS.md` instructions) and exercise the flow you touched.

Fix failures in the code, never in the checks. Count attempts per distinct
failure; 3 failed attempts on the same failure is stop condition 3.

## Step 7 — Self-review the diff

```sh
git diff origin/main...HEAD --stat
git diff origin/main...HEAD
```

Go through the issue's **Acceptance criteria** one by one and write down
the evidence for each (test name or command output). Then check:

- Every changed file is in the issue's code paths or is a direct necessity
  of them (tests, mirrors, docs). Anything else → revert it or stop (scope).
- No debug output, commented-out code, TODOs, stray files, secrets, or
  lockfile churn unrelated to the change. `Cargo.lock` changes only with a
  dependency change; never create `package-lock.json`.
- Nothing listed under **Out of scope** was done.

## Step 8 — Commit and push

Conventional Commits scoped by app; reuse the issue title when it fits:

```sh
git add -A
git commit -m "feat(rca): list archived incidents with --archived" \
  -m "Closes #N"
git push -u origin HEAD
```

Several focused commits are fine — the PR is squash-merged.

## Step 9 — Open the PR

Fill `.github/pull_request_template.md`. Write the body to a temp file:

```sh
body=$(mktemp)
cat >"$body" <<'EOF'
## What
...
## Why
...
## How tested
- `make check-rca` ✅, `make standards` ✅
- New tests: ...
- Acceptance criteria: each one, with its evidence
## Release
release:minor — cuts beagle-rca-vX.Y.Z on merge
## Notes for reviewers
Migration notes / new dependency justification / "None"

Closes #N
EOF
gh pr create --base main --head "$(git branch --show-current)" \
  --title "<issue title>" --body-file "$body" \
  --label "app:<app>" --label "release:<bump>"
```

**Copy the issue's `release:*` label onto the PR** — the release workflow
reads the merged PR's labels to pick the semver bump (`release:major`,
`release:minor`, otherwise patch). A wrong label ships a wrong version.

## Step 10 — Wait for CI and fix failures

```sh
gh pr checks <pr> --watch --interval 30
```

- The required check is **`gate`**; it's green only when every gate that
  ran is green.
- On failure: `gh run view <run-id> --log-failed`, reproduce locally with
  the same `make` target, fix, commit (`fix(<app>): …` or amend-free new
  commit), push, watch again. Same 3-attempt limit as Step 6.
- If someone requested changes on the PR, address them or stop and ask.

## Step 11 — Merge

Only when `gate` is green and the PR is mergeable:

```sh
gh pr view <pr> --json mergeable,mergeStateStatus,reviewDecision,labels
gh pr merge <pr> --squash --delete-branch
```

- If `main` moved and the PR is behind, `git pull --rebase origin main`,
  re-run `make check-<app>`, push, and go back to Step 10.
- If branch protection requires a human approval, stop here: report the PR
  as ready, green, and awaiting review. Never bypass protection
  (`--admin`) unless the user explicitly tells you to for this PR.

Then confirm the issue closed (`Closes #N`) and remove `in-progress`:

```sh
gh issue view N --json state -q .state
gh issue edit N --remove-label in-progress
```

## Step 12 — Report the release and watch it

Releases follow `docs/RELEASING.md`: a push to `main` that touches an app's
release-relevant paths releases **that app only**, tagged
`beagle-<app>-vX.Y.Z` (app ∈ `tandem`, `rca`, `workflows`; a `utils/`
change releases all three). Docs-only changes don't release.

Predict the version from the latest tag and the label:

```sh
git fetch --tags origin
git tag --list "beagle-<app>-v*" --sort=-v:refname | head -1
```

Bump major/minor/patch per the label (no tag yet → the app's current
version-file value is the first release). Then find and watch the run:

```sh
gh run list --branch main --event push --limit 10
gh run watch <release-run-id> --exit-status
gh release view beagle-<app>-vX.Y.Z --json tagName,url,assets
```

If the release run fails, don't retry blindly: report the failed job and
its log tail (`gh run view <id> --log-failed | tail -50`) to the user —
release fixes are their own issue.

## Final report

Short, concrete:

- Issue `#N` → PR `#P` (URL), merged as `<sha>`.
- Acceptance criteria: each ✅ with its evidence.
- Gate: green (link to the run).
- Release: `beagle-<app>-vX.Y.Z` — published / in progress / failed (link).
  Users get it via the in-app updater or `brew upgrade`.
- Anything skipped or left for a follow-up issue.
