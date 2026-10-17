# AGENTS.md — Agentdesk (`workflows/desktop/`)

Read [`../AGENTS.md`](../AGENTS.md) (agentd itself) and the repo-wide
[`../../AGENTS.md`](../../AGENTS.md) and
[`../../docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first.
This file holds only what is specific to the desktop app.

**Agentdesk** is a macOS desktop view of the same agentd orchestrator
`agenttop` shows in the terminal. Tauri 2: a thin Rust shell over the
`agent-core` crate, with a React/TypeScript frontend.

It is a **second view, not a second system.** The daemon is the only
writer of run state; this app reads the same SQLite store. Anything that
would give Agentdesk its own copy of the truth — a cache, a sync protocol,
a parallel schema — is the wrong change.

## Rules specific to this app

1. **Never write to the store except through a user action.** Only
   `trigger_job` and `kill_run` write, and both fire on an explicit click.
   Nothing in a render path, an effect, or the poller may create or modify
   a run.
2. **Errors reach the user.** Every fallible command returns `AppError`
   with a message someone could act on, and it surfaces in the error bar —
   not only in a log.
3. **Paths from the frontend are untrusted.** `read_markdown` canonicalises
   and refuses anything outside agentd's state dir. Keep that check on any
   command that takes a path.

## Architecture map

- `src-tauri/src/lib.rs` — command registration (the IPC surface).
- `src-tauri/src/commands/*` — one file per domain. Validate, lock,
  delegate to `agent-core`, release.
- `src-tauri/src/state.rs` — `Mutex<Store>`. `agent_core::Store` is "one
  per thread, not `Sync`", so commands take the lock briefly and release.
- `src-tauri/src/poller.rs` — background thread with its OWN `Store`,
  hashing what the runs table displays every 2s and emitting
  `agentdesk://runs-updated` only on change.
- `src/lib/ipc.ts` — the only place command names appear.
- `src/lib/types.ts` — hand-mirrored from `agent-core`'s serde output.
  **No codegen**: a field added in Rust and not here is invisible, not a
  build error. Check this file when a value shows as `undefined`.
- `src/state/store.ts` — all app state and actions. Components never call
  `ipc` directly.
- `src/lib/graph.ts` — pure tidy-tree layout for the chain graph. No
  React, so the geometry is testable on its own.
- **Self-update** — `src/lib/updater.ts` (the only place the updater /
  process plugin APIs appear), `src/state/updates.ts` (its state, separate
  from agentd state), `UpdateButton.tsx` (top-bar pill: mount, every 30
  min, on focus) and `Versions.tsx` (version button + dialog to install any
  `beagle-workflows-v*` release, including rollbacks, via
  `src-tauri/src/commands/updates.rs`). Endpoint and pubkey live in
  `tauri.conf.json`; a failed background check is silent, a failed install
  goes to the error bar.
- `src/e2e/` — the Playwright IPC mock and fixtures, loaded only in
  `vite --mode e2e`; see [`../AGENTS.md`](../AGENTS.md#testing).

**The DAG is not computed here.** The lineage walk and the subtree-cost
rule live in `agent_core::lineage`, shared with agenttop; tmux session
handling lives in `agent_core::tmux`. Change them there, with tests, or
the views will quietly disagree.

## Patterns

- **Events, not timers.** The backend polls SQLite because the file has no
  change notification; the frontend never polls. If you need fresh data,
  emit from the poller.
- **Read is free, write is explicit.** `get_*`/`list_*` hit the store
  directly — it is WAL, local, and microseconds. No caching layer.

## Commands (from `workflows/desktop/`)

```sh
make install   # pnpm install
make dev       # debug build; window titled "Agentdesk Dev"
make build     # release .app + .dmg
pnpm e2e       # Playwright against the mocked frontend (no Tauri, no agentd)
make check     # the root `make check-workflows` gate
```

A dev build reads the same store as the release app — there is only one
agentd — so the window title is the only thing telling two open windows
apart. If you changed runtime behaviour, run `make dev` and exercise it.
