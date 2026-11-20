# beagle

One command for every beagle-tools terminal tool. Install one binary,
`beagle`, and each tool is a subcommand:

| Command | What it does |
|---|---|
| `beagle rca` | Root-cause analysis workspaces: a live TUI plus commands for agents ([`../rca/`](../rca/README.md)) |
| `beagle recap` | PR recaps: the diff plus a plain-language write-up ([`crates/recap/`](crates/recap/README.md)) |
| `beagle scout` | Watch agent work as projects of small, ordered tasks ([`crates/scout/`](crates/scout/README.md)) |
| `beagle serve` | Every tool's data, read-only over HTTPS on your Tailscale address, for the phone app ([`../docs/serve-api.md`](../docs/serve-api.md)) |
| `beagle standup` | Standup reports: an overview, then a short write-up per ticket ([`crates/standup/`](crates/standup/README.md)) |
| `beagle tandem` | Review PRs in a terminal queue (no arguments opens the TUI; `add`, `list`, `claim`, `ingest`, `fail`), read Tandem's local review data as JSON, and queue reviews ([`../tandem/docs/CLI.md`](../tandem/docs/CLI.md)) |

## Install

With Homebrew (macOS, Linux x86_64):

```sh
brew install beagle-tools/tap/beagle
```

From a release tarball (macOS arm64/x86_64, Linux x86_64 — static musl);
the archive holds `beagle` at its root, with a `.sha256` beside it:

```sh
curl -fsSL https://github.com/beagle-tools/beagle-tools/releases/download/beagle-cli-latest/beagle-cli-aarch64-apple-darwin.tar.gz \
  | tar xz -C /usr/local/bin beagle
```

Swap the target for `x86_64-apple-darwin` or `x86_64-unknown-linux-musl`.

With cargo (the repo is a monorepo, so name the package):

```sh
cargo install --locked --git https://github.com/beagle-tools/beagle-tools beagle-cli
cargo install --locked --path cli/crates/beagle      # from a checkout
```

### Switching from the standalone tools

`beagle` replaces the old standalone binaries. Every command moved under
its tool's name:

| Before | Now |
|---|---|
| `beagle list`, `beagle new …` (the rca CLI, `brew install beagle-tools/tap/beagle-rca`) | `beagle rca list`, `beagle rca new …` |
| `scout`, `recap`, `standup` | `beagle scout`, `beagle recap`, `beagle standup` |
| `tandem-cli …` | `beagle tandem …` (the copy inside the Tandem app keeps working) |

The `beagle-rca` formula also installs a `beagle`, so the two formulas
conflict. Switch with:

```sh
brew uninstall beagle-rca
brew install beagle-tools/tap/beagle
beagle setup               # agent skills (which now say `beagle rca …`), scout hooks + store
beagle doctor              # checks the setup; lists old standalone binaries still on your PATH
```

The rca agent daemon, `beagle-agentd`, is installed alongside `beagle`
(and `beagle update` updates both); set it up with `beagle rca agent install`.

## Use

```sh
beagle help              # every tool, plus the commands below
beagle help rca          # one tool's own help (same as `beagle rca --help`)
beagle rca               # open the RCA TUI
beagle rca list --json   # any tool's subcommands, after its name
beagle version           # the version, and the release target or "source build"
beagle setup             # every tool's one-time install steps (asks per step; --yes for all)
beagle doctor            # setup checks (warnings), plus old binaries that shadow this one
```

## Update

```sh
beagle update --check            # installed vs latest, changes nothing
beagle update                    # install the latest release
beagle update --version 0.2.0    # or any release, older or newer
beagle version list              # browse releases; enter installs one
```

`beagle update` downloads `beagle-cli-<target>.tar.gz` from the
`beagle-cli-vX.Y.Z` release, **verifies its sha256**, and atomically swaps
the running binary — never a half-written executable. It updates every
tool at once, so the tools' own updaters step aside under `beagle`
(`beagle rca update` and `beagle tandem update` are not commands; they
say to run this).

How it updates depends on how `beagle` was installed:

| Installed with | `beagle update` |
|---|---|
| Homebrew | runs `brew upgrade beagle-tools/tap/beagle` |
| a release tarball | downloads and swaps in place |
| `cargo install` / a source build | prints the `cargo install` that rebuilds it (`--version` still installs that release binary) |

## Develop

See [`AGENTS.md`](AGENTS.md) for the tool contract and how to add a tool.

```sh
make check-cli                       # from the repo root: fmt, clippy, test, doc
cargo run -p beagle-cli -- help      # from cli/
```
