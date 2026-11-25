# beagle-tools — agent guide

A monorepo of developer tools. This file applies everywhere in the repo. Each
app has its own `AGENTS.md` with app-specific facts; read both before you
change code in an app.

Every tool is one top-level folder named like its `beagle` subcommand,
with one folder per runnable part (`tui/`, `daemon/`, `desktop/`, `web/`,
`ios/`) and its libraries in `lib/` ([`docs/LAYOUT.md`](docs/LAYOUT.md);
`make standards` fails anything else).

| Command | Folder | Parts | Guide |
|---|---|---|---|
| `beagle` | `beagle/` | the command (help, update, doctor, setup); `serve/` (`beagle serve`) | `beagle/AGENTS.md` |
| `beagle agent` | `agent/` | `tui/` agentctl, `daemon/` agentd, `desktop/` Agentdesk, `lib/core`, `lib/top` agenttop, `examples/demo` | `agent/AGENTS.md` |
| `beagle rca` | `rca/` | `tui/`, `daemon/` (`beagle rca agentd`), `desktop/` Beagle app, `web/` postmortem site, `lib/agent`, `examples/rcas` | `rca/AGENTS.md` |
| `beagle recap` | `recap/` | `tui/` | `recap/tui/README.md` |
| `beagle scout` | `scout/` | `tui/` | `scout/tui/AGENTS.md` |
| `beagle standup` | `standup/` | `tui/` | `standup/tui/README.md` |
| `beagle tandem` | `tandem/` | `tui/` tandem-cli, `desktop/` Tandem app, `lib/{core,github,cache,agents,queue}` | `tandem/AGENTS.md` |

| Shared | What it is | Guide |
|---|---|---|
| `ios/` | The Beagle iPhone app for every tool (home screen of tools; each tool's screens in `ios/Beagle/Tools/<Tool>/`) | `ios/AGENTS.md` |
| `ios/` | The Beagle iPhone app for every tool (home screen of tools; each tool's screens in `ios/Beagle/Tools/<Tool>/`) | `ios/AGENTS.md` |
| `utils/` | `beagle-utils` and `beagle-tui`: Rust helpers shared by two or more tools; `swift/` BeagleKit, the Swift package the iPhone apps share | `utils/README.md` |
| `standards/`, `scripts/`, root configs | Shared lint, format, compiler and CI configuration | `docs/CODING_STANDARDS.md` |
| `skills/` | Agent skills: `beagle-create-issue`, `beagle-complete-issue`, and rca's `beagle` / `beagle-review` (embedded into `beagle`, served by `beagle rca skill install`). `.claude/skills/*` symlink here | `skills/README.md` |
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
   - `standards/swift/` (swift-format rules, `Shared.xcconfig`, package
     settings, coverage floors)

   `scripts/check-standards.mjs` fails CI on any drift.
3. **The gate is the merge contract.** CI's `gate` job runs the same scripts
   as `make check`. Green means mergeable, so never weaken a check to get
   green.
4. **pnpm only** (pinned via `packageManager`, run through corepack). Never
   create a `package-lock.json`.
5. **One cargo workspace at the root, one `Cargo.lock`.** Every crate is a
   member of the root `Cargo.toml` except the Tauri desktop shells, which
   each keep their own workspace and lockfile. CI builds with `--locked`, so
   commit lockfile changes along with the dependency change.
6. **No employer or client affiliation, anywhere.** This repo is public and
   personal: no company names, private org/repo names, internal hosts,
   ticket keys or work emails, in code, tests, docs, fixtures or commit
   metadata. Use neutral examples (`acme/widgets`, `ABC-123`,
   `tracker.example.com`) and make integrations configurable instead of
   hard-coding a destination. `scripts/check-affiliation.sh` enforces this
   from patterns kept outside the repo. No personal email either: commit
   with your GitHub noreply address (`scripts/check-commit-email.sh`).
7. **Every PR carries a `release:*` label** when it changes an app:
   `release:patch` (the default), `release:minor` or `release:major`. Merging
   to `main` releases only the apps the PR changed, as
   `beagle-<app>-vX.Y.Z` (see `docs/RELEASING.md`).

## Verify before you finish

Run these from the repo root. Each runs exactly what CI runs:

```sh
make standards          # 400-line rule, affiliation guard, shared-config drift, CI-script tests
make check-rust         # the root cargo workspace (every crate but the Tauri shells)
make check-tandem       # or check-rca / check-agent: root workspace + that app's desktop shell + frontends
make check-ios          # BeagleKit + the Beagle app (macOS + Xcode): swift-format, tests, coverage, DocC, analyzer
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
  - `.` (the root `Cargo.toml`): `utils`, `beagle`, `beagle/serve`, `scout/tui`, `recap/tui`, `standup/tui`, `rca/tui`,
    `rca/daemon`, `rca/lib/agent`, `tandem/tui`, `tandem/lib/*`, `agent/tui`, `agent/daemon`, `agent/lib/*`
  - `tandem/desktop/src-tauri`
  - `rca/desktop/src-tauri`
  - `agent/desktop`
- **Frontends:**
  - `tandem/desktop`
  - `rca/desktop`
  - `rca/web`
  - `agent/desktop`
- **iPhone apps and Swift packages** (Xcode, macOS only): the Beagle app `ios`
  and the shared package `utils/swift` (BeagleKit), each gated by
  `scripts/ci/ios.sh` (`make check-ios`, CI job `ios` on a macOS runner).

To add a workspace or frontend, add it to `scripts/check-standards.mjs`,
`.github/workflows/ci.yml` and the root `Makefile` in the same PR. An iPhone
app goes in `IOS_APPS` and a Swift package in `SWIFT_PACKAGES`
(`scripts/ci/ios-standards.mjs`) instead of `check-standards.mjs`.
