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

```sh
make install    # installs the three binaries and restarts the daemon
agentctl get agents
agentctl -j hello -p name=you
agenttop
```

The daemon runs under launchd from `deploy/com.agentd.daemon.plist`.

## Development

Quality gate (what CI runs), from the repo root: `make check-workflows`.
Architecture and invariants for contributors and coding agents:
[AGENTS.md](AGENTS.md).
