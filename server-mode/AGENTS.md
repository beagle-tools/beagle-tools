# AGENTS.md — server-mode (`beagle server-mode`)

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first. What it
does for a user: [`README.md`](README.md).

## Layout

`tui/` is the crate `beagle server-mode` links (package
`beagle-server-mode`, lib `server_mode`). It is a CLI only; there is no
terminal UI (`standards/tui-standards.json` lists it under `not_tuis`).

```text
src/lib.rs      ABOUT, run, the help text; non-macOS prints "macOS only"
src/cli.rs      arguments → Cmd (pure)
src/session.rs  on / off / status / log / lock over a Ctx
src/state.rs    the state dir (one value per file), fmt_dur, tail
src/procs.rs    Programs, spawn_detached (own process group), alive, stop
src/tests/      parsing and state; session.rs runs the commands on fakes
```

## Rules

- **Never run the real system programs in tests.** `Ctx` takes a
  `Programs`; tests point `caffeinate`, `osascript`, `pmset` and
  `sysadminctl` at fake scripts in a temp dir, and the state dirs at temp
  dirs. A test that starts processes stops them before it ends.
- **No `unsafe`**: signals go through the `kill` program, process groups
  through `CommandExt::process_group`. Every helper command is bounded by
  `beagle_utils::process::output_with_timeout`.
- **Keep the old script's session working**: `status`, `log` and `off` read
  `~/.local/state/server-mode/` too, and `on` refuses while it is on.
- **The timer runs `<beagle> server-mode off`** through `exec`, so `off`
  finds its own pid in `timer.pid` and does not kill itself. The binary
  path comes from `beagle_utils::paths::stable_exe`, so a Homebrew upgrade
  doesn't break a running timer.
- Errors start with `beagle server-mode: `; usage errors exit 2, failures 1.

## Commands

```sh
cargo test -p beagle-server-mode
cargo run -p beagle-cli -- server-mode --help
```
