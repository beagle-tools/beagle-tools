# Beagle desktop

The desktop viewer for Beagle RCA workspaces: the same incidents as
`beagle rca`, in a window. It watches `rcas/` and updates live as your agent
writes, tracks attached PRs, and has an **Agents** view for the
[agent daemon](../docs/agent-runtime.md). Under the hood it is a Tauri 2
shell whose Rust backend reuses the `tui/` crate's domain layer (store,
model, markdown, similar) and whose React frontend renders it. The on-disk
format under `rcas/` is the shared API: this app is a second renderer, not a
second implementation.

## Install

```sh
brew install --cask beagle-tools/tap/beagle-rca-desktop    # macOS
```

Linux builds (AppImage, `.deb`) are on the
[`beagle-rca-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-rca-latest)
release. The app updates itself: an update pill appears in the header when a
new version is out.

Point it at your store with `root` in `~/.config/beagle/rca.toml`
(`beagle rca config`). A GUI app has no useful working directory, so unlike
the CLI it does not look for `.beagle` files.

## Develop

```sh
cd rca/desktop
corepack pnpm install
BEAGLE_RCA_ROOT="$PWD/../examples" corepack pnpm tauri dev   # the example store (absolute path)
# (or `make desktop` from rca/, which sets that for you)
```

Root resolution: `$BEAGLE_RCA_ROOT` → config file `root` (`~/.config/beagle/rca.toml`, or the
older `~/.config/beagle/config.toml`) →
nearest ancestor directory containing `rcas/` → the working directory.
Running apps in a browser with mocked IPC, and other tooling, is in
[`docs/DEVELOPMENT.md`](../../docs/DEVELOPMENT.md).

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
