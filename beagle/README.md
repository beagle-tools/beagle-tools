# beagle

One command for every beagle-tools terminal tool and daemon. Install one
binary, `beagle`, and each tool is a subcommand. One leash, a whole pack of
tools:

| Command | What it does |
|---|---|
| `beagle agent` | Schedule and run coding agents (agentd): the live dashboard (bare, or `top`), the control CLI (`apply`, `get`, `-j <job>`, `logs`, `errors`, …) and the daemon (`daemon run`, `daemon install`) ([`../agent/`](../agent/README.md)) |
| `beagle rca` | Root-cause analysis workspaces: a live TUI plus commands for agents, and the daemon that implements handed-off RCAs (`rca agentd run`, installed by `rca agent install`) ([`../rca/`](../rca/README.md)) |
| `beagle recap` | PR recaps: the diff plus a plain-language write-up ([`../recap/`](../recap/README.md)) |
| `beagle scout` | Watch agent work as projects of small, ordered tasks ([`../scout/`](../scout/README.md)) |
| `beagle serve` | Every tool's data, read-only over HTTPS on your Tailscale address, for the phone app ([`serve/README.md`](serve/README.md)) |
| `beagle standup` | Standup reports: an overview, then a short write-up per ticket ([`../standup/`](../standup/README.md)) |
| `beagle tandem` | Review PRs in a terminal queue (no arguments opens the TUI; `add`, `list`, `claim`, `ingest`, `fail`), read Tandem's local review data as JSON, and queue reviews ([`../tandem/docs/CLI.md`](../tandem/docs/CLI.md)) |

`beagle` itself answers five commands of its own: `help`, `version`,
`update`, `setup` and `doctor`. Everything else is a tool.

## Quick start

```sh
brew install beagle-tools/tap/beagle   # or see "Install" for other ways
beagle setup                           # one-time install steps for every tool (asks before each)
beagle doctor                          # sniff out anything missing
beagle scout                           # open a tool; `beagle help` lists them all
```

Setting up a whole machine from scratch (every tool, the desktop apps, the
phone)? See [`../docs/GETTING_STARTED.md`](../docs/GETTING_STARTED.md).

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
cargo install --locked --path beagle      # from a checkout
```

A source build needs the Rust toolchain pinned in `rust-toolchain.toml`
(rustup picks it up); see [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md).

## Set up: `beagle setup`

Some tools need a one-time step outside the binary: agent skills copied
where Claude Code and Codex find them, hooks in Claude Code's settings, a
store folder. `beagle setup` runs every one of those steps through the
tool's own command, so it does exactly what the documented commands do.

| Step id | What it does | Runs |
|---|---|---|
| `rca-skills` | installs the `/beagle` and `/beagle-review` agent skills | `beagle rca skill install` |
| `scout-skills` | installs the `/scout` and `/scout-build` agent skills (and, on macOS, Scout's notifier app) | `beagle scout skill install` |
| `scout-hooks` | records Claude Code sessions for `beagle scout watch` (adds hooks to `~/.claude/settings.json`, keeping a one-time backup) | `beagle scout hook install` |
| `scout-store` | creates the default scout store at `~/.scout` | `beagle scout init ~/.scout` |

```sh
beagle setup                         # asks [Y/n] before each step that isn't done yet
beagle setup --yes                   # run them all without asking (needed when not on a terminal)
beagle setup --skip scout-hooks      # leave one out (repeatable)
beagle setup --only rca-skills,scout-skills
```

Steps already in place print `✓ … already in place` and are skipped, so
running it again is always safe. `beagle update` re-runs
`setup --only rca-skills,scout-skills` with the new binary, so the skills
always match the CLI they drive.

What `setup` does **not** do (each is a choice, not a default):

- `beagle serve` for the phone app: Tailscale, a cert, pairing
  ([`serve/README.md`](serve/README.md)).
- Background agent daemons: `beagle agent daemon install`,
  `beagle rca agent install`.
- Store locations for recap and standup: they work from the current folder
  unless you set `root` in their config ([`../recap/`](../recap/README.md),
  [`../standup/`](../standup/README.md)).

## Check: `beagle doctor`

```sh
beagle doctor
```

It prints one line per check, `✓` fine, `⚠` a warning with the fix, `✗` a
failure:

- **Fails (non-zero exit):** an old standalone binary still on `PATH`
  (`scout`, `recap`, `standup`, `agentctl`, `agenttop`, `agentd`, or another
  `beagle` that shadows or duplicates this one), or, on macOS, the
  `beagle serve` background service installed but not running.
- **Warns only:** `gh` missing or signed out (recap, standup's agent
  workflow and tandem use it), an agent service still running a retired
  binary, `tmux` missing (scheduled agent runs need it), agent skills
  missing or stale, scout's hooks not installed, push notifications not set
  up (only once `beagle serve install` has run).

It changes nothing. It never runs the old binaries it finds on `PATH`;
its only probes are `gh auth status` and, on macOS, `launchctl print`.

## Use

```sh
beagle help              # every tool, plus the commands below
beagle help rca          # one tool's own help (same as `beagle rca --help`)
beagle rca               # open the RCA TUI
beagle rca list --json   # any tool's subcommands, after its name
beagle version           # the version, and the release target or "source build"
beagle scout --version   # every tool reports beagle's version
```

Every tool, under `beagle`, has the same shape: `help`/`-h`/`--help`,
`version`/`-V`/`--version`, `--root DIR` (before or after the subcommand)
for tools with a store, errors prefixed `beagle <tool>: `, and exit codes
`0` ok, `1` failure, `2` usage error.

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

### Switching from the standalone tools

`beagle` replaces the old standalone binaries. Every command moved under
its tool's name:

| Before | Now |
|---|---|
| `beagle list`, `beagle new …` (the rca CLI, `brew install beagle-tools/tap/beagle-rca`) | `beagle rca list`, `beagle rca new …` |
| `scout`, `recap`, `standup` | `beagle scout`, `beagle recap`, `beagle standup` |
| `tandem-cli …` | `beagle tandem …` (the copy inside the Tandem app keeps working) |
| `agentctl …` | `beagle agent …` |
| `agenttop` | `beagle agent` (or `beagle agent top`) |
| `agentd` | `beagle agent daemon run` |
| `beagle-agentd` | `beagle rca agentd run` |

The `beagle-rca` formula also installs a `beagle`, so the two formulas
conflict. Switch with:

```sh
brew uninstall beagle-rca beagle-workflows
brew install beagle-tools/tap/beagle
beagle setup                  # agent skills (which now say `beagle rca …`), scout hooks + store
beagle agent daemon install   # repoint launchd from agentd at `beagle agent daemon run`
beagle rca agent install      # repoint the rca daemon's service at `beagle rca agentd run`
beagle doctor                 # checks the setup; lists old standalone binaries still on your PATH
```

The old names (`agentctl`, `agenttop`, `agentd`, `beagle-agentd`) keep
working for one more release so services installed before the switch don't
break; `beagle doctor` warns while a service still runs one of them.

Agent scripts that start other runs should call `${AGENTD_CTL:-agentctl}
run …`: the daemon sets `$AGENTD_CTL` to the control command from its own
install (`<beagle> agent`).

## Troubleshooting

| Symptom | Fix |
|---|---|
| `beagle rca` (or another tool) says "unknown command", or behaves like an old version | Another `beagle` is earlier on `PATH` (often the old `beagle-rca` formula). `beagle doctor` names it; remove it (`brew uninstall beagle-rca`). |
| `beagle setup: not a terminal to ask on` | You ran it from a script or agent: pass `--yes`. |
| `/scout` or `/beagle` missing in Claude Code | `beagle setup --only rca-skills,scout-skills`, then start a new Claude Code session. |
| `beagle doctor` says a service still runs `agentd` / `beagle-agentd` | Run the command it prints (`beagle agent daemon install` or `beagle rca agent install`) to repoint it at `beagle`. |
| `beagle update` only prints a `cargo install` line | It's a source build; run that line, or `beagle update --version X.Y.Z` to switch to a release binary. |

## For agents

- `beagle help` lists the tools; `beagle <tool> --help` is the full,
  current CLI. Trust it over any copy in a doc.
- `beagle setup --yes` and `beagle doctor` are safe to run unattended:
  setup skips finished steps, doctor only reads.
- Prefer `--json` where a tool offers it (`beagle scout list --json`,
  `beagle recap list --json`, `beagle standup list --json`).

## Develop

See [`AGENTS.md`](AGENTS.md) for the tool contract and how to add a tool,
and [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md) for the toolchain.

```sh
make check-beagle                       # from the repo root: fmt, clippy, test, doc
cargo run -p beagle-cli -- help         # from anywhere in the repo
```
