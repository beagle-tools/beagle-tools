# rca/desktop — agent notes

Read [`../AGENTS.md`](../AGENTS.md) (rca) and
[`../../docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first.
This file only covers what is specific to the desktop frontend.

## What it is

A Tauri 2 viewer for the same `rcas/` workspaces the CLI/TUI reads. The
Rust backend (`src-tauri/`) does all disk access; the React frontend in
`src/` only renders what it gets over IPC. It is a second renderer, not a
second implementation — domain rules belong on the Rust side.

## Layout

```text
src/api.ts         the ONLY module that calls invoke()/listen(); add new
                   commands here as typed wrappers
src/types.ts       the IPC contract — mirrors src-tauri/src/dto.rs field for
                   field; change both in the same commit
src/App.tsx        composes the hooks; owns no data logic itself
src/hooks/         all state: useIncidents (listing, selection, tab, section
                   content), useAgents (agentd status events), usePrStates,
                   useActions, useKeybindings, useTheme
src/lib/           pure logic (filter, fuzzy finder, markdown, ansi, pr,
                   sections, keys) + vitest tests — keep it DOM-free
src/components/    one presentational component per file + render tests
src/styles/        CSS split by area; imported in cascade order from App.tsx
```

## Commands

```sh
corepack pnpm install
corepack pnpm tauri dev    # run the app (vite on :1420 + the Rust shell)
make check-rca             # from the repo root: the full rca gate
```

Releases: bump the version in `package.json`, `src-tauri/tauri.conf.json`
and `src-tauri/Cargo.toml` together, then push a `desktop-v<version>` tag;
`.github/workflows/rca-release-desktop.yml` refuses mismatched versions.
