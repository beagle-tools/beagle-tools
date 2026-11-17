# AGENTS.md — cli (the `beagle` command)

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first.

## What it is

One cargo workspace that builds the single `beagle` binary. Each tool is a
library crate in `crates/<tool>/`; `crates/beagle` (package `beagle-cli`)
links them all and dispatches `beagle <tool> …` to the tool.

## The tool contract

A tool crate exposes, from its `lib.rs`:

```rust
/// One line for `beagle help`.
pub const ABOUT: &str = "…";
/// Run with the arguments AFTER the tool's name; `--help` must work.
pub fn run(args: Vec<std::ffi::OsString>) -> std::process::ExitCode;
```

To add a tool: create `crates/<tool>/`, add it as a dependency of
`crates/beagle`, and add one entry to `TOOLS` in
`crates/beagle/src/tools.rs`. A tool never reads `std::env::args` itself, so
it behaves the same standalone and under `beagle`.

Tools keep their own on-disk formats, config files and environment
variables. Shared helpers belong in `../utils` (std-only `beagle-utils`, or
`utils/tui` for terminal UI pieces), not copied between tools.

## Commands

```sh
make check-cli     # from the repo root: fmt, clippy, test, doc
cargo run -p beagle-cli -- help
```
