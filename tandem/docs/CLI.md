# tandem-cli

Tandem keeps everything in one SQLite file with no server in front of
it, so "read the file" is the integration path for another script or
agent. `tandem-cli` is that path with the locations, table names and
JSON digging already done.

It opens the database **read-only**. A tool that can only read can't
corrupt a review you're in the middle of, and Tandem's WAL mode means
you don't have to close the app first. The one exception is `review`,
which appends a request to the review queue (below).

Run with **no command**, it opens the [review TUI](#the-review-tui): a
terminal review queue with its own store, where an agent reviews the
PRs you queue and you triage the result.

The code is a library crate (`crates/tandem-cli`, `tandem_cli::run`)
with a thin `main.rs`, so the same commands are also linked into the
unified `beagle` command as **`beagle tandem`** (`tandem_cli::run_as`):
`beagle tandem prs`, `beagle tandem queue`, … behave exactly like
`tandem-cli prs`, `tandem-cli queue`, ….

## Getting it

It ships inside the app. The Homebrew cask puts it on your PATH:

```bash
brew install --cask beagle-tools/tap/beagle-tandem
tandem-cli paths
```

On macOS it lives at `Tandem.app/Contents/MacOS/tandem-cli`; the Linux
packages and the Windows installer put it next to the app's own
executable. No standalone binary is published; to run it without the
app, build it from source:

```bash
cd tandem && cargo build --release -p tandem-cli   # target/release/tandem-cli
```

Or install the `beagle` command (`brew install beagle-tools/tap/beagle`,
see `cli/README.md` at the repo root) and use `beagle tandem …`.

## Commands

```bash
tandem-cli                            # the review TUI (below)
tandem-cli paths                      # database, run logs, settings, the TUI's store
tandem-cli prs [--repo owner/name] [--limit 50]
tandem-cli comments --repo owner/name --pr 123 [--threads]
tandem-cli summary  --repo owner/name --pr 123
tandem-cli runs [--repo owner/name [--pr 123]] [--limit 20]
tandem-cli queue [--all]
tandem-cli review --repo owner/name --pr 123 [--agent name] [--by who]
tandem-cli update [--check]

# the review TUI's queue (its own store; see "The review TUI")
tandem-cli add <pr>... [--by who] [--no-kick]
tandem-cli list
tandem-cli claim --out <dir> [--agent name]      # for the reviewer
tandem-cli ingest --run <id> --file <jsonl>      # for the reviewer
tandem-cli fail --run <id> --error <why>         # for the reviewer
```

The app-store commands print JSON on stdout (`--pretty` to indent), so it pipes
into `jq` or straight into another agent's context. Errors go to
stderr with exit code 1.

Global flags:

- `--db <path>` reads a specific database; `$TANDEM_DB` does the same.
- `--dev` reads the `make dev` store (`tandem-dev`) instead of the
  installed app's.

## Examples

Every unresolved local comment on a PR, as text an agent can read:

```bash
tandem-cli comments --repo acme/widgets --pr 123 \
  | jq -r '.[] | select(.status == "open") | "\(.path):\(.line) \(.author_name): \(.body)"'
```

What the agent concluded, if it wrote a summary:

```bash
tandem-cli summary --repo acme/widgets --pr 123 | jq -r '.body // "no summary yet"'
```

Which PRs an agent has already reviewed:

```bash
tandem-cli runs --limit 200 \
  | jq -r 'map(select(.purpose == "pr review")) | map("\(.repo_slug)#\(.pr_number)") | unique[]'
```

## Driving Tandem from a scheduled job

The point of `queue` and `review` is that **an outside process decides
what needs reviewing, and Tandem decides how to review it**, with the
agents, prompts and conventions you have configured. Nothing out there
starts an agent itself.

```bash
# what is waiting on me that nothing has reviewed yet?
tandem-cli queue

# ask Tandem to review one
tandem-cli review --repo acme/widgets --pr 123 --by nightly-job
```

A whole cron job is those two lines:

```bash
tandem-cli queue | jq -r '.[] | "\(.repo.owner)/\(.repo.name) \(.number)"' |
  while read -r repo number; do
    tandem-cli review --repo "$repo" --pr "$number" --by nightly-job
  done
```

`queue` is the review-request inbox (mirrored into the database each
time the app loads your "requested" list) minus anything an agent has
already finished reviewing and anything already queued, so running it
twice in a row does not review the same PR twice. A queued request that
failed to start doesn't count as queued. `--all` ignores the queued
filter.

`review` records a request and returns immediately. Tandem picks it up
within ~20 seconds while it is running, syncs the PR if it has never
been opened, and starts a normal review run, the same one the review
button starts. `--agent <name>` picks an agent; otherwise Tandem uses
its first configured one. Requests made while Tandem is closed are
picked up when it next opens. A request that can't start is marked with
its error (`review_jobs.error`) rather than retried.

## The review TUI

`beagle tandem` (or `tandem-cli`) with no arguments opens a terminal
review queue. You queue PRs; a reviewer — normally an agentd job —
claims each one and has an agent review it into **local** comments, a
summary and a verdict; you go through them in the terminal and post
what you choose.

```text
add / `a` ──▶ queued ──claim──▶ reviewing ──ingest──▶ ready ──`m`──▶ done
                                    └──fail──▶ failed
```

- **Left:** every queued PR and where it stands (`queued`, `reviewing`,
  `ready`, `failed`, `done`) with the verdict as a badge.
- **Right:** the selected PR in three tabs — **Summary** (the verdict
  first), **Diff** (your own `delta --side-by-side` when it is
  installed, a plain colored diff otherwise) and **Comments** (each
  with the agent's local history reply under it).
- **GitHub writes only on a confirmed keypress:** `P` posts the
  selected comment (a one-line fix on the new side becomes a GitHub
  suggestion), `A` approves. `x` dismisses a comment, `m` marks the PR
  gone-through, `a` queues another, `e` reviews it again, `r` refreshes
  from GitHub, `?` lists every key.

The TUI keeps **its own store**, `<data dir>/tandem-tui/cache.sqlite3`
(`$TANDEM_TUI_DB` moves it), not the desktop app's: the app drains its
own review queue, and sharing a file would have it race the reviewer
for every PR you queue. `--db` and `--dev` never apply to it;
`tandem-cli paths` shows it under `review_tui`.

### Config

All optional, in `~/.config/tandem-tui/config.yaml`
(`$TANDEM_TUI_CONFIG` moves it). Unknown keys are an error, and so is a
file that doesn't parse: silently ignoring it could post as the wrong
account.

```yaml
# The gh account reads, posts and approvals use (`gh auth token -u`).
# Omit to use gh's active account.
github_user: octocat
# Run after a PR is queued, so the review starts now rather than on the
# reviewer's next scheduled tick. Omit to only queue.
kick: [agentctl, run, demo/review-queue]
```

GitHub access goes through the `gh` CLI: log in with `gh auth login`
(with `github_user` set, that account must be one of gh's logged-in
accounts).

### The reviewer

Anything that runs these three commands can be the reviewer; the
example is an agentd job, `workflows/manifests/demo/review-queue.yaml`
(with `review-pr.yaml` and `jobs/review-queue/job-purpose.md`):

1. `tandem-cli claim --out <dir>` takes the oldest queued PR, fetches
   it (detail, comments, diff) into the TUI's store, writes
   `<dir>/context.md` and prints `<run id> <context path>`. It prints
   nothing when the queue is empty. A PR it can't fetch is marked
   failed, and the next one is tried.
2. An agent reads `context.md` — the PR, everything already said on it,
   the diff and the exact output contract — and writes JSON lines: one
   `tandem_verdict` (`mergeable`, `mergeable-with-suggestions`,
   `mergeable-with-unknowns` or `not-mergeable`), one `tandem_summary`
   (unless an earlier review wrote one), and one `tandem_comment` per
   comment, each with a `context` that stays local.
3. `tandem-cli ingest --run <id> --file <jsonl>` checks every line and
   loads nothing if one is wrong, printing what to fix; the agent fixes
   the file and runs it again. A reviewer that gives up runs
   `tandem-cli fail --run <id> --error <why>`, which the TUI shows. A
   review still running after 45 minutes shows as failed.

The context names the program that claimed it (`beagle tandem` or
`tandem-cli`) in the `ingest` line it tells the agent to run. Nothing in
this loop writes to GitHub.

## Staying current

```bash
tandem-cli update --check   # what you have, what's out, whether you're behind
tandem-cli update           # do whatever updating means for this install
```

`--check` compares this build's version with the `latest.json` on the
`beagle-tandem-latest` release (the same file the in-app updater
reads). What `update` does depends on how this copy got here:

| Installed as                   | `update` does                                                                                |
| ------------------------------ | -------------------------------------------------------------------------------------------- |
| inside the app, via Homebrew   | `brew upgrade --cask beagle-tandem`                                                          |
| inside the app, some other way | tells you to use the app's own updater; a CLI shouldn't swap out a running app               |
| standalone build (from source) | tells you to rebuild, or to install the app                                                  |
| `beagle tandem`                | not a command (hidden; exit 2): run `beagle update`, which updates the whole `beagle` binary |

The version it reports is the release it shipped in (release CI stamps
it with `scripts/set-version.mjs`). A build made outside the release
pipeline carries the workspace placeholder and reads as "behind" — by
design: claiming you're current when we can't tell is worse.

## Notes

- **Don't write review data from outside.** The queue above is the
  seam built for it; everything else is Tandem's. Tandem holds its own
  connection and emits events when data changes; an outside write
  lands silently and the app won't know. Something that needs to push
  data _in_ belongs behind a Tauri command.
- **Columns are stable; the JSON blobs grow.** Fields are added
  additively (serde defaults), and schema changes go through
  append-only migrations tracked in `PRAGMA user_version`. Check that
  number if your script cares. The queue tables arrive in migration 5,
  the review TUI's `pr_marks` in migration 6.
- Agent run logs are plain JSONL under `runs/<run_id>/events.jsonl` if
  you want what an agent _did_ rather than what it concluded.
