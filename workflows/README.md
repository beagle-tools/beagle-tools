# agentd

Scheduled and on-demand AI coding agents on your own machine, run like
Kubernetes runs pods: declare agents in YAML manifests, and a small daemon
schedules them, runs each one in its own tmux session, and records every run
in a local SQLite store.

| Tool | What it does |
|---|---|
| `agentd` | The daemon: reconciles manifests, fires cron schedules, spawns and watches runs, sends macOS notifications |
| `agentctl` | kubectl-style CLI: `apply`, `get`, `run`, `logs`, `kill`, `attach`, `resume`, `errors`, and `-j <job> -p k=v` |
| `agenttop` | Live terminal UI, jobs → runs → logs: watch, run, resume, attach, kill |
| Agentdesk | The same view as a macOS app — see [desktop/README.md](desktop/README.md) |

## A manifest

```yaml
kind: Agent
name: hello
runner: claude          # claude | codex | script
model: haiku
schedule: "0 9 * * 1-5" # optional; omit for manual-only
timezone: local         # optional: read the schedule on this Mac's clock
                        # (DST included); default utc
workdir: ~
prompt: prompts/hello.md
timeout_minutes: 5
max_concurrent: 2       # optional: at most this many runs of THIS agent
triggers: [report]      # optional: the agents (same project) it may start —
                        # documentation the views draw, not enforcement
```

One directory per project; see [`manifests/`](manifests/) for examples,
including a DAG where a script stage fans out to Claude and Codex children.

On top of `max_concurrent`, agentd caps the whole machine: at most 5 model
runs (claude + codex) and 20 script runs live at once, across every project.
Over the cap a run stays `pending` and starts, oldest first, when a slot
frees (`crates/agentd/src/caps.rs`; no manifest can raise it).

### Every job explains itself: `job-purpose.md`

A job — scheduled, declared `entrypoint: true`, or not triggered by any
sibling — must have `<project>/jobs/<job>/job-purpose.md`, or it **does not
load** (`agentctl apply` reports it). Steps need none; their job's file
covers them. Sections: `# <project>/<job>`, `## How it starts`, `## Steps`,
`## Where things live`, `## Known failure modes`, `## Rules a repair must
keep`, `## Verify a fix` (see [`manifests/demo/jobs/`](manifests/demo/jobs/)).
agentd puts the opening sections (what the job is for, how it starts, its
steps) at the top of every model step's prompt, with the path to the whole
file; scripts get the path as `$AGENTD_JOB_PURPOSE`.

### When something breaks

```sh
agentctl errors                          # what broke in the last 48h, deduplicated
agentctl errors --since 2026-10-27T12:00:00Z --json
agentctl errors --since-last <project>/<agent> --write DIR
agentctl resume <run-id>                 # re-run a failed step in its chain
```

`errors` lists failed and timed-out runs plus `ERROR` lines in runs that
carried on; the same failure in the same step is one entry with a count.
Logs are pruned after 4 hours, but a run's state, result and `ERROR` lines
stay in the store for a week, and so does its data directory (the handover
between steps). That week is what makes `resume` work: a new run of the
step with the same parent, params and model, which joins the same chain.
agenttop's `R` and Agentdesk's Restart… do the same, after a confirmation.

## Configuration

Two optional files in `~/.config/agentd/` (`$AGENTD_CONFIG_DIR` moves both).
Unknown keys are ignored, so a newer file still loads.

```yaml
# ui.yaml — how agenttop and Agentdesk show things
ticket_url: https://tracker.example.com/issue/{key}   # where a `ticket` param links
investigations_dir: ~/notes/investigations           # <dir>/<run id>/OUTPUT.md write-ups
hidden_projects: [demo]                               # left off the jobs screens (H shows them)
```

```yaml
# agentd.yaml — the daemon
protected_orgs: [acme]   # GitHub orgs whose main/master no run may push to
```

**Git guard.** Every run gets agentd's `git` shim first on `PATH` and
`core.hooksPath` pointed at agentd's hook dispatcher (both written to
`~/.local/state/agentd/lib/` on startup). The shim always refuses
`push --no-verify` and overriding `core.hooksPath`; the `pre-push` hook
refuses a push to `main`/`master` on any of `protected_orgs`
(`$AGENTD_PROTECTED_ORGS` overrides the file; none by default). Every hook
then runs the repo's own hook, so nothing is skipped. When a model run
finishes, its log notes the repos and branches it wrote to
(`repos touched: acme/widgets @ abc-1-fix (commit ×2, push)`), flagged
`⚠ PUSHED TO MAIN` if one got past the guard.

agentd also writes a heartbeat every tick (`~/.local/state/agentd/agentd.heartbeat`);
`agentctl status`, agenttop and Agentdesk show a red banner when the daemon
is stopped or stalled. Runs keep the Mac awake (`caffeinate -i`) while they
run, and get `~/.cargo/bin` and `~/.local/bin` on `PATH`.

## Install

macOS only (the daemon runs under launchd and runs agents in tmux).

### Homebrew

```sh
brew install beagle-tools/tap/beagle-workflows          # agentd, agentctl, agenttop
brew install --cask beagle-tools/tap/beagle-agentdesk   # the Agentdesk app (optional)
brew install tmux terminal-notifier                     # runs live in tmux; notifier = icons
```

### Direct download

Every release refreshes the rolling
[`beagle-workflows-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-workflows-latest)
release, whose assets keep stable names:

| Asset | What |
|---|---|
| `beagle-workflows-aarch64-apple-darwin.tar.gz` (+ `.sha256`) | CLIs, Apple silicon |
| `beagle-workflows-x86_64-apple-darwin.tar.gz` (+ `.sha256`) | CLIs, Intel |
| `Agentdesk-macos-arm64.dmg` / `Agentdesk-macos-x64.dmg` | the app |

Each tarball holds `agentd`, `agentctl` and `agenttop` at its root; put them
together in a directory on your `PATH`. Versioned releases are tagged
`beagle-workflows-vX.Y.Z`.

### Start the daemon

The binaries carry everything the daemon setup needs (launchd plist
template, notification icons, notifier build script), so this works the
same for a brew, tarball or source install:

```sh
agentctl daemon install   # writes ~/Library/LaunchAgents/com.agentd.daemon.plist
                          # for the agentd next to agentctl, unpacks the icons,
                          # builds the branded notifiers (if terminal-notifier
                          # is installed) and loads the daemon
agentctl daemon install --print   # just show the plist it would write
```

Then give it manifests (see [`manifests/`](manifests/) for examples) and
check it sees them:

```sh
mkdir -p ~/.config/agentd/manifests   # one subdirectory per project
cp -R manifests/demo ~/.config/agentd/manifests/   # from a checkout of this repo
agentctl apply
agentctl get agents
agentctl -j fanout
agenttop
```

Daemon logs: `~/Library/Logs/agentd/agentd.{out,err}.log`.
`agentctl daemon uninstall` stops it and removes the unit (state and logs
stay).

### Updating

- **CLIs:** `brew upgrade beagle-workflows`, **then `agentctl daemon restart`**.
  A running daemon keeps the old binary; manifests are parsed strictly, so
  an old daemon silently drops agents that use newer fields.
- **Agentdesk** updates itself: an *Update to vX.Y.Z* pill appears in the
  top bar when a release is out (checked at launch, every 30 minutes and on
  focus). The version button beside it lists every release and can install
  any of them, including rolling back. Updates are signature-checked.

### From source

```sh
make install          # cargo install the three binaries + restart the daemon
make install-daemon   # first time only: agentctl daemon install
```

## Development

Quality gate (what CI runs), from the repo root: `make check-workflows`.
Architecture and invariants for contributors and coding agents:
[AGENTS.md](AGENTS.md).
