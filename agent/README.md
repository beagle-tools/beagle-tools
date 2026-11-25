# beagle agent (agentd)

Your own pack of AI coding agents, kept on a short leash on your own Mac.

You write down what an agent should do — a prompt, which CLI runs it
(Claude Code, Codex, or a plain shell script), where it works and when —
in a small YAML manifest. A daemon, **agentd**, picks the manifests up,
starts runs on a cron schedule or when you ask, runs each one in its own
tmux session, and keeps every run in a local SQLite store. You watch the
pack from a terminal dashboard or from **Agentdesk**, the macOS app.

It borrows its shape from Kubernetes:

| Kubernetes | agentd | Meaning |
|---|---|---|
| Namespace | project (a directory of manifests) | A group of related agents |
| Workload | agent (one YAML file) | Prompt + runner + schedule |
| Pod | run | One execution of an agent |
| Container | tmux session `agentd-<run id>` | The process doing the work |

## Why you'd want it

- **Agents on a schedule.** "Every weekday at 9, triage new issues" is
  one line of cron, read on your Mac's clock if you like.
- **Chains, not just one-shots.** A cheap script step can look at the
  world and fan out to Claude and Codex steps, handing them files through
  a shared data directory. The views draw the whole chain and what it
  cost.
- **You can always look.** Every run is a real tmux session: attach to
  it, read its log live, kill it. Nothing runs in the dark.
- **Guard rails by default.** Runs cannot push to `main`, skip hooks,
  merge PRs or approve them unless a manifest says so
  ([details](docs/configuration.md#safety-rails)).
- **Built to be repaired.** Every job carries a `job-purpose.md`, failures
  are grouped by `beagle agent errors`, and a failed step can be resumed
  in place for a week.
- **Local.** One SQLite file, one daemon, your own CLIs and logins. No
  service to sign up for.

## The pieces

Everything is one command, `beagle agent`, in the unified `beagle` binary
([`../beagle/`](../beagle/README.md)):

| Command | What it does |
|---|---|
| `beagle agent` (or `beagle agent top`) | The live dashboard: jobs → runs → logs. Watch, run, resume, attach, kill. |
| `beagle agent <cmd>` | kubectl-style CLI: `apply`, `get`, `run`, `logs`, `kill`, `attach`, `resume`, `errors`, `status`, and `-j <job> -p k=v`. |
| `beagle agent daemon …` | The daemon: `run` it in the foreground, or `install` / `restart` / `uninstall` it under launchd. |
| Agentdesk | The same store in a macOS window — see [desktop/README.md](desktop/README.md). |

## Install

agentd is built for macOS: the daemon runs under launchd and keeps the
Mac awake with `caffeinate`. (`beagle agent daemon install` is
macOS-only; elsewhere you would run `beagle agent daemon run` under your
own service manager, which is not a tested setup.)

### Homebrew (recommended)

```sh
brew install beagle-tools/tap/beagle                    # the beagle command (beagle agent …)
brew install --cask beagle-tools/tap/beagle-agentdesk   # the Agentdesk app (optional)
```

### Direct download

The `beagle` tarballs are described in
[`../beagle/README.md`](../beagle/README.md). Agentdesk's DMGs live on the
rolling
[`beagle-workflows-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-workflows-latest)
release, under stable names: `Agentdesk-macos-arm64.dmg` and
`Agentdesk-macos-x64.dmg`. Versioned releases are tagged
`beagle-workflows-vX.Y.Z`.

### From source

From a checkout of this repo (needs a Rust toolchain; the repo pins it in
`rust-toolchain.toml`):

```sh
cargo install --locked --path beagle    # the unified beagle command
```

Agentdesk from source: `cd agent/desktop && make install && make build`
(the `.app` and `.dmg` land under `agent/desktop/target/release/bundle`).
Node, pnpm and the rest of the toolchain are covered in
[`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md); setting up a whole
machine is [`../docs/GETTING_STARTED.md`](../docs/GETTING_STARTED.md).

## Set it up

### 1. Prerequisites

| Tool | Needed for | Install |
|---|---|---|
| `tmux` | every run lives in a tmux session | `brew install tmux` |
| `claude` (Claude Code), signed in | `runner: claude` agents | see Claude Code's docs; then run `claude` once to sign in |
| `codex`, signed in | `runner: codex` agents | see Codex's docs |
| `terminal-notifier` | optional: notifications with icons (otherwise plain `osascript` ones) | `brew install terminal-notifier` |
| `gh`, signed in | optional: agents that read GitHub | `brew install gh && gh auth login` |

You only need the runners your agents use: script-only agents need
nothing but tmux. `beagle doctor` checks tmux, `gh` and old binaries for
you.

The daemon runs under launchd, whose `PATH` is short: the `beagle`
binary's own directory, `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`
and `/bin`, plus `~/.cargo/bin` and `~/.local/bin` for runs. A `claude` or
`codex` installed anywhere else (say, an `nvm`-managed npm prefix) will
not be found — link it into `~/.local/bin`.

### 2. Start the daemon

```sh
beagle agent daemon install          # write the launchd unit and start agentd
beagle agent daemon install --print  # or: just show the unit it would write
beagle agent status                  # "DAEMON  agentd running" means you're good
```

`daemon install` writes `~/Library/LaunchAgents/com.agentd.daemon.plist`
running `<beagle> agent daemon run`, unpacks the notification icons,
builds the branded notifier apps (when `terminal-notifier` is installed)
and loads the unit. Everything it needs is built into the binary, so it
works the same for a brew, tarball or source install.

### 3. Give it manifests

agentd reads `~/.config/agentd/manifests/`: one subdirectory per project,
one YAML file per agent. The demo project in
[`examples/demo/`](examples/demo/) is the quickest start. Its scripts
expect to live at `~/.config/agentd/manifests/demo/`, so copy it there
from a checkout of this repo:

```sh
mkdir -p ~/.config/agentd/manifests
cp -R agent/examples/demo ~/.config/agentd/manifests/
beagle agent apply          # load the manifests; prints a WARN for anything wrong
beagle agent get jobs       # what you can run, and the params each takes
```

The daemon also reloads manifests on every tick, so `apply` is mostly for
seeing problems straight away.

The demo's jobs:

| Job | What it does | Costs |
|---|---|---|
| `writeup` | A script writes a sample write-up and starts a second script step. | nothing — no model |
| `fanout` | A script records `df -h /`, then starts a Claude step and a Codex step in parallel that each explain it in one sentence. | a little; needs `claude` and `codex` |
| `review-queue` | Claims a PR queued with `beagle tandem` and hands it to a Claude reviewer. | needs `beagle tandem` set up |

## Your first job

Start with the free one:

```sh
beagle agent -j writeup        # prints the new run id, and how to follow it
beagle agent                   # the dashboard: Enter on writeup, pick a step, w for its write-up
```

Then a real model chain:

```sh
beagle agent -j fanout
beagle agent logs demo/report-claude -f   # follow that step's latest run
```

A run starts within a tick (5 seconds) of being queued. Other ways to
start things:

```sh
beagle agent run demo/fanout                  # a run by <project>/<agent>
beagle agent -j fanout -m sonnet              # override the model once
beagle agent -j triage -p ticket=ABC-123      # a job with declared params
```

### Writing your own

```yaml
# ~/.config/agentd/manifests/hello/hello.yaml
kind: Agent
name: hello
runner: claude          # claude | codex | script
model: haiku
schedule: "0 9 * * 1-5" # optional; omit for manual-only
timezone: local         # optional: read the schedule on this Mac's clock; default utc
workdir: ~
prompt: prompts/hello.md
timeout_minutes: 5
```

Because `hello` is a job (nothing else starts it), it also needs
`~/.config/agentd/manifests/hello/jobs/hello/job-purpose.md` — a short
page saying what the job is for, how it starts, its steps and how it
fails. Without it the agent **does not load**, and `beagle agent apply`
tells you so. Copy one from [`examples/demo/jobs/`](examples/demo/jobs/)
to start.

Every field, declared params, the environment a run sees and how to build
a chain: **[docs/manifests.md](docs/manifests.md)**.

## Watching and steering

| Command | What it does |
|---|---|
| `beagle agent` | The dashboard. `?` lists its keys. |
| `beagle agent status` | Daemon health, agents per project, runs per state, the latest runs. |
| `beagle agent get projects\|jobs\|agents\|runs` | Lists. `get runs` takes `-p PROJECT`, `-a AGENT`, `--state STATE`, `-n LIMIT`. |
| `beagle agent logs <run id \| project/agent> [-f] [--kind K]` | A run's events; `-f` follows it to the end. |
| `beagle agent attach <run id>` | Jump into the run's tmux session (detach with `ctrl-b d`). |
| `beagle agent kill <run id>` | Stop a run. |
| `beagle agent resume <run id>` | Re-run a failed, timed-out or killed step in its chain. |
| `beagle agent errors` | What broke in the last 48 hours, deduplicated. |

The dashboard uses the same keys as every beagle TUI (`Enter` open,
`Esc`/`b` back, `/` search, `r` reload, `E` edit the manifest, `y`/`Y`/`i`
copy), plus `R` run or resume, `x` kill, `t` attach, `w` a step's
write-up, `m` its details and links, `f` running only, `L` every job's
runs on one page, `W` follow the newest step and `H` show hidden
projects.

## When something breaks

```sh
beagle agent errors                                     # the last 48h, grouped
beagle agent errors --since 2026-10-27T12:00:00Z --json
beagle agent errors --since-last <project>/<agent> --write DIR
beagle agent resume <run-id>                            # re-run a failed step in its chain
```

`errors` lists failed and timed-out runs plus `ERROR` lines in runs that
carried on; the same failure in the same step is one entry with a count.
Logs are pruned after 4 hours, but a run's state, result and `ERROR` lines
stay in the store for a week, and so does its data directory (the handover
between steps). That week is what makes `resume` work: a new run of the
step with the same parent, params and model, which joins the same chain.
The dashboard's `R` and Agentdesk's Restart… do the same, after a
confirmation.

### Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| Runs sit `pending` forever | The daemon is down or stalled: `beagle agent status` shows a red banner. `beagle agent daemon restart`, then check `~/Library/Logs/agentd/agentd.err.log`. Otherwise you are at a cap: the agent's `max_concurrent`, or the machine-wide 5 model / 20 script runs. |
| `agent … not found in the store` / a job vanished | Run `beagle agent apply` and read its `WARN` lines: usually a missing `job-purpose.md`, a YAML typo, or a field the running daemon is too old for. |
| An agent "disappears" right after an upgrade | The old daemon is still running and drops manifests that use newer fields. Always `beagle agent daemon restart` after `beagle update`. |
| `job "x" is missing required parameter(s)` | Pass them with `-p`; `beagle agent get jobs` lists what each job takes. |
| `job "x" needs MCP server(s) that are not ready` | The job declares `requires_mcp`. Run `claude`, then `/mcp`, and sign in to each server; check with `claude mcp get <name>`. A server it does not know at all needs `claude mcp add --scope user …` first. |
| A run fails at once with `command not found` | `claude`, `codex` or another tool is not on the daemon's `PATH` — see [Prerequisites](#1-prerequisites). |
| `cannot attach: run … is pending` | It has not started yet; wait a tick or check the daemon. |
| A push or `gh` call fails with `agentd: BLOCKED` / the pre-push guard | Working as intended: runs cannot push to `main`/`master` or write to GitHub on their own. See [safety rails](docs/configuration.md#safety-rails). |
| Notifications have no icon | `brew install terminal-notifier`, then `beagle agent daemon install` again. |
| `beagle doctor` flags `agentctl` / `agenttop` / `agentd` | Left over from the deprecated `beagle-workflows` formula: `brew uninstall beagle-workflows`, then `beagle agent daemon install` so the unit runs `beagle`. |

## Configuration

Two optional files in `~/.config/agentd/` (`$AGENTD_CONFIG_DIR` moves
both); unknown keys are ignored:

```yaml
# ui.yaml — how the dashboard and Agentdesk show things
ticket_url: https://tracker.example.com/issue/{key}   # where a `ticket` param links
investigations_dir: ~/notes/investigations           # <dir>/<run id>/OUTPUT.md write-ups
hidden_projects: [demo]                               # left off the jobs screens (H shows them)
```

```yaml
# agentd.yaml — the daemon (restart it after changing this)
protected_orgs: [acme]   # narrow the push guard to these orgs (default: every remote)
```

Where state and logs live, the daemon's flags, the caps and the guard
rails in full: **[docs/configuration.md](docs/configuration.md)**.

## Updating and removing

- **The `beagle` command and the daemon:** `beagle update`, **then
  `beagle agent daemon restart`**. A running daemon keeps the old binary,
  and since manifests are parsed strictly, an old daemon silently drops
  agents that use newer fields.
- **Agentdesk** updates itself: an *Update to vX.Y.Z* pill appears in the
  top bar when a release is out (checked at launch, every 30 minutes and
  on focus). The version button beside it lists every release and can
  install any of them, including rolling back. Updates are
  signature-checked.
- **Coming from the standalone `agentctl` / `agenttop` / `agentd`** (the
  deprecated `beagle-workflows` formula): `brew uninstall
  beagle-workflows && brew install beagle-tools/tap/beagle`, then
  `beagle agent daemon install` — the old unit points at the `agentd`
  binary. `agentctl <cmd>` is `beagle agent <cmd>`.
- **Removing:** `beagle agent daemon uninstall` stops the daemon and
  removes its unit. State (`~/.local/state/agentd/`), manifests and logs
  stay until you delete them.

## For coding agents

- Run jobs with `beagle agent -j <job> -p k=v`; it prints the run id on
  the first line. Follow with `beagle agent logs <id> -f`, or poll
  `beagle agent get runs -n 5`.
- Inside a run, start other runs with `$AGENTD_CTL run <project>/<agent>
  --parent "$AGENTD_RUN_ID"`, never a bare `agentctl` (a `beagle`-only
  install has none). Hand data over through `$AGENTD_DATA_DIR`.
- `beagle agent errors --json` is the machine-readable "what broke".
- A job's `jobs/<job>/job-purpose.md` says what it is for, its steps,
  known failure modes and how to verify a fix — read it before repairing
  one.
- Changing this code: read [AGENTS.md](AGENTS.md) (architecture and
  invariants) first.

## Repo layout

| Part | What it is |
|---|---|
| [`tui/`](tui/) | agentctl: the CLI, and what `beagle agent` runs |
| [`lib/top/`](lib/top/) | agenttop: the live dashboard (`beagle agent top`) |
| [`daemon/`](daemon/) | agentd: the scheduler daemon (`beagle agent daemon run`) |
| [`desktop/`](desktop/README.md) | Agentdesk, the macOS app |
| [`lib/core/`](lib/core/) | agent-core: types, manifests, the SQLite store |
| [`examples/demo/`](examples/demo/) | An example project of manifests |
| [`docs/`](docs/) | Manifest and configuration reference |

Also here: `deploy/` (the launchd plist template), `assets/notify/`
(notification icons) and `scripts/` (`agentman`, the notifier build,
release version stamping).

## Development

Quality gate (what CI runs), from the repo root: `make check-agent`.
Running things locally: [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md).
Releases: [`../docs/RELEASING.md`](../docs/RELEASING.md). Architecture and
invariants for contributors and coding agents: [AGENTS.md](AGENTS.md).
