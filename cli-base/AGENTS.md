# cli-base (`beagle`) — agent guide

Read [`../AGENTS.md`](../AGENTS.md) and
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md) first; they hold
the repo-wide rules and the gate. This file only covers what is specific to
the base CLI.

`beagle` is one command that every Beagle tool plugs into. It does three
things and nothing else:

1. installs, updates, pins and removes **extensions** (`beagle extensions …`);
2. dispatches `beagle <ext> …` to that extension's own binary;
3. updates itself (`beagle update`).

Each app owns its CLI and ships it as an extension tarball. The base never
parses an extension's arguments. Design and phasing: issue #2.

## Module map (`src/`)

| File | Job |
|---|---|
| `main.rs` | Builds the `Context` from the environment, then exits or execs. |
| `lib.rs` | `Context` (every path/env input, injectable for tests), `Outcome`. |
| `cli.rs` | Hand-rolled parser and `USAGE` (no clap, like rca). Extension args stay `OsString`. |
| `commands/` | One file per command group: `mod.rs` (help/version/doctor/dispatch), `extensions.rs`, `install.rs` (install/update + picker), `self_update.rs`. |
| `registry.rs` | The closed, compiled-in extension set, `RESERVED` names, `BASE`. |
| `manifest.rs` | `extension.toml` (schema 1). |
| `releases.rs`, `version.rs` | GitHub release listing per tag prefix; `X.Y.Z` parsing. |
| `install/` | `mod.rs` (the staged pipeline), `download.rs`, `extract.rs`, `swap.rs` (`current`/prune/uninstall), `lock.rs`. |
| `dispatch.rs` | Resolution order and the env contract; `exec`. |
| `doctor.rs`, `state.rs` | `beagle doctor`; the cached update check behind the notice. |
| `picker.rs`, `fuzzy.rs` | The version picker (pure state machine + crossterm loop). `fuzzy.rs` is a copy of rca's until rca drops its updater (phase 3). |
| `tool.rs` | Running curl/tar/shasum with a timeout and kill path. |

## Concurrency model

Synchronous: one process, no async runtime, no threads beyond the two pipe
readers `tool.rs` uses per child. Every child process (curl, tar, shasum, a
post-update hook) has a timeout and is killed when it overruns. Two `beagle`
processes never install the same extension at once: `locks/<ext>.lock` is an
OS file lock (released even when the holder is killed), and a second process
waits up to 30 s for it and then fails.

## Public contracts

These are read by extensions, users' scripts and the release pipeline.
Changing one is a breaking change: it needs a migration note in the PR, an
update here, and a bump of `manifest::PROTOCOL` or `manifest::SCHEMA` when it
affects extensions.

### Dispatch (`beagle <name> args…`)

1. A reserved name (`extensions ext update version help doctor completions
   self`) is a base command, never an extension.
2. Aliases resolve to the canonical name.
3. A registry name runs `$EXT_DIR/<name>/current/bin/<bin>` (`bin` from its
   `extension.toml`), or fails with exit 1 and an install hint if it isn't
   installed.
4. Otherwise `beagle-<name>` on `PATH` runs (the dev escape hatch).
5. Otherwise exit 2: `unknown command "<name>"`, with a "did you mean" hint.

The base **execs** the binary with the arguments byte-for-byte. Only
`beagle <ext> update [--version <v>]` (registry extensions only) is
intercepted. Before exec it sets:

| Var | Value |
|---|---|
| `BEAGLE_BASE` | absolute path of the `beagle` binary |
| `BEAGLE_BASE_VERSION` | base semver |
| `BEAGLE_EXTENSION` | canonical extension name (`rca`) |
| `BEAGLE_EXTENSION_VERSION` | active version (unset for `PATH` binaries) |
| `BEAGLE_EXTENSION_DIR` | `$EXT_DIR/<name>/<version>` (unset for `PATH` binaries) |
| `BEAGLE_INVOKED_AS` | how the user typed it, e.g. `beagle rca` |

Extensions must work without these (developers run `beagle-rca` directly);
they only use them to print `beagle rca …` in help and hints.

### On-disk layout (`$XDG_DATA_HOME/beagle`, else `~/.local/share/beagle`)

```text
extensions/<ext>/<version>/bin/<bin>, <extra_bins>
extensions/<ext>/<version>/extension.toml   copied from the tarball
extensions/<ext>/<version>/install.toml     tag, sha256, source, installed_at (RFC 3339)
extensions/<ext>/current -> <version>       relative symlink, swapped by rename
extensions/<ext>/.staging-<pid>/            an install in progress
state.toml                                  last release check + notice time per extension
locks/<ext>.lock                            install lock
```

Install: download to staging → verify sha256 → extract → validate manifest
→ rename to `<version>/` → swap `current` → prune (keeps 1 previous). Any
failure deletes staging and leaves `current` untouched; the next run deletes
stale `.staging-*` dirs.

### `extension.toml` (schema 1, `deny_unknown_fields`)

```toml
schema = 1
name = "rca"                         # must match the registry entry
version = "0.29.0"                   # must match the release tag
protocol = 1                         # refused if > manifest::PROTOCOL
min_base_version = "0.1.0"
bin = "beagle-rca"                   # entrypoint; plain file name
extra_bins = ["beagle-rca-daemon"]   # optional, installed alongside
description = "…"
post_update = ["daemon", "restart"]  # optional; run as `<bin> <args>` after an update
```

### Registry ⇄ release scripts

`registry.rs` must match `scripts/release/apps.sh`: tag prefix
`beagle-<release app>-v`, `APP_EXT_ASSET` (extensions) or `APP_ASSET_PREFIX`
(the base), and the published targets. `registry_matches_the_release_scripts`
parses `apps.sh` and fails on drift. Extension tarballs are named
`<APP_EXT_ASSET>-<target>.tar.gz` with a `.sha256` beside them and hold the
binaries plus `extension.toml` at the root.

### Release

App `cli` in `scripts/release/`: tags `beagle-cli-vX.Y.Z`, mirror
`beagle-cli-latest`, asset `beagle-cli-<target>.tar.gz` holding only
`beagle`. CI stamps the version with `node scripts/set-version.mjs X.Y.Z`
(from `cli-base/`); never bump it by hand. There is no Homebrew formula yet
(issue #2, phase 5); `beagle update` refuses to replace a binary under a
Homebrew `Cellar`.

`BEAGLE_RELEASES_URL` / `BEAGLE_DOWNLOAD_BASE` point listings and downloads
at a mirror (`file://` works). `BEAGLE_NO_UPDATE_CHECK=1` or `CI` turns the
update notice off.

## Rules

- **The dispatch path never touches the network.** The update notice reads
  the cached `state.toml` only; listings refresh it.
- **No new runtime dependencies** without a justification in the PR. Today:
  `serde`, `toml`, `serde_json`, `thiserror`, `crossterm`, `beagle-utils`.
  No clap, anyhow, tokio or HTTP/TLS crate: downloads shell out to `curl`.
- **Tests never use the network or the real `$HOME`.** Use
  `src/tests/fixture.rs` (a `file://` release server in a tempdir) and a
  `Layout` rooted in a tempdir. `tests/end_to_end.rs` drives the public `run`
  with a test registry, and the real binary for exec behaviour.

## Commands (from the repo root)

```sh
make check-cli-base                   # the CI gate for this workspace
cargo run --manifest-path cli-base/Cargo.toml -- extensions available
node --test cli-base/scripts/         # set-version tests (also in make standards)
```
