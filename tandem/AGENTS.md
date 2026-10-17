# Tandem — agent guide

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first; they hold
the repo-wide rules and the gate. This file only covers what is specific to
Tandem.

Tandem is a local-first PR review desktop app (Tauri 2: a Rust workspace
plus a React/TypeScript frontend). It caches everything about a GitHub PR
in SQLite, renders a differ, and runs coding agents (claude, codex, grok,
or a custom command) that leave **local** review comments for you to
triage. Deeper background: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
[`docs/SANDBOXING.md`](docs/SANDBOXING.md), [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Architecture map

| Path                   | What lives there                                                                                                                                                                                                                                        |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/tandem-core`   | Pure domain types (`github`, `review`, `agent`, `diff`), the unified-diff parser (`diff_parse`), `TandemError`. No I/O, nothing heavier than serde/chrono — every other crate speaks these types.                                                       |
| `crates/tandem-github` | GitHub REST/GraphQL client. Wire structs in `wire.rs`, HTTP in `client.rs`, auth strategies in `auth.rs`, discovery in `discovery.rs`, rate-limit backoff in `rate_limit.rs`, the user-only write half in `write.rs`.                                   |
| `crates/tandem-cache`  | SQLite. GitHub-data caching in `store.rs`; local-only review data in `review_store.rs` (+ `summaries.rs`, `revisions.rs`, `reviewed.rs`); merged-PR retention in `archive.rs`; migrations in `schema.rs`.                                               |
| `crates/tandem-agents` | Agent runners. argv per runner in `command.rs`, stdout classification + the comment contract in `events.rs`, prompt assembly in `context.rs` / `reply.rs` / `conversation.rs` / `conventions.rs` / `summary.rs`, process lifecycle in `runner.rs`.      |
| `src-tauri`            | Thin Tauri shell. One file per domain under `src/commands/`; `state.rs` (`AppState`), `settings.rs` (settings + `AppDirs`), `file_config.rs` (optional `tandem.yaml`). Commands validate, lock state briefly, and delegate to the crates.               |
| `src/`                 | React frontend. IPC only through `src/lib/ipc.ts`; shared state only through the zustand store (`src/state/store.ts`, actions in `src/state/*Actions.ts`, event subscriptions in `src/state/initAction.ts`); Rust types mirrored in `src/lib/types.ts`. |

`src/lib/types.ts` must match Rust serde output: snake_case fields and
`kind`-tagged enums. Change both sides together.

## Concurrency model

- The Tauri process hosts one tokio runtime. Commands are `async`; network
  and child processes run on the runtime, never on the UI thread.
- The SQLite connection sits behind a `tokio::sync::Mutex` in `AppState`
  and is held only for short sections with no `.await` inside. The fix for
  contention is a connection pool, never `block_on`.
- Each agent run streams stdout/stderr through channels into one
  `select!` loop (`tandem-agents::runner`). That loop writes the JSONL log,
  forwards events to the UI, and enforces the run's timeout and cancel
  handle. `kill_on_drop` means a run never leaves orphaned children.
- Agent checkouts for runs are serialized by a mutex on `AppState`
  (`ensure_pr_checkout`), with one directory per (repo, PR).

## Domain rules

- **Cache-first.** `get_*` commands read SQLite and return immediately.
  `sync_*` commands hit GitHub, update the cache, and emit `tandem://sync`
  (`{key, phase}`) around the work. The UI never waits on the network
  without a sync event to show.
- **Events over polling.** The backend pushes `tandem://sync`,
  `tandem://agent-event`, `tandem://comments-updated`,
  `tandem://run-updated` and `tandem://summary-updated`, and the frontend
  subscribes in `initAction.ts`.
- **The `tandem_comment` contract.** An agent emits
  `{"type":"tandem_comment","path":…,"side":"new|old","line":…,"severity":…,"body":…}`
  (plus optional `end_line`, `suggestion`, `tag`, `parent_id`) on stdout,
  embedded in its final text, or appended to `$TANDEM_COMMENTS_FILE`. The
  PR-level write-up is `{"type":"tandem_summary","body":…}`. To change
  the contract, update `context.rs` (what the agent is told), `events.rs`
  (what is parsed) and this file together.
- **GitHub writes are user-only.** The only write paths are
  `tandem-github/src/write.rs`, reached from
  `src-tauri/src/commands/github_write.rs` and `merge.rs`, and they run
  only on explicit user clicks (post a chosen comment, approve, merge,
  commit a suggestion, and so on). An agent run must never be able to
  write to GitHub. Agent output stays local until the user posts it.
- **Schema migrations are append-only.** Add a new entry to `MIGRATIONS`
  in `crates/tandem-cache/src/schema.rs`. Never edit a migration that has
  shipped. Most rows are JSON blobs, so adding a `#[serde(default)]`
  field usually needs no migration at all.
- **Secrets.** A PAT currently lives in the settings file. That file is
  written atomically with mode `0600`, and moving the PAT to the keychain
  is on the roadmap. Secrets never go in the cache DB, run logs, or
  fixtures.
- **Runner-agnostic agents.** v2 moves runs into a Docker + squid sandbox.
  Keep "local process" assumptions inside `runner.rs` so a sandbox backend
  can slot in beside it.
- **Errors.** A `TandemError` message should be something a user can act
  on, for example "`gh auth token` failed — is the gh CLI logged in?".
  Failures show up in the UI error bar, not only in logs.

## Where state lives

- Settings: `<config dir>/tandem/settings.json`.
- Cache DB: `<data dir>/tandem/cache.sqlite3`.
- Agent run dirs: `<data dir>/tandem/runs/<run id>/`, holding
  `events.jsonl` and `comments.jsonl`.
- Clones: `<data dir>/tandem/worktrees/` (`agent-runs/<owner>/<repo>/<n>`
  for agent checkouts).
- Optional declarative config: `$TANDEM_CONFIG` or
  `~/.config/tandem/tandem.yaml` (see `examples/tandem.yaml`).
- **Debug builds use `tandem-dev` instead of `tandem`** for all of the
  above, so `make dev` never touches the installed app's data. The window
  title also says "Tandem Dev".

## App commands (run inside `tandem/`)

```sh
make install   # pnpm install
make dev       # tauri dev (debug build, tandem-dev data dir)
make dev-bg    # same, backgrounded; logs to .tandem-dev.log
make check     # = make check-tandem from the repo root (the CI gate)
```

If you changed runtime behavior, run `make dev` and exercise the flow you
touched.

## Releases

`.github/workflows/tandem-release.yml` releases on every push to `main`
that touches app code (including `utils/` and `rust-toolchain.toml`).
Tags are `tandem-vX.Y.Z`. The bump size comes from the merged PR's
label: `release:major`, `release:minor` or `release:patch` (the default).
A commit that is already tagged `tandem-v*` is released as-is.
`scripts/set-version.mjs` stamps the version into the bundle. Builds go
into a draft release that is published only after every platform has
uploaded, and they are then mirrored to the rolling `tandem-latest`
release. That release backs the in-app updater, `scripts/install.sh`,
`install.ps1` and the Homebrew cask. Tandem releases are never marked
`latest`, because that pointer belongs to the rca CLI.
