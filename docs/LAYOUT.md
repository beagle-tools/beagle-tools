# Repo layout

Where every tool lives, and the rules that keep it that way. Each tool is
one top-level folder named exactly like its `beagle` subcommand, so
`beagle scout` lives in `scout/` and `beagle agent` lives in `agent/`.

> **Status: proposed.** This is the target. The folders move one tool per
> PR in the order below. Until a tool's PR merges, it is still at its old
> path. When the moves are done, this file describes the repo as it is.

## The target

```
beagle/          the beagle command (help, update, doctor, setup, version)
  serve/         beagle serve: the read-only HTTPS API for the phone app
scout/           beagle scout
  tui/           the beagle scout crate (TUI + CLI, embedded skills)
  ios/           the iPhone app (not built yet)
recap/           beagle recap
  tui/
standup/         beagle standup
  tui/
rca/             beagle rca
  tui/           the beagle rca crate (TUI + CLI)
  daemon/        beagle rca agentd
  desktop/       the Beagle desktop app (Tauri)
  web/           the static RCA site (Astro + Docker image)
  lib/agent/     the agent engine the daemon hosts
tandem/          beagle tandem
  tui/           the beagle tandem crate (CLI, and the desktop app's sidecar)
  desktop/       the Tandem desktop app (Tauri + frontend)
  lib/core/  lib/github/  lib/cache/  lib/agents/  lib/queue/
agent/           beagle agent (today's workflows/)
  tui/           the beagle agent crate (agentctl: CLI, dashboard, daemon commands)
  daemon/        agentd, the scheduler daemon
  desktop/       Agentdesk (Tauri)
  lib/core/  lib/top/
utils/           code shared by two or more tools (beagle-utils, beagle-tui)
docs/  scripts/  standards/  packaging/  skills/  .github/   repo-wide
```

## The rules

1. **A tool is one top-level folder**, named exactly like its `beagle`
   subcommand (the `name` in `beagle/src/tools.rs`). No tool code lives
   anywhere else.
2. **Inside a tool, one folder per part you can run**, using only these
   names:
   - `tui/`: the crate `beagle <tool>` links, holding its TUI and CLI. Every
     tool has one.
   - `daemon/`: a long-running background service.
   - `desktop/`: a Tauri desktop app (Rust shell plus frontend).
   - `web/`: a website or web service.
   - `ios/`: an iPhone app.
3. **Libraries shared by a tool's own parts** go in `<tool>/lib/<name>/`.
   The folder name is short and drops the tool prefix (`tandem/lib/core`,
   not `tandem/lib/tandem-core`).
4. **Code shared by two or more tools** goes in `utils/`. Nothing in one
   tool's folder may depend on another tool's folder, except the `beagle`
   crate, which links every tool. `beagle serve` reads every tool's store,
   so it may link them too.
5. **The `beagle` command itself** is `beagle/`, its crate at the folder
   root, with `beagle/serve/` beside its `src/`. `serve` is a `beagle`
   subcommand but not a tool: it has no folder of its own.
6. **Besides its parts, a tool's top level may hold only** `lib/`, `docs/`,
   `assets/`, `scripts/`, `deploy/`, `examples/`, `skills/`, and the files
   `README.md`, `AGENTS.md`, `CLAUDE.md`, `CHANGELOG.md`, `Makefile`,
   `.gitignore`. Inside a part (for example `desktop/src-tauri/`, `tui/docs/`)
   the layout is free.
7. **One Rust workspace at the root**, with one `Cargo.lock`, covering every
   crate except the Tauri shells. Each Tauri shell keeps its own workspace and
   lockfile inside its `desktop/` folder.
8. **Package names don't follow folder names.** Folders say where code is;
   package names stay what they are, so imports, `cargo -p` and release
   builds don't churn. Rename a package only when it has a reason of its own.

Task 9 of the move turns these rules into a `make standards` check.

## The full map

Every crate and app folder today, its new home, and its package name. All
package names stay unless the last column says otherwise.

### `cli/` → `beagle/`, `scout/`, `recap/`, `standup/`

| Today | New path | Package | Notes |
|---|---|---|---|
| `cli/crates/beagle` | `beagle/` | `beagle-cli` (bin `beagle`) | |
| `cli/crates/serve` | `beagle/serve/` | `beagle-serve` | |
| `cli/crates/scout` (with `assets/`, `skills/`) | `scout/tui/` | `beagle-scout` | `include_str!` paths are crate-relative, unchanged |
| `cli/crates/recap` (with `docs/`) | `recap/tui/` | `beagle-recap` | |
| `cli/crates/standup` (with `docs/`) | `standup/tui/` | `beagle-standup` | |
| `cli/crates/agent` | folded into `agent/tui/` | `beagle-agent-tool` goes away | See [Open choices](#open-choices) |
| `cli/scripts/set-version.mjs` (+ test) | `beagle/scripts/` | | Unless task 2 replaces it |
| `cli/README.md`, `AGENTS.md`, `CLAUDE.md`, `.gitignore` | `beagle/` | | |
| `cli/Cargo.toml`, `cli/Cargo.lock` | removed (task 2) | | Root workspace replaces them |

### `rca/`

| Today | New path | Package | Notes |
|---|---|---|---|
| `rca/cli` (with `assets/`) | `rca/tui/` | `beagle` (lib `beagle`) | Same depth, so its `../../../skills/…` include still resolves |
| `rca/crates/beagle-agentd` | `rca/daemon/` | `beagle-agentd` | |
| `rca/crates/beagle-agent` | `rca/lib/agent/` | `beagle-agent` | |
| `rca/desktop` (frontend + `src-tauri/`) | `rca/desktop/` (stays) | `beagle-desktop` | Path dep `../../cli` → `../../tui`; own workspace stays |
| `rca/web` | `rca/web/` (stays) | npm `rca/web` | |
| `rca/rcas/` (example RCA) | `rca/examples/rcas/` | | Proposed; see [Open choices](#open-choices) |
| `rca/deploy/helm/`, `rca/docs/`, `rca/scripts/`, `Makefile`, `CHANGELOG.md` | stay | | |
| `rca/Cargo.toml`, `rca/Cargo.lock` | removed (task 2) | | |

### `tandem/`

| Today | New path | Package | Notes |
|---|---|---|---|
| `tandem/crates/tandem-cli` | `tandem/tui/` | `tandem-cli` (lib + bin `tandem-cli`) | The bin is the desktop app's sidecar; the name stays |
| `tandem/crates/tandem-tui` | `tandem/lib/queue/` | `tandem-tui` | See [Open choices](#open-choices) |
| `tandem/crates/tandem-core` | `tandem/lib/core/` | `tandem-core` | |
| `tandem/crates/tandem-github` | `tandem/lib/github/` | `tandem-github` | |
| `tandem/crates/tandem-cache` | `tandem/lib/cache/` | `tandem-cache` | |
| `tandem/crates/tandem-agents` | `tandem/lib/agents/` | `tandem-agents` | |
| `tandem/src-tauri/` | `tandem/desktop/src-tauri/` | `tandem` | Leaves the tandem workspace for its own (rule 7) |
| `tandem/src/`, `index.html`, `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig*.json`, `eslint.config.js`, `playwright.config.ts`, `e2e/` | `tandem/desktop/` | npm `tandem` | The frontend lives with its shell, like rca's and Agentdesk's |
| `tandem/art/` | `tandem/assets/` | | No file references it by path |
| `tandem/scripts/install.sh`, `install.ps1` | **stay** | | Public `curl …/main/tandem/scripts/install.sh` URLs |
| `tandem/scripts/set-version.mjs` (+ test) | stay | | Update the file it stamps |
| `tandem/docs/`, `examples/`, `Makefile` | stay | | |
| `tandem/Cargo.toml`, `tandem/Cargo.lock` | removed (task 2) | | |

### `workflows/` → `agent/`

| Today | New path | Package | Notes |
|---|---|---|---|
| `workflows/crates/agentctl` | `agent/tui/` | `agentctl` (bin `agentctl`) | What `beagle agent` runs; it already owns `top` and `daemon run` |
| `workflows/crates/agenttop` | `agent/lib/top/` | `agenttop` (bin `agenttop`) | See [Open choices](#open-choices) |
| `workflows/crates/agentd` (with `assets/`) | `agent/daemon/` | `agentd` (bin `agentd`) | |
| `workflows/crates/agent-core` | `agent/lib/core/` | `agent-core` | |
| `workflows/desktop` (frontend + `src-tauri/`, own workspace) | `agent/desktop/` | `agentdesk` | Path deps `../crates/agent-core` → `../lib/core`, `../../utils` unchanged |
| `workflows/deploy/`, `assets/`, `scripts/` | `agent/deploy/`, `agent/assets/`, `agent/scripts/` | | agentctl's `include_str!("../../../deploy/…")` becomes `../../deploy/…` (one level shallower) |
| `workflows/manifests/demo/` | `agent/examples/demo/` | | Proposed; see [Open choices](#open-choices) |
| `workflows/README.md`, `AGENTS.md`, `CLAUDE.md`, `Makefile`, `.gitignore` | `agent/` | | |
| `workflows/Cargo.toml`, `workflows/Cargo.lock` | removed (task 2) | | |

### Unchanged

`utils/` (`beagle-utils`) and `utils/tui/` (`beagle-tui`), `docs/`,
`scripts/`, `standards/`, `packaging/`, `skills/`, `.github/`, and the root
config files.

## What doesn't change

Installed apps, Homebrew and users' machines follow these names, so the moves
keep every one of them:

- **Release tags and app keys:** `beagle-cli-v*`, `beagle-rca-v*`,
  `beagle-tandem-v*`, `beagle-workflows-v*`, and the `beagle-<app>-latest`
  mirror releases. The release app keys stay `cli`, `rca`, `tandem`,
  `workflows` even though two of their folders become `beagle/` and `agent/`.
- **Homebrew:** formulas `beagle`, `beagle-rca`, `beagle-workflows`; casks
  `beagle-tandem`, `beagle-rca-desktop`, `beagle-agentdesk`; tarball names
  (`beagle-cli-<target>.tar.gz` and so on). Only the `homepage` URLs in the
  templates change.
- **Binary names:** `beagle`, `beagle-agentd`, `tandem-cli`, `agentd`,
  `agentctl`, `agenttop`.
- **App bundle IDs and product names:** `com.matthewmyrick.tandem`
  (Tandem), `com.matthewmyrick.beagle` (Beagle), `com.matthewmyrick.agentdesk`
  (Agentdesk).
- **Data and config locations:** every store, config file, launchd label
  (`com.agentd.daemon`) and log path. Code finds these at runtime, never
  through the repo layout.
- **Public URLs:** the tandem install scripts (`tandem/scripts/install.*`), the
  `ghcr.io/beagle-tools/beagle-rca-web` image, and installed skills
  (`scripts/skills/install.sh` links into top-level `skills/`).
- **GitHub labels:** `app:*` and `release:*` keep their names; only the
  descriptions in `scripts/github/labels.sh` change.

## The order of the moves

One PR per step, each green on its own, using `git mv` so history follows the
files. Every move PR carries a `release:patch` label, since it touches the
paths of a releasing app.

| # | PR | Moves |
|---|---|---|
| 2 | Root workspace | Nothing. One root `Cargo.toml` + `Cargo.lock` at today's paths |
| 3 | beagle | `cli/crates/beagle` → `beagle/`, `cli/crates/serve` → `beagle/serve/` |
| 4 | scout | `cli/crates/scout` → `scout/tui/`, adds `scout/README.md` |
| 5 | recap + standup | `cli/crates/{recap,standup}` → `{recap,standup}/tui/`; folds `cli/crates/agent` into agentctl; deletes `cli/` |
| 6 | rca | `rca/cli` → `rca/tui`, `rca/crates/*` → `rca/daemon`, `rca/lib/agent` |
| 7 | tandem | crates → `tandem/tui`, `tandem/lib/*`; desktop → `tandem/desktop/` |
| 8 | agent | `workflows/` → `agent/`, reshaped |
| 9 | enforce | the layout check in `make standards` |

**Coordinate step 8 with the `agent-swarm` project.** It plans changes inside
`workflows/` (agentd in particular). Land the move before that work starts, or
after it merges, never alongside: a rename under an open branch turns every
one of its edits into a conflict.

### What each move PR must update

Every move PR checks this list for the paths it moves:

- **Root `Cargo.toml`:** workspace members and `[workspace.dependencies]`
  paths; path deps inside the moved crates; the Tauri shells' path deps
  (`rca/desktop/src-tauri`, `tandem/desktop/src-tauri`, `agent/desktop`).
- **`include_str!` / `include_bytes!`** that leave their crate: rca's skills
  (`../../../skills/…`, same depth after the move), agentctl's
  `deploy/`, `scripts/build-notifier.sh` and `assets/notify/` (one level
  shallower), `utils/tui`'s `standards/tui-standards.json` (unmoved).
- **`.github/workflows/ci.yml`:** the `changes` filters (rename `cli` →
  `beagle` and `workflows` → `agent`, and add `scout/**`, `recap/**`,
  `standup/**` and every linked path to the `beagle` filter), the
  `_rust.yml` / `_frontend.yml` `dir:` inputs, the rca web image `file:`, and
  `cli/scripts/*.test.mjs` in the standards job.
- **`.github/workflows/release.yml`:** `app-dir`, `cargo-dir`, `project`,
  `extra-app-dir`/`extra-cargo-dir`, the rca web `file:`; the comments that
  name paths.
- **`scripts/release/lib.mjs`:**
  - `APPS[*].versionFile`.
  - A new `dir` per app, since `isReleaseRelevant` assumes the app key is the
    folder (`${app}/`). After the moves `cli` → `beagle/`, `workflows` →
    `agent/`.
  - `EMBEDDED_PREFIXES` (recap/standup docs, scout skills).
  - `LINKED_PREFIXES.cli` and `LINKED_FILES.cli`. `scout/`, `recap/`,
    `standup/` join them once they leave `cli/`.
  - `PACKAGING_FILES` (paths only; names stay).
- **`scripts/release/linked.test.mjs`:** the crate it walks from
  (`cli/crates/beagle` → `beagle`), the `covered` prefix, and the ci.yml
  filter name it slices.
- **`Makefile`:** `check-*`, `RUST_WORKSPACES`, `e2e-*`, the `standards` and
  `test-scripts` test globs. Rename `check-cli` → `check-beagle` and
  `check-workflows` → `check-agent`, and `e2e-workflows-desktop` →
  `e2e-agent-desktop`.
- **`scripts/check-standards.mjs`:** `RUST_WORKSPACES` and `FRONTENDS`.
- **`standards/tui-standards.json`:** the crate paths it keys on.
- **`scripts/dev/run.sh`, `scripts/dev/env.sh`:** target dirs and
  `FRONTENDS`. Rename `workflows-desktop` → `agent-desktop`.
- **`scripts/ci/*`:** comments and examples that name paths;
  `affected-lib.mjs` if it lists paths.
- **`packaging/homebrew/`:** `homepage` URLs in the formula templates and casks.
- **`beagle/src/selfupdate/install_kind.rs`:** the `cargo install --path
  cli/crates/beagle` hint and its test.
- **`rca/web`:** `src/lib/rcas.ts`, the `Dockerfile` `COPY rcas/` and its
  build context, if `rcas/` moves.
- **`scripts/github/labels.sh`:** label descriptions.
- **Docs:** root `README.md` and `AGENTS.md` (the path table and the
  workspace list), every moved folder's `README.md` / `AGENTS.md`,
  `docs/{DEVELOPMENT,RELEASING,TESTING,SDLC,CODING_STANDARDS,serve-api}.md`,
  `skills/README.md` and the `skills/*/SKILL.md` files that name paths.
- **Comments and doc links in Rust/TS** that name old paths
  (`grep -rn 'cli/crates\|workflows/\|rca/cli\|rca/crates\|tandem/crates'`
  should come back empty for the moved tool).

## Open choices

Things that don't fit the rules cleanly. Each has a recommendation; the move
PRs follow it unless decided otherwise.

1. **`beagle agent`'s crates (agentctl, agenttop, the `cli/crates/agent`
   shim).** agentctl is the CLI `beagle agent` runs, and it already links
   agenttop (`top`) and agentd (`daemon run`). The 62-line shim only exists
   because agentctl sat in another workspace.
   **Recommend:** `agent/tui/` = agentctl, with the shim's `PROGRAM` and
   bare-command default folded into agentctl's lib (step 5, so `cli/` can be
   deleted). agenttop becomes `agent/lib/top/`, a library `tui/` links.
   Merging agenttop into `tui/` as a module is a later, optional cleanup.
2. **tandem-cli vs tandem-tui.** tandem-cli is the `beagle tandem` crate and
   the desktop sidecar binary; tandem-tui is the review-queue screen it
   links. **Recommend:** `tandem/tui/` = tandem-cli, and tandem-tui →
   `tandem/lib/queue/` (`lib/tui` beside `tui/` would read as a duplicate).
   Folding it into `tandem/tui/` later is optional.
3. **Example data (`rca/rcas/`, `workflows/manifests/demo/`).** Neither is a
   runnable part. **Recommend:** `<tool>/examples/` for both
   (`rca/examples/rcas/`, `agent/examples/demo/`). The rca web build then
   reads `../examples/rcas` (default in `rcas.ts`, `COPY` in the Dockerfile).
   *Alternative:* allow `rcas/` as a named exception and change nothing.
4. **`skills/`.** It holds rca's embedded skills (`beagle`, `beagle-review`)
   and the repo's own (`beagle-create-issue`, `beagle-complete-issue`), while
   scout's embedded skills live in its crate. **Recommend:** keep top-level
   `skills/` as is. `scripts/skills/install.sh` symlinks users'
   `~/.claude/skills/*` into it, so moving rca's skills breaks installs.
   Scout's skills stay in `scout/tui/skills/`.
5. **`packaging/`.** **Recommend:** keep it top-level. It is the Homebrew
   tap's source, rendered by the release pipeline for every app at once, and
   formula/cask names don't map 1:1 to tools (`beagle` covers all of them).
6. **Desktop workspace manifests sit in two places:** `rca/desktop/src-tauri/`
   vs `agent/desktop/` (one level up, with `src-tauri` as a member).
   **Recommend:** leave both as they are; only tandem's needs a new one, and
   it should copy rca's (manifest in `tandem/desktop/src-tauri/`). Unifying
   is optional.
7. **Package names that mislead.** rca's crate is named `beagle` and its
   engine `beagle-agent` (not `beagle agent`). **Recommend:** keep them for
   this move (rule 8); a rename (`beagle-rca`, `rca-agent`) is its own PR
   later, if wanted.
8. **Release app keys `cli` and `workflows`.** **Recommend:** keep them (tags,
   mirrors and updaters depend on them) and add the `dir` field above,
   instead of renaming the apps to `beagle` and `agent`.
