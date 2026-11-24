# AGENTS.md — cli (the `beagle` command)

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first.

## What it is

The crates (members of the repo-root cargo workspace) that build the
single `beagle` binary — the one
binary users install (`brew install beagle-tools/tap/beagle`, see
[`README.md`](README.md)). Each tool is a library crate; `crates/beagle`
(package `beagle-cli`) links them all and dispatches `beagle <tool> …` to
the tool.

| Tool | Crate | Where it lives |
|---|---|---|
| `agent` | `beagle-agent-tool` → `agentctl` (which runs `agenttop` as `top` and `agentd` as `daemon run`) | `crates/agent`, over `../workflows/crates/*` — which also build the standalone binaries until they are retired |
| `rca` | `beagle` (renamed `beagle-rca` in `Cargo.toml`), plus `beagle-agentd` for `beagle rca agentd` (routed in `tools.rs`: the daemon depends on rca, so rca can't link it) | `../rca/cli`, `../rca/crates/beagle-agentd` — also build the standalone binaries until they are retired |
| `recap` | `beagle-recap` | `crates/recap` |
| `scout` | `beagle-scout` | `crates/scout` |
| `serve` | `beagle-serve` | `crates/serve` — the tools' stores as a read-only HTTPS JSON API ([`../docs/serve-api.md`](../docs/serve-api.md)); it reads each store through that tool's crate |
| `standup` | `beagle-standup` | `crates/standup` |
| `tandem` | `tandem-cli` (linked through `run_as("beagle tandem", …)`) | `../tandem/crates/tandem-cli` — also ships inside the Tandem app |

A tool that lives with its own app is a path dependency in the root
`Cargo.toml`'s `[workspace.dependencies]`. A change to it releases `beagle`
too ([`../docs/RELEASING.md`](../docs/RELEASING.md)), and its own app's
gate (`make check-rca`, `make check-tandem`, `make check-workflows`) still
applies.

**Daemons are `beagle` too.** Every long-running service is a subcommand,
and the unit it installs runs this binary: `beagle agent daemon install`
writes a launchd unit for `<beagle> agent daemon run`, `beagle rca agent
install` a launchd/systemd unit for `<beagle> rca agentd run`. Both take
the running binary's path through `beagle_utils::paths::stable_exe`, so a
Homebrew keg path never lands in a unit (it would break on upgrade). Only
the desktop apps stay separate.

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
- `setup [--yes] [--skip <id>] [--only <ids>]` (`setup.rs`): every tool's
  one-time install steps (rca skills, scout skills, scout hooks, the scout
  store), each run through the tool's own command and skipped when already
  in place. `beagle update` runs the new binary's `setup --only
  rca-skills,scout-skills` afterwards. A tool with a new install step adds
  it to `STEPS`.
- `doctor`: old standalone binaries (`scout`, `recap`, `standup`,
  `agentctl`, `agenttop`, `agentd`, another `beagle`) still on `PATH`, and
  (macOS) whether the `beagle serve` launchd agent is running — both fail
  it — plus setup checks that only warn (`health.rs`: `gh` signed in,
  agent services still running a retired binary, `tmux`, skills, scout
  hooks).

**Tools never update themselves under `beagle`.** One tool's download
would replace every other tool in the binary, so under `beagle` a tool has
no `update` (or `version list`): they are usage errors (exit 2) that name
`beagle update` (rca's `run`, tandem's `run_as`).

**One CLI shape.** Every tool, under `beagle`:
- answers `help` / `-h` / `--help` with its help and `version` / `-V` /
  `--version` with `beagle <tool> <beagle's version>` — `main.rs` answers
  these (`shared.rs`) before the tool sees them;
- takes `--root DIR` / `--root=DIR` before or after its subcommand (when it
  has a store);
- prefixes errors with `beagle <tool>: ` and exits 2 on a usage error
  (unknown command, word or flag), 1 on a failure.

## Commands

```sh
make check-cli     # from the repo root: fmt, clippy, test, doc
cargo run -p beagle-cli -- help
cargo run -p beagle-cli -- rca list --root ../rca
```

## Releases

`beagle` releases as `beagle-cli-vX.Y.Z` (tarball `beagle-cli-<target>.tar.gz`
holding `beagle`; formula `beagle`). The version is each
`crates/*/Cargo.toml`'s `[package] version`; CI stamps them (and their
root `Cargo.lock` entries) with `scripts/set-version.mjs` (tested by
`scripts/set-version.test.mjs`, run by `make test-scripts`) and never
commits the result. See [`../docs/RELEASING.md`](../docs/RELEASING.md).
