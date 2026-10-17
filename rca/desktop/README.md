# beagle desktop

The desktop viewer for RCA workspaces: a Tauri 2 shell whose Rust backend
reuses the `cli/` crate's domain layer (store, model, markdown, similar)
and whose React frontend renders it. The on-disk format under `rcas/` is
the shared API — this app is a second renderer, not a second
implementation.

## Develop

```sh
cd rca/desktop
corepack pnpm install
corepack pnpm tauri dev   # opens the app; finds the nearest rcas/ above the cwd
```

Root resolution: config file `root` (`~/.config/beagle/config.toml`) →
nearest ancestor directory containing `rcas/` → the working directory.

## Quality gate

From the repo root, `make check-rca` runs exactly what CI runs for rca
(desktop frontend and Rust included). Standards live in
[`docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md); agent notes
for this component are in [`AGENTS.md`](AGENTS.md).

## Layout

```text
src/types.ts     IPC contract (mirrors src-tauri/src/dto.rs)
src/api.ts       all invoke() wrappers
src/lib/         pure logic + unit tests
src/components/  one component per file + render tests
src-tauri/       Rust: commands.rs (IPC surface), dto.rs (contract)
```
