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
  (`make restart-daemon`); never install without it. Brew installs restart
  with `agentctl daemon restart` (`make restart-daemon` only finds a
  `~/.cargo/bin` agentd).
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
make install-daemon   # first-time launchd setup: `agentctl daemon install`
make restart-daemon   # launchctl kill + kickstart com.agentd.daemon, verified
make check            # delegates to the repo root's `make check-workflows`
```

Deploying the daemon: `agentctl daemon install|restart|uninstall`
(`crates/agentctl/src/daemon.rs`, pure parts in `launchd.rs`). The binary
embeds `deploy/com.agentd.daemon.plist` (a template with `__AGENTD_BIN__`,
`__CARGO_BIN_DIR__`, `__LOG_DIR__` placeholders), `assets/notify/*.png` and
`scripts/build-notifier.sh`, so a brew or tarball install needs no
checkout — keep it that way: anything the setup reads from the repo at run
time breaks brew users. A Homebrew Cellar path is mapped to the stable
`<prefix>/bin` so the unit survives `brew upgrade`. `KeepAlive` only
revives a crashed daemon, so a clean exit stays down until kickstarted.
Daemon logs: `~/Library/Logs/agentd/agentd.{out,err}.log`.

Notifications prefer `terminal-notifier` (icons) and fall back to
`osascript`. macOS only shows the sending app's own icon, so after adding a
png to `assets/notify/` re-run `agentctl daemon install` (or
`scripts/build-notifier.sh`); a test fails until the new png is embedded.
agentd looks for its stock icons next to the manifests repo, then in
`<state dir>/assets/notify/` (where `daemon install` unpacks them).

## Testing

Strategy and rules: [`../docs/TESTING.md`](../docs/TESTING.md). Specific
to this app:

- **Rust:** unit tests beside the code; the crates' integration-style tests
  live in each crate's `src/tests/`. Anything touching launchd, tmux or
  the network stays a thin shell over a pure, tested function.
- **Agentdesk e2e** (`desktop/e2e/*.spec.ts`, `pnpm e2e`): Playwright drives
  the real React frontend in Chromium, served by `vite --mode e2e` on port
  1431. In that mode `src/main.tsx` dynamically imports
  `src/e2e/mockIpc.ts`, which answers every Tauri command from
  `src/e2e/fixtures.ts` — a small in-memory agentd (`trigger_job` enforces
  required params and queues a pending run; calls are recorded on
  `window.__agentdeskE2E.triggers`). `?update=X.Y.Z` makes the updater
  report an update. The import sits behind `import.meta.env.MODE === "e2e"`,
  which vite inlines, so production bundles contain no fixtures — keep any
  new mock code behind that same branch. A new IPC command needs a mock
  answer, or the specs touching it get `null`.
- `scripts/set-version.test.mjs` (node:test) runs from Agentdesk's `test`
  script.

## Releases

Merging to `main` with release-relevant changes under `workflows/` (not
`*.md` or `docs/`) cuts `beagle-workflows-vX.Y.Z` (patch by default;
`release:minor` / `release:major` PR labels bump more) — see
[`../docs/RELEASING.md`](../docs/RELEASING.md). One version covers all of
workflows: release CI runs `node scripts/set-version.mjs X.Y.Z` from
`workflows/`, which stamps both cargo workspaces' `[workspace.package]`
version, their `Cargo.lock` entries, and Agentdesk's `package.json` and
`tauri.conf.json` (never committed). The release ships:

- `beagle-workflows-<target>.tar.gz` (agentd, agentctl, agenttop) for
  aarch64/x86_64-apple-darwin → the `beagle-workflows` brew formula;
- `Agentdesk-macos-{arm64,x64}.dmg` plus signed updater bundles and
  `latest.json`, mirrored to the rolling `beagle-workflows-latest` release
  that Agentdesk's updater polls and the `beagle-agentdesk` cask installs.

Agentdesk's in-app version list (`src-tauri/src/commands/updates.rs`) shows
only `beagle-workflows-v*` releases that carry a `latest.json`.
