# Beagle (rca) — agent guide

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first; they hold
the repo-wide rules and the gate. This file only covers what is specific to
Beagle.

Beagle renders **RCA (root-cause analysis) workspaces**: directories of
markdown, a TOML manifest and ASCII diagrams that an agent (or a human) writes
while debugging an incident. The user keeps a Beagle viewer open and it
live-reloads as the write-up lands. Because it runs *during incidents*, next
to whatever is broken, a crash, a hung redraw or a broken terminal is a P0.

**Authoring RCAs is not covered here.** If you are debugging an incident or
writing one up, use the `/beagle` skill
([`../skills/beagle/SKILL.md`](../skills/beagle/SKILL.md));
to answer questions about an existing RCA, `/beagle-review`
([`../skills/beagle-review/SKILL.md`](../skills/beagle-review/SKILL.md)).
Both skills are embedded into the `beagle` binary (`cli/src/skill.rs`,
`include_str!`), so editing them changes what `beagle skill install` ships.

## Components

| Path | What it is | Notes |
|---|---|---|
| `cli/` | Crate `beagle`: the domain layer (`model`, `store`, `markdown`, `similar`, `config`, …) **and** the TUI (`ui/`) + CLI (`main.rs`, `cli/`, `commands/`) | The only implementation of the data format. Everything else depends on it. |
| `crates/beagle-agent` | Agent-runtime engine: config, SQLite job store, RCA poller, worktree manager, headless `claude` runner, orchestrator, control protocol | Depends on `beagle`. |
| `crates/beagle-agentd` | The always-on daemon binary hosting the engine | See [`docs/agent-runtime.md`](docs/agent-runtime.md). |
| `desktop/` | Tauri 2 + React viewer. `src-tauri/` is its **own cargo workspace** (own `Cargo.lock`), path-depending on `cli/` | [`desktop/AGENTS.md`](desktop/AGENTS.md) |
| `web/` | Astro static site of *published* postmortems | [`web/AGENTS.md`](web/AGENTS.md) |
| `deploy/` | Helm chart for the web image | `web/DEPLOY.md` |
| `rcas/` | Sample/real workspaces the apps read by default | Data, not code. |

`Cargo.toml` here is the workspace root for `cli` + `crates/*`; it excludes
`desktop/`. Shared helpers (XDG paths, atomic writes, `PATH` augmentation,
truncation) come from `beagle-utils` (`../utils`) — don't re-add local copies.

### Module map (`cli/src`)

- Dependencies point one way: `ui → store → model`. `model` imports nothing
  but `error`; `store` never imports `ui`.
- `store/fsio.rs` is the only place that reads or writes section files:
  bounded reads (`MAX_FILE_BYTES`, checked from metadata before reading) and
  atomic writes.
- `agentd/paths.rs` is the **single source** of the daemon's socket, state
  and config paths. The daemon crates and `desktop/src-tauri` call it, so
  every client looks where the daemon listens.
- `prs.rs` / `tickets.rs` / `notify.rs` / `clipboard.rs` shell out (`gh`,
  `osascript`, `pbcopy`, …). A missing tool degrades to a no-op, never an
  error screen. Resolve tools through `beagle_utils::process` so the bundled
  desktop app (minimal GUI `PATH`) finds them.
- The desktop IPC contract is `desktop/src-tauri/src/dto.rs` ⇄
  `desktop/src/types.ts`; commands return `desktop/src-tauri/src/error.rs`'s
  `Error`, which serializes to a plain message string.

## Concurrency model

**Threads + `std::sync::mpsc`, no async**, in `cli` and the daemon crates.

- The TUI event loop (`ui/event_loop.rs`) blocks on `crossterm::event::poll`
  with a timeout. It wakes at least every 250 ms for the spinner and is
  otherwise idle (~0% CPU).
- Background producers each own a thread and send results over a channel:
  the `notify` filesystem watcher, the `gh` PR-state poller (`prs.rs`) and
  the `beagle-agentd` status poller (`agentd/client.rs`). They exit when the
  receiver drops.
- The watcher callback only sends an event. No I/O, no parsing. The loop
  drains the channel and reloads **once per drain**, because editors and
  LLMs write in bursts.
- `beagle-agent` uses a `Mutex` around the job store and per-repo worktree
  locks (the orchestrator runs a bounded batch of jobs concurrently). Every
  job runs under `catch_unwind` in its own process group.
- The Tauri shell is the one exception: its commands run on Tauri's runtime,
  and blocking work goes through `tauri::async_runtime::spawn_blocking`.

## The data format is the public API

Everything under `rcas/` is read by the TUI, desktop, web, the daemon, the
skills and humans with a text editor. A change to it is a **breaking change**:
it needs a migration note in the PR and updates to
`../skills/beagle/SKILL.md` and the `README.md` "Data format" section.

```text
rcas/<slug>/          slug: [a-z0-9-], 1..=64 chars (RcaId)
  rca.toml            title, severity, status, created (RFC 3339, quoted),
                      updated?, systems, tags, prs, tickets, published?
                      — deny_unknown_fields
  summary.md timeline.md root-cause.md impact.md remediation.md
  commands.md? final-review.md notes.md log.md
  diagrams/NN-name.txt   ASCII, rendered unwrapped, ANSI SGR allowed
rcas/archive/<slug>/  archived workspaces (hidden by default, still mined)
toolbox.md, systems/*.md   agent context at the store root (`beagle init`)
```

- Lifecycle: `investigating → review → agent → final-review → finished`.
  Special tags: `skip-final-review` and `security` (`model/mod.rs`).
- Store root resolution: `--root` → nearest `.beagle` walking up (git-style)
  → global config `root` → nearest ancestor containing `rcas/` → cwd.
  `$BEAGLE_CONFIG` skips `.beagle` discovery.

## TUI rules

- **Never panic on user data.** Malformed TOML, missing sections, invalid
  UTF-8 and 10 MB files all degrade: a corrupt manifest skips that workspace
  with a warning (or a "broken" sidebar row); a missing section renders a hint.
- **Terminal restore is unconditional.** Raw mode and the alternate screen
  are torn down on every exit path, panics included (`ratatui::init` /
  `restore` hooks). Leaving the user's shell broken is a P0.
- **Never touch disk in the draw loop.** Drawing (`ui/view/`) is a pure
  function of `App` (scroll clamping aside). Reads happen at startup, on
  watcher events and on explicit reload.
- **Lazy load, cache narrowly, evict.** Listing reads only manifests. Section
  content is read when its tab opens, cached per (workspace, tab, diagram)
  (`ui/pane.rs`) and dropped on switch. Markdown renders once per change,
  not per frame.
- **All state lives in `App`** and changes only on key presses or watcher
  events, so every transition is unit-testable without a terminal
  (`ui/tests/`).
- Performance budgets:
  - cold start to first frame < 50 ms with 100 RCAs
  - redraw on keypress < 5 ms
  - idle CPU ~0%
  - memory proportional to the *open* RCA, not the corpus

### What needs tests

| Change | Tests |
|---|---|
| `model` | serde round-trips + invalid-input rejection |
| `store` | temp-dir round-trips, missing/corrupt files |
| markdown / diagram rendering | assertions on the rendered `Text` |
| key handling / `App` state | state-transition tests, no terminal |
| pure layout | manual check is OK; describe it in the PR |

## Dependencies

The `beagle` crate's runtime deps are deliberately few:
- `ratatui` + `crossterm` (TUI)
- `serde` + `toml` + `serde_json` (manifests, `--json`)
- `notify` (fs watching)
- `thiserror`, `time`
- `beagle-utils`

There is no `clap` (the parser in `cli/src/cli/` is small and audited), no
`anyhow` (one closed `Error` enum) and no async runtime. `beagle-agent` adds
`rusqlite` (bundled) and `nix`. The desktop app must not depend on
`beagle-agent`: it speaks the daemon's newline-JSON wire format itself, so it
doesn't bundle SQLite.

## Agent runtime

Setting an RCA to `status = "agent"` hands it to `beagle-agentd`, which
implements `remediation.md` in an isolated worktree via headless `claude`,
opens a PR, attaches it and advances the RCA to `final-review`. Contracts,
config (`agents.toml`), service install (`beagle agent install`) and the
control socket are in [`docs/agent-runtime.md`](docs/agent-runtime.md).

## Commands

```sh
make check            # from rca/: same as `make check-rca` at the repo root
make run              # open the TUI against ./rcas
make agentd           # build the daemon
make fmt|lint|test    # quick loops over both rust workspaces (rca + desktop/src-tauri)
make install          # pnpm install for desktop/ and web/
make desktop          # tauri dev (see desktop/AGENTS.md)
make web              # astro dev server
```

## Releases

rca is **one version** across `cli`, `crates/*`, `desktop` (Tauri +
`package.json`) and `web`. Release automation (repo root, see
[`../AGENTS.md`](../AGENTS.md)) cuts `beagle-rca-vX.Y.Z` after a merge to
`main` that touched `rca/` (docs-only changes don't release); the bump is
patch unless the PR carries `release:minor` / `release:major`. Don't bump
versions by hand — CI runs `node scripts/set-version.mjs X.Y.Z` (from
`rca/`), which stamps every manifest plus the matching `Cargo.lock` entries,
and never commits the result. Its tests (`scripts/set-version.test.mjs`) run
in `desktop`'s `pnpm test`.

What ships, per release (and mirrored with stable names to the rolling
`beagle-rca-latest` release):

| Asset | Consumer |
|---|---|
| `beagle-rca-<target>.tar.gz` + `.sha256` (`beagle`, `beagle-agentd` at the root; aarch64/x86_64-apple-darwin, x86_64-unknown-linux-musl) | `beagle update`, `brew install beagle-tools/tap/beagle-rca` |
| `Beagle-macos-{arm64,x64}.dmg`, `Beagle-linux-x86_64.AppImage`, `Beagle-linux-amd64.deb` + `latest.json` | the desktop updater, `brew install --cask beagle-tools/tap/beagle-rca-desktop` |
| `ghcr.io/beagle-tools/beagle-rca-web:X.Y.Z` / `:latest` | `web/DEPLOY.md` |

The self-updater (`cli/src/update/`) only considers `beagle-rca-v*` tags and
downloads exactly those asset names, so don't rename them. The desktop
updater checks `beagle-rca-latest/latest.json`; its rollback list filters by
the same tag prefix (`desktop/src-tauri/src/updates.rs`).

Bug reports should include:
- OS and terminal
- `beagle --version`
- the triggering workspace (or a minimized copy)
- for a panic, the message with `RUST_BACKTRACE=1`
