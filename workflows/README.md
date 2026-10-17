# agentd

Scheduled and on-demand AI coding agents on your own machine, run like
Kubernetes runs pods: declare agents in YAML manifests, and a small daemon
schedules them, runs each one in its own tmux session, and records every run
in a local SQLite store.

| Tool | What it does |
|---|---|
| `agentd` | The daemon: reconciles manifests, fires cron schedules, spawns and watches runs, sends macOS notifications |
| `agentctl` | kubectl-style CLI: `apply`, `get`, `run`, `logs`, `kill`, `attach`, and `-j <job> -p k=v` |
| `agenttop` | Live terminal dashboard: watch runs, read logs, attach, kill |
| Agentdesk | The same view as a macOS app — see [desktop/README.md](desktop/README.md) |

## A manifest

```yaml
kind: Agent
name: hello
runner: claude          # claude | codex | script
model: haiku
schedule: "0 9 * * 1-5" # optional; omit for manual-only
workdir: ~
prompt: prompts/hello.md
timeout_minutes: 5
```

One directory per project; see [`manifests/`](manifests/) for examples,
including a DAG where a script stage fans out to Claude and Codex children.

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
agentctl -j hello -p name=you
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
