# How work flows: issue → PR → merge → release

Every change to this monorepo takes the same path, whether a human or a
coding agent does the work. Each step has one owner and one artifact, so
you can always tell where a change is and what it's waiting on.

```
 idea ──/beagle-create-issue──▶ issues (agent-ready, app:*, release:*)
                                   │
                    /beagle-complete-issue
                                   ▼
 branch <type>/<N>-<slug> ──▶ make check-<app> + make standards (local)
                                   │
                                   ▼
 PR (template, Closes #N, app:* + release:* labels) ──▶ CI ──▶ `gate` ✅
                                   │
                         squash-merge to main
                                   ▼
 release workflow: only the apps whose release paths changed
   └─▶ tag beagle-<app>-vX.Y.Z + GitHub release + beagle-<app>-latest mirror
         ├─▶ desktop apps: in-app "Update" button (Tauri updater)
         └─▶ CLIs / casks: brew upgrade (beagle-tools/tap)
```

Related docs:

- [`CODING_STANDARDS.md`](CODING_STANDARDS.md) — the rules the gate enforces.
- [`TESTING.md`](TESTING.md) — what tests a change needs and where they go.
- [`DEVELOPMENT.md`](DEVELOPMENT.md) — tooling setup and running each app locally.
- [`RELEASING.md`](RELEASING.md) — tags, versioning, assets, Homebrew, updater.
- [`../skills/README.md`](../skills/README.md) — the agent skills used below.

## 1. Spec: `/beagle-create-issue`

You describe a feature or bug. The skill
([`skills/beagle-create-issue`](../skills/beagle-create-issue/SKILL.md))
investigates the code, asks only the questions it can't answer from the
code, and splits the work into **one-PR issues**:

- **One app per issue** (`tandem`, `rca`, `workflows`), or `utils` for code
  shared by two or more apps. Cross-app work becomes several issues.
- Each issue lists exact code paths (files, line refs, symbols), numbered
  required changes, testable acceptance criteria, the tests required, what's
  out of scope, the release impact, and the commands that verify it.
- Dependencies are explicit: `Blocked by #N`, created in merge order.

Humans can file the same thing by hand with the **Agent task** issue form
(`.github/ISSUE_TEMPLATE/agent-task.yml`); the sections are identical.

## 2. Build: `/beagle-complete-issue`

The skill ([`skills/beagle-complete-issue`](../skills/beagle-complete-issue/SKILL.md))
takes one issue (or the oldest unblocked `agent-ready` one) and:

1. Claims it (`in-progress`, assigned), branches `<type>/<N>-<slug>` from
   fresh `main`.
2. Reads the root and app `AGENTS.md`, the standards and `TESTING.md`.
3. Implements exactly the required changes and the required tests.
4. Runs `make check-<app>` and `make standards` (plus any e2e or manual
   verification the issue lists) until green — never by weakening a check.
5. Self-reviews the diff against each acceptance criterion.
6. Commits (Conventional Commits, scoped by app), pushes, and opens a PR
   from `.github/pull_request_template.md` with `Closes #N` and the issue's
   `app:*` and `release:*` labels.
7. Waits for CI (`gh pr checks --watch`), fixes failures (at most 3 attempts
   per failure), and squash-merges when `gate` is green.
8. Reports the release the merge cuts and watches the release run.

It **stops and asks** instead of guessing when the issue is ambiguous, when
finishing needs out-of-scope changes, or when a check won't go green.

## 3. Gate: CI

`.github/workflows/ci.yml` runs only the gates for the apps a PR touches
(shared tooling and `utils/` re-run everything). The `gate` job is the
single required check: **green means mergeable**. It runs the same scripts
as `make check-<app>`, so a green local run predicts a green CI run.

## 4. Release: per app, on merge

A merge to `main` releases **only the apps whose release-relevant paths
changed** (`<app>/**` except Markdown and `<app>/docs/**`, plus `utils/**`
and `rust-toolchain.toml`, which are compiled into every app). Docs-only
merges don't release.

- The version bump comes from the merged PR's label: `release:major`,
  `release:minor`, otherwise patch.
- Each app is tagged independently: `beagle-tandem-vX.Y.Z`,
  `beagle-rca-vX.Y.Z`, `beagle-workflows-vX.Y.Z`.
- Users install once (`brew install beagle-tools/tap/beagle-rca`,
  `brew install --cask beagle-tools/tap/beagle-tandem`, …). Desktop apps then
  update themselves from an **Update** button; CLIs update with
  `brew upgrade`.

Details — assets, the `beagle-<app>-latest` mirror, signing, the tap — are
in [`RELEASING.md`](RELEASING.md).

## Labels

Created by `scripts/github/labels.sh` (idempotent; rerun after editing it).

| Label                                    | Meaning                                                        | Set by                             |
| ---------------------------------------- | -------------------------------------------------------------- | ---------------------------------- |
| `app:tandem`, `app:rca`, `app:workflows` | The one app an issue/PR changes                                | create-issue, PR author            |
| `app:utils`                              | Shared `utils/` crate; a merge releases every app              | create-issue, PR author            |
| `release:major`                          | Breaking change: merge cuts `X+1.0.0`                          | create-issue; copied to the PR     |
| `release:minor`                          | New user-visible feature: merge cuts `X.Y+1.0`                 | create-issue; copied to the PR     |
| `release:patch`                          | Fix/refactor/internal: merge cuts `X.Y.Z+1` (also the default) | create-issue; copied to the PR     |
| `agent-ready`                            | Spec is complete; an agent may pick it up                      | create-issue or a human            |
| `in-progress`                            | Claimed; don't start it in parallel                            | complete-issue, removed on merge   |
| `blocked`                                | Waiting on another issue or on a human decision                | create-issue / complete-issue stop |

The **PR's** `release:*` label is what the release workflow reads. If it's
wrong, fix the label before merging — after the merge, the version is cut.

## Who does what

| Step                 | Humans                                                                    | Agents                                                  |
| -------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------- |
| Decide what to build | ✅ describe the feature, answer blocking questions                        | ask only blocking questions                             |
| Write issues         | review/edit the proposed list                                             | investigate code, write and create the issues           |
| Implement + test     | optional (same flow by hand)                                              | branch, code, tests, local gate                         |
| Review               | review PRs when branch protection requires it; can always request changes | self-review against acceptance criteria                 |
| Merge                | approve if required                                                       | merge only on green `gate`, never `--admin` unless told |
| Release              | nothing — automatic; fix failed releases via a new issue                  | report the release and watch the run                    |
| Standards/CI changes | own them (a PR to `standards/`, `scripts/`, workflows)                    | never weaken a check to get green                       |

## Doing it by hand

The skills are a convenience, not a requirement. By hand: file an Agent task
issue, branch `<type>/<N>-<slug>` from `main`, run `make check-<app>` and
`make standards`, open a PR with the template and labels, merge on green.
The release follows automatically either way.
