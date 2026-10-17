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
| `.claude/skills/` | The `beagle` / `beagle-review` skills (rca authoring); embedded into the rca binary | `rca/AGENTS.md` |

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

## Verify before you finish

Run these from the repo root. Each runs exactly what CI runs:

```sh
make standards          # 400-line rule + shared-config drift
make check-tandem       # or check-rca / check-workflows / check-utils
make check              # everything
```

The scripts behind these:
- `scripts/ci/rust.sh <dir>`: fmt, clippy `-D warnings`, test, and doc
  `-D warnings`, all `--locked`.
- `scripts/ci/frontend.sh <dir>`: frozen install, prettier, eslint, tsc,
  vitest and build.

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
