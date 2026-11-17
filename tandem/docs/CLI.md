# tandem-cli

Tandem keeps everything in one SQLite file with no server in front of
it, so "read the file" is the integration path for another script or
agent. `tandem-cli` is that path with the locations, table names and
JSON digging already done.

It opens the database **read-only**. A tool that can only read can't
corrupt a review you're in the middle of, and Tandem's WAL mode means
you don't have to close the app first. The one exception is `review`,
which appends a request to the review queue (below).

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
tandem-cli paths                      # database, run logs, settings
tandem-cli prs [--repo owner/name] [--limit 50]
tandem-cli comments --repo owner/name --pr 123 [--threads]
tandem-cli summary  --repo owner/name --pr 123
tandem-cli runs [--repo owner/name [--pr 123]] [--limit 20]
tandem-cli queue [--all]
tandem-cli review --repo owner/name --pr 123 [--agent name] [--by who]
tandem-cli update [--check]
```

Everything prints JSON on stdout (`--pretty` to indent), so it pipes
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

## Staying current

```bash
tandem-cli update --check   # what you have, what's out, whether you're behind
tandem-cli update           # do whatever updating means for this install
```

`--check` compares this build's version with the `latest.json` on the
`beagle-tandem-latest` release (the same file the in-app updater
reads). What `update` does depends on how this copy got here:

| Installed as                   | `update` does                                                                  |
| ------------------------------ | ------------------------------------------------------------------------------ |
| inside the app, via Homebrew   | `brew upgrade --cask beagle-tandem`                                            |
| inside the app, some other way | tells you to use the app's own updater; a CLI shouldn't swap out a running app |
| standalone build (from source) | tells you to rebuild, or to install the app                                    |
| `beagle tandem`                | refuses and points at `beagle update`, which updates the whole `beagle` binary |

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
  number if your script cares. The queue tables arrive in migration 5.
- Agent run logs are plain JSONL under `runs/<run_id>/events.jsonl` if
  you want what an agent _did_ rather than what it concluded.
