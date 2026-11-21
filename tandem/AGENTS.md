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

| Path                   | What lives there                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/tandem-core`   | Pure domain types (`github`, `review`, `agent`, `diff`), the unified-diff parser (`diff_parse`), `TandemError`. No I/O, nothing heavier than serde/chrono — every other crate speaks these types.                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `crates/tandem-github` | GitHub REST/GraphQL client. Wire structs in `wire.rs`, HTTP in `client.rs`, auth strategies in `auth.rs`, discovery in `discovery.rs`, rate-limit backoff in `rate_limit.rs`, the user-only write half in `write.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `crates/tandem-cache`  | SQLite. GitHub-data caching in `store.rs`; local-only review data in `review_store.rs` (+ `summaries.rs`, `revisions.rs`, `reviewed.rs`); merged-PR retention in `archive.rs`; the outside-process review queue in `queue.rs`, plus job listing, atomic claiming and `pr_marks` for the review TUI in `triage.rs`; migrations in `schema.rs`.                                                                                                                                                                                                                                                                                                              |
| `crates/tandem-agents` | Agent runners. argv per runner in `command.rs`, stdout classification + the comment contract in `events.rs`, prompt assembly in `context.rs` / `reply.rs` / `conversation.rs` / `conventions.rs` / `summary.rs`, process lifecycle in `runner.rs`.                                                                                                                                                                                                                                                                                                                                                                                                         |
| `crates/tandem-cli`    | `tandem-cli`: read-only JSON CLI over the cache DB, plus `app-review` (enqueue for the desktop app; alias `review`) and `update`; with no command it opens the review TUI, and `tandem-tui`'s queue commands (`add`, `claim`, `ingest`, `fail`, `list`) are flattened into its own. A library (`tandem_cli::run`/`run_as`, `ABOUT`) with a thin `main.rs`; `beagle` links it as `beagle tandem`. Reads raw SQL, so schema changes must keep it working (`query_tests.rs` builds a real store). See `docs/CLI.md`.                                                                                                                                          |
| `crates/tandem-tui`    | The review TUI (library only, linked by `tandem-cli`): a terminal review queue on its own store. `lib.rs` holds the `Command` enum and `open`/`run`/`paths`; `jobs.rs` the queue life cycle (add → claim → ingest/fail), `context.rs` + `ingest.rs` + `verdict.rs` the reviewer's output contract (change them together), `gh.rs` the GitHub reads and the two user-only writes, `state.rs` where each PR stands, `read.rs` the queue and one PR as JSON for `beagle serve` (store opened with `Cache::open_read_only`, never created), `ui/` the ratatui app (`keys.rs`, `view.rs`, `tabs.rs`, `diff.rs`, `worker.rs` for GitHub work off the UI thread). |
| `src-tauri`            | Thin Tauri shell. One file per domain under `src/commands/`; `state.rs` (`AppState`), `settings.rs` (settings + `AppDirs`), `file_config.rs` (optional `tandem.yaml`). Commands validate, lock state briefly, and delegate to the crates.                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src/`                 | React frontend. IPC only through `src/lib/ipc.ts`; shared state only through the zustand store (`src/state/store.ts`, actions in `src/state/*Actions.ts`, event subscriptions in `src/state/initAction.ts`); Rust types mirrored in `src/lib/types.ts`.                                                                                                                                                                                                                                                                                                                                                                                                    |

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
- `commands/drain.rs` polls `review_jobs` every 20s (and once on
  startup) and starts the reviews other processes queued through
  `tandem-cli app-review`. The queue is the one place an outside process
  writes; it records requests, never review data.
- The review TUI (`tandem-tui`) is synchronous on its UI thread: it
  polls keys every 250ms, re-reads its store every 2s, and asks the
  worker every 2 minutes (and once at startup) to re-check open PRs on
  GitHub, so merged and closed PRs show as finished and sort last. GitHub work
  (fetch, post, approve) runs on one worker thread with its own store
  connection and a current-thread tokio runtime, fed over channels; the
  `claim` command uses the same kind of runtime. Never `block_on` from
  inside async code.

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
  `src-tauri/src/commands/github_write.rs` and `merge.rs`, and from the
  review TUI's `ui/worker.rs` (via `gh.rs`, always as a named gh account —
  `ui/account.rs`, never gh's active one by default), and they run only on
  explicit user clicks or confirmed keypresses (post a chosen comment,
  approve, merge, commit a suggestion, and so on). An agent run must never be able to
  write to GitHub, and the runner enforces it: `beagle_utils::agent_env::deny_github`
  overrides `GH_TOKEN` with a dummy (so every `gh` call gets 401, keychain
  login included), empties `GH_CONFIG_DIR` and clears git's credentials, and
  Claude runs also get `--disallowedTools` for `gh`, `git push` and `security`.
  A process running as the user can still read the keychain directly; the v2
  sandbox closes that. Agent output stays local until the user posts it.
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
- Cache DB: `<data dir>/tandem/cache.sqlite3`. Agent env values (often API
  keys) are kept out of it, in `agent-env.json` (0600) in the same dir.
- Agent run dirs: `<data dir>/tandem/runs/<run id>/`, holding
  `events.jsonl` and `comments.jsonl`.
- Clones: `<data dir>/tandem/worktrees/` (`agent-runs/<owner>/<repo>/<n>`
  for agent checkouts).
- Optional declarative config: `$TANDEM_CONFIG` or
  `~/.config/tandem/tandem.yaml` (see `examples/tandem.yaml`).
- **Debug builds use `tandem-dev` instead of `tandem`** for all of the
  above, so `make dev` never touches the installed app's data. The window
  title also says "Tandem Dev".
- The review TUI (same in debug builds): its own store at `<data dir>/tandem-tui/cache.sqlite3`
  (`$BEAGLE_TANDEM_ROOT`/config `root` name its folder; `$TANDEM_TUI_DB`
  still names the file) — never the app's, whose drain would race the
  reviewer — and an optional `~/.config/beagle/tandem.toml`
  (`$TANDEM_TUI_CONFIG`; `github_user`, `kick`, `root`; the older
  `~/.config/tandem-tui/config.yaml` is the fallback; a broken file warns
  and the defaults apply). The reviewer's contract
  is in `docs/CLI.md` ("The review TUI"); an example agentd job is
  `workflows/manifests/demo/review-queue.yaml`.

## App commands (run inside `tandem/`)

```sh
make install   # pnpm install
make dev       # tauri dev (debug build, tandem-dev data dir)
make dev-bg    # same, backgrounded; logs to .tandem-dev.log
make check     # = make check-tandem from the repo root (the CI gate)
```

If you changed runtime behavior, run `make dev` and exercise the flow you
touched.

## Testing

- **Unit (vitest)**: `*.test.ts` beside the code in `src/` (helpers in
  `src/test/fixtures.ts`). `pnpm test` also runs the node:test suite for
  `scripts/set-version.mjs` (`scripts/set-version.test.mjs`, which works on
  a temp copy and never touches the checkout).
- **Rust**: unit tests beside the code (`#[cfg(test)]`); cross-module
  integration tests go in a crate's `tests/`.
- **e2e (Playwright, Chromium)**: specs in `e2e/*.spec.ts`, config in
  `playwright.config.ts`. `pnpm e2e` starts `vite --mode e2e` on port 1430
  (clear of `make dev` on 1420) and drives the real frontend in a plain
  browser. First run: `corepack pnpm exec playwright install chromium`.

### e2e mock mode

In `--mode e2e`, `src/main.tsx` dynamically imports `src/e2e/mockIpc.ts`
before rendering. It installs `mockIPC` (with `shouldMockEvents`) and
`mockWindows("main")` from `@tauri-apps/api/mocks`, and answers every
command in `src/lib/ipc.ts`, plus the plugin calls the UI makes
(`plugin:app|version`, `plugin:updater|check`, window/opener), with the
fixtures in `src/e2e/fixtures.ts`. The import sits behind
`import.meta.env.MODE === "e2e"`, which vite inlines, so production
builds drop the mock and the fixtures entirely. After changing that
code, check that `grep -c installIpcMocks dist/assets/*.js` prints 0
for every file after `pnpm build`.

- **When you add or rename an IPC command**, add it to `mockIPC`'s
  handler. Unknown commands resolve to `null`, which looks like an empty
  result rather than an error.
- **Fixtures** mirror `src/lib/types.ts` and are type-checked against it,
  so a type change that breaks them fails `pnpm typecheck`.
- **Knobs**: `?update=X.Y.Z` on the page URL makes the updater report an
  update, so the header button shows. With no knob, no update exists.
- **Selectors**: prefer roles and visible text over classes. Specs are
  typed TS under the shared ESLint rules.

## Releases

Release CI (see the repo's `docs/SDLC.md`) cuts a Tandem release when a
merge to `main` touches release-relevant paths: `tandem/**` except
`**/*.md` and `tandem/docs/**`, plus `utils/**` and
`rust-toolchain.toml`.

- **Tags** are `beagle-tandem-vX.Y.Z`, one GitHub release each, titled
  `Beagle Tandem vX.Y.Z`. They are published with `make_latest=false`,
  because no app owns the repo's "latest" pointer.
- **Bump size** comes from the merged PR's label: `release:major`,
  `release:minor`, or patch when there is no label. A commit that
  already carries a `beagle-tandem-v*` tag is released as-is. The first
  release uses the current version-file value.
- **Versions are stamped in CI only.**
  `node scripts/set-version.mjs X.Y.Z`, run from `tandem/`, stamps
  `package.json`, `src-tauri/tauri.conf.json`, `Cargo.toml`'s
  `[workspace.package] version`, and the workspace members' `Cargo.lock`
  entries, so `--locked` builds still work. Never commit the stamped
  files.
- **The mirror.** Each release refreshes the rolling
  `beagle-tandem-latest` release, which holds the stable-named installers
  (`Tandem-macos-arm64.dmg`, `Tandem-macos-x64.dmg`,
  `Tandem-windows-x64-setup.exe`, `Tandem-linux-x86_64.AppImage`,
  `Tandem-linux-amd64.deb`) and `latest.json`. That release backs:
  - the in-app updater: the `plugins.updater` endpoint and pubkey in
    `tauri.conf.json`;
  - `scripts/install.sh` and `install.ps1`;
  - the Homebrew cask (`brew install --cask beagle-tools/tap/beagle-tandem`).
    The cask's source is root `packaging/homebrew/`, which release CI
    owns, not this directory.
- **The CLI sidecar.** `tandem-cli` ships inside the bundle: release CI
  builds it per target and declares it in `tauri.conf.json`'s
  `bundle.externalBin` (`scripts/release/stage-sidecar.sh`, CI only, so
  plain cargo builds never need it). The cask links
  `Tandem.app/Contents/MacOS/tandem-cli` onto PATH, and
  `set-version.mjs` stamps it with the other workspace members.
- **Rollback.** Settings › Version (`src-tauri/src/commands/updates.rs`)
  lists the releases whose tag starts with `TAG_PREFIX = "beagle-tandem-v"`.
  It installs one through that tag's own `latest.json`, and the signature
  check applies either way. If you change the tag scheme, update
  `TAG_PREFIX` and its tests in the same change.
