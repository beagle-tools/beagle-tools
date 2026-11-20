# AGENTS.md — cli (the `beagle` command)

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first.

## What it is

One cargo workspace that builds the single `beagle` binary — the one
binary users install (`brew install beagle-tools/tap/beagle`, see
[`README.md`](README.md)). Each tool is a library crate; `crates/beagle`
(package `beagle-cli`) links them all and dispatches `beagle <tool> …` to
the tool.

| Tool | Crate | Where it lives |
|---|---|---|
| `rca` | `beagle` (renamed `beagle-rca` in `Cargo.toml`) | `../rca/cli` — also builds the standalone rca binary until it is retired |
| `recap` | `beagle-recap` | `crates/recap` |
| `scout` | `beagle-scout` | `crates/scout` |
| `serve` | `beagle-serve` | `crates/serve` — the tools' stores as a read-only HTTPS JSON API ([`../docs/serve-api.md`](../docs/serve-api.md)); it reads each store through that tool's crate |
| `standup` | `beagle-standup` | `crates/standup` |
| `tandem` | `tandem-cli` (linked through `run_as("beagle tandem", …)`) | `../tandem/crates/tandem-cli` — also ships inside the Tandem app |

A tool that lives with its own app is a path dependency in the workspace
`Cargo.toml`. A change to it releases `beagle` too
([`../docs/RELEASING.md`](../docs/RELEASING.md)), and its own gate
(`make check-rca`, `make check-tandem`) still applies.

## The tool contract

A tool crate exposes, from its `lib.rs`:

```rust
/// One line for `beagle help`.
pub const ABOUT: &str = "…";
/// Run with the arguments AFTER the tool's name; `--help` must work.
pub fn run(args: Vec<std::ffi::OsString>) -> std::process::ExitCode;
```

To add a tool: create `crates/<tool>/` (or path-depend on it), add it as a
dependency of `crates/beagle`, and add one entry to `TOOLS` in
`crates/beagle/src/tools.rs` (alphabetical). A tool never reads
`std::env::args` itself, so it behaves the same standalone and under
`beagle`. Its help and hints say `beagle <tool> …`.

Tools keep their own on-disk formats, config files and environment
variables. Shared helpers belong in `../utils` (std-only `beagle-utils`, or
`utils/tui` for terminal UI pieces), not copied between tools.

## The binary's own commands

`crates/beagle/src/main.rs` answers these before looking up a tool, so no
tool may be named after one:

- `help [tool]`, `version` / `--version` (version plus release target, or
  "source build").
- `update [--version X.Y.Z] [--check]` and `version list`
  (`selfupdate/`): the `beagle-cli-vX.Y.Z` releases, through
  `beagle_utils::update` and `beagle_tui::version_picker`. A Homebrew
  install runs `brew upgrade beagle-tools/tap/beagle`; a source build
  prints the `cargo install` that rebuilds it. Release CI marks release
  builds with `BEAGLE_RELEASE_TARGET`; anything without it is a source build.
- `doctor`: old standalone binaries (`scout`, `recap`, `standup`, another
  `beagle`) still on `PATH`.

**Tools never update themselves under `beagle`.** One tool's download
would replace every other tool in the binary, so a tool's own `update`
refuses and points at `beagle update` when linked here (rca's `run`,
tandem's `run_as`).

## Commands

```sh
make check-cli     # from the repo root: fmt, clippy, test, doc
cargo run -p beagle-cli -- help
cargo run -p beagle-cli -- rca list --root ../rca
```

## Releases

`beagle` releases as `beagle-cli-vX.Y.Z` (tarball `beagle-cli-<target>.tar.gz`
holding `beagle`; formula `beagle`). The version is `[workspace.package]`
in `Cargo.toml`; CI stamps it with `scripts/set-version.mjs` (tested by
`scripts/set-version.test.mjs`, run by `make test-scripts`) and never
commits the result. See [`../docs/RELEASING.md`](../docs/RELEASING.md).
