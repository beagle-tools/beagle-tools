# beagle-tools — agent guide

A monorepo of developer tools. This file applies everywhere in the repo. Each
app has its own `AGENTS.md` with app-specific facts; read both before you
change code in an app.

| Path | What it is | App guide |
|---|---|---|
| `tandem/` | Tandem: Tauri desktop app for reviewing GitHub PRs alongside coding agents | `tandem/AGENTS.md` |
| `rca/` | Beagle: RCA workspaces, with a TUI/CLI (`cli/`), an agent daemon (`crates/`), a desktop app (`desktop/`) and a static site (`web/`) | `rca/AGENTS.md` |
| `workflows/` | agentd: a scheduled agent-job daemon with CLIs/TUI (`crates/`) and the Agentdesk desktop app (`desktop/`) | `workflows/AGENTS.md` |
| `utils/` | `beagle-utils`: Rust helpers shared by two or more apps | `utils/README.md` |
| `standards/`, `scripts/`, root configs | Shared lint, format, compiler and CI configuration | `docs/CODING_STANDARDS.md` |
| `skills/` | Agent skills: `beagle-create-issue`, `beagle-complete-issue`, and rca's `beagle` / `beagle-review` (embedded into the rca binary). `.claude/skills/*` symlink here | `skills/README.md` |
| `packaging/`, `scripts/release/` | Homebrew formulas and casks, plus the per-app release pipeline | `docs/RELEASING.md` |

## How work flows

Read the doc for the stage you are in:

| Stage | Doc |
|---|---|
| Spec work as GitHub issues → implement → PR → merge → release | [`docs/SDLC.md`](docs/SDLC.md) (skills: `beagle-create-issue`, `beagle-complete-issue`) |
| What to test, and how test selection works | [`docs/TESTING.md`](docs/TESTING.md) |
| Running apps locally, browser mode with mocked IPC, tooling setup | [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) |
| What a merge releases, versions, tags, in-app updates, brew | [`docs/RELEASING.md`](docs/RELEASING.md) |

## Rules that apply to every app

1. **One set of coding standards: [`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md).**
   App guides add architecture and domain rules. They never loosen a
   standard. If an app guide seems to contradict the standards, the standards
   win; flag the conflict instead of picking one side.
2. **Shared config lives at the root and is never copied or overridden per
   app.** This covers:
   - `rust-toolchain.toml`, `rustfmt.toml`, `.prettierrc.json` and `.nvmrc`
   - `standards/cargo-lints.toml`, `standards/tsconfig.base.json` and
     `standards/eslint.base.js`

   `scripts/check-standards.mjs` fails CI on any drift.
3. **The gate is the merge contract.** CI's `gate` job runs the same scripts
   as `make check`. Green means mergeable, so never weaken a check to get
   green.
4. **pnpm only** (pinned via `packageManager`, run through corepack). Never
   create a `package-lock.json`.
5. **Each cargo workspace has its own `Cargo.lock`.** CI builds with
   `--locked`, so commit lockfile changes along with the dependency change.
6. **Every PR carries a `release:*` label** when it changes an app:
   `release:patch` (the default), `release:minor` or `release:major`. Merging
   to `main` releases only the apps the PR changed, as
   `beagle-<app>-vX.Y.Z` (see `docs/RELEASING.md`).

## Verify before you finish

Run these from the repo root. Each runs exactly what CI runs:

```sh
make standards          # 400-line rule, shared-config drift, CI-script tests
make check-tandem       # or check-rca / check-workflows / check-utils
make test-affected      # only the cargo tests your change affects (vs origin/main)
make e2e-tandem         # Playwright for one frontend (see docs/TESTING.md)
make check              # everything
```

The scripts behind these:
- `scripts/ci/rust.sh <dir>`: fmt, clippy `-D warnings`, test, and doc
  `-D warnings`, all `--locked`. With `AFFECTED_BASE=<ref>` (which CI sets on
  PRs), it tests only the affected packages and their dependents.
- `scripts/ci/frontend.sh <dir>`: frozen install, prettier, eslint, tsc,
  vitest, build and Playwright e2e.

## Cargo workspaces and frontends (all gated in CI)

- **Rust:**
  - `utils`
  - `tandem`
  - `rca` (cli and crates)
  - `rca/desktop/src-tauri`
  - `workflows` (crates)
  - `workflows/desktop`
- **Frontends:**
  - `tandem`
  - `rca/desktop`
  - `rca/web`
  - `workflows/desktop`

To add a workspace or frontend, add it to `scripts/check-standards.mjs`,
`.github/workflows/ci.yml` and the root `Makefile` in the same PR.
