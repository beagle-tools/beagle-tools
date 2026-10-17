# AGENTS.md — workflows (agentd)

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first. This file
holds only what is specific to `workflows/`.

## What it is

A local orchestrator for scheduled and on-demand AI coding agents, modelled
on Kubernetes:

| Kubernetes | Here | Meaning |
|---|---|---|
| Namespace | project (a directory of manifests) | A group of related agents |
| Workload | agent (`AgentSpec`, one YAML file) | Prompt + runner + schedule |
| Pod | run (`Run`) | One execution of an agent |
| Container | tmux session `agentd-<run id>` | The process doing the work |

Four tools share one state store:

- **`agentd`** — the daemon ("kubelet"). Each tick: reconcile manifests into
  the store, schedule due cron runs, spawn pending runs in tmux, monitor
  running ones, prune expired logs.
- **`agentctl`** — kubectl-style CLI: `apply`, `get`, `run`, `kill`, `status`,
  `attach`, `logs`, and `-j <job> -p k=v` to start a job with checked params.
- **`agenttop`** — btop-style live TUI: watch, attach, kill, read logs.
- **Agentdesk** (`desktop/`) — the Tauri desktop view of the same store; see
  [`desktop/AGENTS.md`](desktop/AGENTS.md).

`scripts/agentman` (bash) wires which manifest roots the machine uses;
`scripts/build-notifier.sh` builds the branded notifier apps.

## Layout

```
crates/agent-core   shared lib: types, manifest loading, Store (SQLite),
                    lineage/pipelines, transcript prettifying, paths, tmux
crates/agentd       the daemon (reconciler, scheduler, spawner, monitor,
                    retention, notify, runscript/runenv = the shell a run gets)
crates/agentctl     the CLI
crates/agenttop     the TUI (ratatui)
desktop/            Agentdesk — its own cargo workspace + pnpm package
manifests/          example projects (demo/, personal/) — real manifests
                    usually live elsewhere and are linked in by agentman
deploy/             launchd plist template for the daemon
assets/notify/      notification icons; one notifier app is built per png
```

## Invariants

- **The daemon is the single writer of run state.** agentctl, agenttop and
  Agentdesk only create pending runs and mark runs Killed — explicit user
  actions — and never talk to the daemon directly. Everything else (start,
  finish, usage, events) is written by agentd.
- **State lives in SQLite** at `$AGENTD_STATE_DIR` →
  `$XDG_STATE_HOME/agentd` → `~/.local/state/agentd` (`state.db`, plus
  `logs/`, `worktrees/`, `data/<run id>/` for passing data between DAG
  stages). WAL mode: one writer, many readers; one `Store` per thread.
- **Manifests are the desired state.** Default root
  `~/.config/agentd/manifests` (override `$AGENTD_MANIFESTS_DIR`); each
  subdirectory is a project with an optional `project.yaml` and one YAML per
  agent. Paths in a manifest resolve relative to it; `~` expands.
- **Manifests use `serde(deny_unknown_fields)`.** A manifest using a field
  the running daemon predates is rejected and its agent silently dropped from
  the store on the next tick. So `make install` always restarts the daemon
  (`make restart-daemon`); never install without it.
- **tmux sessions are targeted by exact name** (`=name`) through
  `agent_core::tmux` — tmux's default prefix match can hit the wrong run.
- **Killing the daemon is safe.** Runs live on in tmux; the startup recovery
  pass finalises anything that ended while it was down. Log events written
  during the gap are not replayed into the event feed.

## Concurrency model

Synchronous throughout: no async runtime. agentd is a single-threaded loop
(`run_tick` every `--tick-seconds`, default 5). Concurrency between runs comes
from tmux processes, not threads; per-agent `max_concurrent` and
`concurrency: forbid|allow` gate how many run at once. Agentdesk adds one
poller thread with its own `Store`.

## Commands (from `workflows/`)

```sh
make install          # cargo install agentd/agentctl/agenttop + restart daemon
make restart-daemon   # launchctl kill + kickstart com.agentd.daemon, verified
make check            # delegates to the repo root's `make check-workflows`
```

Deploying the daemon: `deploy/com.agentd.daemon.plist` is a template
(`__CARGO_BIN_DIR__`, `__LOG_DIR__` placeholders) for
`~/Library/LaunchAgents`. `KeepAlive` only revives a crashed daemon, so a
clean exit stays down until kickstarted. Daemon logs:
`~/Library/Logs/agentd/agentd.{out,err}.log`.

Notifications prefer `terminal-notifier` (icons) and fall back to
`osascript`. macOS only shows the sending app's own icon, so run
`scripts/build-notifier.sh` after adding a png to `assets/notify/`.
