# Local development

How to set up a Mac for this monorepo, start each app, and drive the UIs
with Playwright. The same commands work for humans and for agents (every
script puts cargo and nvm's node on PATH itself, so a fresh shell works).
For tests and what to write, see [`TESTING.md`](TESTING.md).

## 1. Set up the machine

```sh
make bootstrap-check   # report what's missing; changes nothing (exit 1 if anything is)
make bootstrap         # install what's missing; safe to re-run
```

`scripts/dev/bootstrap.sh` checks, in order, and skips anything already
present:

| Step | Checks | Installs (when missing) |
|---|---|---|
| Xcode Command Line Tools | `xcode-select -p` | `xcode-select --install` (GUI; re-run bootstrap after) |
| Homebrew | `brew` on PATH | nothing — prints the https://brew.sh instructions |
| CLI tools | `gh`, `tmux`, `jq` | `brew install …` |
| Optional | `glow` | only with `scripts/dev/bootstrap.sh --with-optional` |
| Rust | `rustup`; the channel in `rust-toolchain.toml` | `brew install rustup` + `rustup-init` (proxies in `~/.cargo/bin`), then `rustup toolchain install` |
| Node | node major from `.nvmrc` | `brew install nvm`, then `nvm install` (into `~/.nvm`, where the Makefile looks) |
| pnpm | `corepack` shims; `corepack pnpm --version` | `corepack enable` |
| Frontends | `node_modules` in each package; Playwright Chromium for packages that depend on `@playwright/test` | `pnpm install --frozen-lockfile`; `pnpm exec playwright install chromium` |

After bootstrap: `gh auth login` once (tandem and the skills use it).

Linux/Windows aren't scripted; install the same tools by hand (CI's
`_rust.yml` lists the Linux packages Tauri needs).

## 2. Start an app

```sh
make dev APP=<target>                  # or: make dev-<target>
make dev APP=<target> ARGS="--flag x"  # pass args to the app
make dev APP=<tauri target> BROWSER=1  # frontend only, mocked IPC (see 3)
scripts/dev/run.sh --list              # all targets
```

`make dev` calls `scripts/dev/run.sh <target> [--browser] [-- <args>]`. Every
target runs in the foreground; Ctrl-C stops it. First run of a frontend
target installs its `node_modules`.

| Target | Starts | Dir |
|---|---|---|
| `tandem` | Tandem desktop (`pnpm tauri dev`, vite on 1420) | `tandem/` |
| `rca-desktop` | Beagle RCA desktop (`pnpm tauri dev`, vite on 1420) | `rca/desktop/` |
| `workflows-desktop` | Agentdesk (`pnpm tauri dev`, vite on 1421) | `workflows/desktop/` |
| `rca-tui` | `beagle` TUI/CLI (`cargo run -p beagle --`) | `rca/` |
| `rca-agentd` | `beagle-agentd` daemon, foreground | `rca/` |
| `rca-web` | RCA static site (`astro dev`) | `rca/web/` |
| `agentd` | `agentd` scheduler daemon, foreground | `workflows/` |
| `agenttop` | `agenttop` TUI | `workflows/` |
| `agentctl` | `agentctl` CLI | `workflows/` |

Notes:
- `tandem` and `rca-desktop` both use port 1420; run one at a time.
- Debug builds of the desktop apps keep their data apart from an installed
  release app (see each app's `AGENTS.md`).
- The daemons use the same state dirs as an installed copy
  (`~/.local/state/agentd`, `~/.local/state/beagle-agent`). Stop the
  launchd service first (see `workflows/AGENTS.md` / `rca/AGENTS.md`), or
  point `XDG_STATE_HOME` at a scratch dir, so two daemons never write the
  same store.
- One-shot CLI calls work too: `make dev APP=rca-tui ARGS="--version"`,
  `make dev APP=agentctl ARGS="--help"`.

## 3. Drive a desktop UI in a browser (mocked IPC)

Tauri UIs normally need the Rust backend. With `--browser` (`BROWSER=1`),
`run.sh` starts only the frontend's vite dev server in **mode `e2e`**:

```sh
make dev APP=tandem BROWSER=1                         # http://localhost:1420
make dev APP=tandem BROWSER=1 ARGS="--port 1430"      # the port tandem's e2e config uses
```

**Contract (each Tauri frontend implements it):** when
`import.meta.env.MODE === "e2e"`, the app dynamically imports its mock
module and calls `mockIPC` from `@tauri-apps/api/mocks` with realistic
fixture data *before* rendering. Every Tauri command the UI calls has a
fixture; an unmocked command throws with its name. The import is guarded so
the mocks never reach the production bundle. The same mode is what each
app's `playwright.config.ts` `webServer` starts, so what you see in the
browser is exactly what the e2e specs see.

For agents: start the server in the background, then drive it with
Playwright (or `corepack pnpm exec playwright codegen <url>` to explore):

```sh
make dev APP=tandem BROWSER=1 ARGS="--port 1430" > /tmp/tandem-ui.log 2>&1 &
cd tandem && corepack pnpm exec playwright test e2e/inbox.spec.ts   # reuses the running server locally
```

`playwright.config.ts` sets `reuseExistingServer` outside CI, so a server
you already started on the config's port is reused instead of a second one.
Stop it with `kill %1` (or the PID) when done.

Real end-to-end with the Rust backend is `make dev APP=<target>` (no
`BROWSER`); Playwright can't attach to the Tauri webview, so automated UI
checks always go through mode `e2e`.

## 4. Test and check

```sh
make test-affected              # cargo tests for what changed vs origin/main (+ dependents)
make test-affected BASE=HEAD    # only your uncommitted changes
make e2e-tandem                 # Playwright: e2e-tandem, e2e-rca-desktop, e2e-rca-web, e2e-workflows-desktop
make check-<app>                # the full CI gate for one app (tandem, rca, workflows, utils)
make standards                  # 400-line rule, shared-config drift, CI-script tests
make check                      # everything CI runs
```

Selection rules, single-test commands and what to test:
[`TESTING.md`](TESTING.md).

## 5. Where things live

| Path | What |
|---|---|
| `scripts/dev/bootstrap.sh` | machine setup (`--check`, `--with-optional`) |
| `scripts/dev/run.sh` | start an app (`--list`, `--browser`, `-- args`) |
| `scripts/dev/env.sh` | shared PATH/pnpm setup sourced by the dev scripts |
| `scripts/ci/rust.sh` | Rust gate; `AFFECTED_BASE=<ref>` selects tests, `--tests-only` skips fmt/clippy/doc |
| `scripts/ci/frontend.sh` | frontend gate: install, prettier, eslint, tsc, vitest, build, e2e |
| `scripts/ci/affected.mjs` | which cargo packages a diff affects (rules in `affected-lib.mjs`) |
| `Makefile` | every entry point above |
