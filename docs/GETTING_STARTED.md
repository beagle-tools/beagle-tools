# Getting started

From nothing to every Beagle tool running on your machine. There are two
paths, and you can mix them:

- **[Use the tools](#path-a-use-the-tools)**: install the released binaries
  and apps. About five minutes.
- **[Hack on the repo](#path-b-hack-on-the-repo)**: clone, bootstrap and run
  everything from source.

Coding agents setting up a machine should read
[the section for agents](#for-coding-agents) too: every step below is
non-interactive except where that section says it isn't.

## What you need

| You want | You need |
|---|---|
| The `beagle` CLI and its tools | macOS (arm64 or x86_64) or Linux x86_64. Homebrew is the easy route |
| GitHub-backed tools (recap, tandem, rca's fix PRs) | [`gh`](https://cli.github.com), logged in: `gh auth login` |
| An agent to do the work | [Claude Code](https://claude.com/claude-code) (`claude`) or Codex (`codex`) on your `PATH` |
| `beagle agent` (agentd) | `tmux`. Every run gets its own session |
| The desktop apps | macOS for Agentdesk and Beagle RCA desktop; macOS, Linux or Windows for Tandem |
| The iPhone app | A Mac with Xcode 16+, an Apple developer account, and Tailscale on the Mac and the phone |

## Path A: use the tools

### 1. Install `beagle`

```sh
brew install beagle-tools/tap/beagle
```

No Homebrew? Use a release tarball or cargo. Both are in
[`beagle/README.md`](../beagle/README.md#install).

### 2. Run the one-time setup

```sh
beagle setup     # asks before each step; --yes runs them all
beagle doctor    # checks gh, tmux, the agent services, skills and hooks
```

`beagle setup` runs each tool's own install step and skips any that are
already done:

| Step | Runs | What it does |
|---|---|---|
| `rca-skills` | `beagle rca skill install` | Installs the `/beagle` and `/beagle-review` agent skills |
| `scout-skills` | `beagle scout skill install` | Installs the `/scout` and `/scout-build` agent skills (plus, on macOS, Scout's notifier) |
| `scout-hooks` | `beagle scout hook install` | Adds hooks to `~/.claude/settings.json` so `beagle scout watch` can follow Claude Code sessions |
| `scout-store` | `beagle scout init ~/.scout` | Creates the default scout store |

Use `--only <ids>` or `--skip <id>` to pick steps. `beagle doctor` changes
nothing. It warns about anything missing and about old standalone binaries
(`agentctl`, `agentd`, `beagle-agentd` and so on) still on your `PATH`.

### 3. Start the background services you want (optional)

```sh
beagle agent daemon install   # macOS: agentd, the scheduler, under launchd
beagle rca agent install      # the rca daemon that turns handed-off RCAs into fix PRs
```

Skip either one if you don't need it. The CLIs and TUIs work without them.

### 4. Install the desktop apps (optional)

```sh
brew install --cask beagle-tools/tap/beagle-tandem       # Tandem: PR review with agents
brew install --cask beagle-tools/tap/beagle-rca-desktop  # Beagle RCA desktop viewer
brew install --cask beagle-tools/tap/beagle-agentdesk    # Agentdesk: agentd's dashboard
```

The desktop apps update themselves from inside the app. Tandem has install
scripts and direct downloads for Linux and Windows in
[`tandem/README.md`](../tandem/README.md#install).

### 5. Take each tool for a walk

| Tool | First thing to try | Guide |
|---|---|---|
| scout | In Claude Code: `/scout <a goal>`, then `beagle scout` in another terminal | [`scout/`](../scout/README.md) |
| rca | `beagle rca new my-first-rca --title "Checkout times out"`, then `beagle rca` | [`rca/`](../rca/README.md) |
| recap | `beagle recap new https://github.com/<owner>/<repo>/pull/<n>`, ask an agent to write the recap, then `beagle recap` | [`recap/`](../recap/README.md) |
| standup | Ask an agent to follow `beagle standup guide`, then `beagle standup` | [`standup/`](../standup/README.md) |
| agent | Copy the demo manifests and run the free `writeup` job (below) | [`agent/`](../agent/README.md) |
| tandem | Open the app, sign in with `gh`, paste a PR URL | [`tandem/`](../tandem/README.md) |

The agentd demo:

```sh
mkdir -p ~/.config/agentd/manifests
cp -R agent/examples/demo ~/.config/agentd/manifests/   # from a checkout of this repo
beagle agent apply
beagle agent -j writeup
beagle agent              # the live dashboard
```

### 6. Put it on your phone (optional)

`beagle serve` exposes every tool's data read-only over HTTPS on your
Tailscale address, and the Beagle iPhone app reads it. Set up the server with
[`beagle/serve/README.md`](../beagle/serve/README.md), then build the app and
pair it with [`ios/README.md`](../ios/README.md). Push notifications work on
the Mac side (`beagle serve push setup`), but the app doesn't register for
them yet ([`docs/serve-push.md`](serve-push.md)).

### Keeping up to date

```sh
beagle update --check   # what's installed vs the latest release
beagle update           # every terminal tool at once (runs brew upgrade under Homebrew)
```

## Path B: hack on the repo

### 1. Clone and bootstrap

```sh
git clone https://github.com/beagle-tools/beagle-tools.git
cd beagle-tools
make bootstrap-check    # report what's missing; changes nothing
make bootstrap          # install it: Rust, Node (via nvm), pnpm, gh, tmux, jq, frontend deps
gh auth login           # once
```

`make bootstrap` is safe to re-run. It is macOS only. On Linux, install the
same tools by hand (the table in
[`docs/DEVELOPMENT.md`](DEVELOPMENT.md#1-set-up-the-machine) lists them).

### 2. Build and run

```sh
cargo run -p beagle-cli -- help        # the beagle command, from source
cargo install --locked --path beagle   # or put your build on PATH as `beagle`
make dev APP=tandem                    # any app; scripts/dev/run.sh --list shows them all
make dev APP=tandem BROWSER=1          # a desktop frontend in the browser, with mocked IPC
```

A source-built `beagle` and a Homebrew one share the same config and data
directories. Keep only one on your `PATH`. `beagle doctor` flags old
binaries that shadow the one you're running.

### 3. Check before you push

```sh
make standards       # the 400-line rule, affiliation guard, shared-config drift
make check-<app>     # check-rust, check-tandem, check-rca, check-agent, check-ios
make test-affected   # only the cargo tests your change touches
make check           # everything CI's gate runs
```

Then read [`CONTRIBUTING.md`](../CONTRIBUTING.md) and
[`docs/SDLC.md`](SDLC.md) for how a change travels from issue to release.

### Optional: the affiliation guard

`scripts/check-affiliation.sh` keeps names that don't belong in a public
repo out of it. Its patterns are never committed. Keep your own in
`~/.config/beagle-tools/affiliation-patterns`, one extended regex per line.
Without that file the check warns and passes locally. CI always runs it.

## For coding agents

- Start at [`AGENTS.md`](../AGENTS.md), then the `AGENTS.md` of the app you
  are changing.
- Every `make` and `scripts/` entry point puts cargo and nvm's node on
  `PATH` itself, so a fresh non-login shell works.
- Commands that need a person:
  - `gh auth login`
  - the first `xcode-select --install`
  - Xcode signing for the iPhone app
  - `beagle setup` without `--yes` (it asks per step and refuses to run
    without a terminal)
- Use `beagle setup --yes`, `make bootstrap-check` (exit code 1 when
  something is missing) and `beagle doctor` to set up and verify a machine
  without prompts.
- Don't run two copies of a daemon against the same store. Stop the
  installed service, or point `XDG_STATE_HOME` at a scratch directory, before
  you run `make dev APP=agentd` or `APP=rca-agentd`.
- Never commit secrets, tokens or personal or employer details. Commit with
  your GitHub noreply address (`scripts/check-commit-email.sh`).
