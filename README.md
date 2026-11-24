<div align="center">

<img src="docs/assets/beagle-logo.png" alt="Beagle logo: a beagle's head" width="120">

# Beagle

Open-source developer tools for working alongside your AI agents — local-first, on your machine,
with you in control.

<a href="https://github.com/beagle-tools/beagle-tools/actions/workflows/ci.yml"><img src="https://github.com/beagle-tools/beagle-tools/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
<a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="License: Apache-2.0"></a>
<a href="https://github.com/beagle-tools/beagle-tools/releases?q=beagle-cli-v"><img src="https://img.shields.io/github/v/release/beagle-tools/beagle-tools?filter=beagle-cli-v*&label=beagle" alt="Latest beagle CLI release"></a>
<a href="https://github.com/beagle-tools/beagle-tools/releases?q=beagle-tandem-v"><img src="https://img.shields.io/github/v/release/beagle-tools/beagle-tools?filter=beagle-tandem-v*&label=tandem" alt="Latest Tandem release"></a>
<a href="https://github.com/beagle-tools/beagle-tools/releases?q=beagle-rca-v"><img src="https://img.shields.io/github/v/release/beagle-tools/beagle-tools?filter=beagle-rca-v*&label=rca" alt="Latest Beagle RCA release"></a>
<a href="https://github.com/beagle-tools/beagle-tools/releases?q=beagle-workflows-v"><img src="https://img.shields.io/github/v/release/beagle-tools/beagle-tools?filter=beagle-workflows-v*&label=agentd" alt="Latest agentd release"></a>

</div>

## The tools

|                                                              | Product            | What it does                                                                                                               | Runs on                                              | Directory                           |
| ------------------------------------------------------------ | ------------------ | -------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------- |
| <img src="docs/assets/tandem-logo.svg" alt="" width="32">    | Tandem             | Review GitHub PRs with Claude/Codex riding along; agent comments stay local until you post                                 | macOS, Linux, Windows desktop                        | [`tandem/`](tandem/README.md)       |
| <img src="docs/assets/beagle-logo.png" alt="" width="32">    | Beagle (RCA)       | An agent investigates an incident and writes the RCA; a TUI/desktop app renders it live; a daemon can ship the fix as a PR | CLI/TUI (macOS, Linux), desktop, daemon, static site | [`rca/`](rca/README.md)             |
| <img src="docs/assets/agentdesk-logo.svg" alt="" width="32"> | agentd + Agentdesk | Schedule coding agents from YAML manifests; each run in its own tmux session, recorded in SQLite                           | macOS                                                | [`workflows/`](workflows/README.md) |
|                                                              | scout              | Watch agent work live as projects of small, ordered tasks (`beagle scout`)                                                 | CLI/TUI (macOS, Linux)                               | [`scout/`](scout/README.md)         |
|                                                              | recap              | PR recaps: the diff plus a plain-language write-up of what it fixes and why (`beagle recap`)                               | CLI/TUI (macOS, Linux)                               | [`recap/`](recap/README.md)         |
|                                                              | standup            | Standup reports: one overview, then a short write-up per ticket (`beagle standup`)                                         | CLI/TUI (macOS, Linux)                               | [`standup/`](standup/README.md)         |

Underneath them, [`utils/`](utils/README.md) (`beagle-utils`) holds the Rust helpers two or more
apps share.

Every terminal tool and daemon ships as **one binary, `beagle`** ([`beagle/`](beagle/README.md)):
`beagle agent` (agentd: CLI, TUI and daemon), `beagle rca` (and its daemon, `beagle rca agentd`),
`beagle recap`, `beagle scout`, `beagle standup` and `beagle tandem` (Tandem's data from a
script). Only the desktop apps install separately.

## Install

<details open>
<summary><b>beagle</b> — the one CLI: agent, rca, recap, scout, standup, tandem</summary>

```sh
brew install beagle-tools/tap/beagle
beagle setup                  # every tool's one-time steps: agent skills, scout hooks + store
beagle doctor                 # checks gh, agent services, tmux, skills and hooks
beagle agent daemon install   # (macOS) run the agentd scheduler under launchd
beagle help                   # every tool it holds
beagle update                 # later: update every tool at once (and refresh the skills)
```

Tarballs and `cargo install`: [`beagle/README.md`](beagle/README.md). Coming from the standalone rca
CLI (`brew install beagle-tools/tap/beagle-rca`)? Both install a `beagle`, so switch with
`brew uninstall beagle-rca && brew install beagle-tools/tap/beagle`; every old `beagle <cmd>` is
now `beagle rca <cmd>`. From the standalone agentd CLIs: `agentctl …` is `beagle agent …`,
`agenttop` is `beagle agent`, and `agentd` is `beagle agent daemon run` — re-run
`beagle agent daemon install` so launchd starts the new binary (and `beagle rca agent install`
for rca's daemon, now `beagle rca agentd run`). The old names keep working for one release. `beagle doctor` lists old
standalone binaries still on your `PATH`.

</details>

<details>
<summary><b>Tandem</b> — desktop app</summary>

```sh
brew install --cask beagle-tools/tap/beagle-tandem
```

```sh
curl -fsSL https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.sh | sh
```

```powershell
powershell -c "irm https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.ps1 | iex"
```

Direct downloads for every platform: [`tandem/README.md`](tandem/README.md#install).

</details>

<details>
<summary><b>Beagle RCA</b> — desktop app and agent daemon (the CLI/TUI is <code>beagle rca</code>)</summary>

```sh
brew install --cask beagle-tools/tap/beagle-rca-desktop   # the desktop app (macOS)
```

The agent daemon is `beagle rca agentd`, installed as a login service with
`beagle rca agent install`; other routes: [`rca/README.md`](rca/README.md#install--run).

</details>

<details>
<summary><b>Agentdesk</b> — desktop app for agentd (macOS; the CLI, TUI and daemon are <code>beagle agent</code>)</summary>

```sh
brew install --cask beagle-tools/tap/beagle-agentdesk   # the Agentdesk app (optional)
beagle agent daemon install                             # the scheduler, under launchd
```

Direct downloads and the tmux/notifier setup: [`workflows/README.md`](workflows/README.md#install).

</details>

**How updates work:** the desktop apps update themselves in-app; `beagle update` updates the
`beagle` CLI (running `brew upgrade` when Homebrew installed it); the other CLIs update with
`brew upgrade`.

## Quick start

**Tandem:** open the app → sign in with `gh` → paste a PR URL.

**Beagle** — scaffold an RCA (`--title` is required), then watch it render live:

```sh
beagle rca new <slug> --title "..."
beagle rca                # open the TUI against ./rcas
```

**agentd** — copy the [demo manifests](workflows/manifests/) into place (their scripts are
referenced from there), then run the free, script-only `writeup` job:

```sh
mkdir -p ~/.config/agentd/manifests
cp -R workflows/manifests/demo ~/.config/agentd/manifests/
beagle agent apply
beagle agent get agents
beagle agent -j writeup
beagle agent              # the live dashboard
```

## How they fit together

```mermaid
flowchart LR
    agentd["beagle agent (agentd)<br/>scheduled agent runs"]
    agent["Agent<br/>claude · codex + /beagle skill"]
    rca[("Beagle RCA workspace<br/>plain files on disk")]
    viewer["beagle rca TUI · Beagle desktop<br/>live render"]
    bagentd["beagle rca agentd<br/>implements remediation.md"]
    pr["Pull request"]
    tandem["Tandem<br/>review with agents"]
    gh["GitHub"]

    agentd -- "runs on a schedule or on demand" --> agent
    agent -- "investigates, writes the RCA" --> rca
    rca -- "watched live" --> viewer
    rca -- "you set status = agent" --> bagentd
    bagentd -- "opens" --> pr
    pr -- "you open it in" --> tandem
    tandem -- "explicit click: post / approve" --> gh
```

Each tool stands on its own; together they cover an incident end to end. An agent — started by
you or by agentd — writes its investigation into a Beagle workspace that you watch live. When you
hand the RCA off, `beagle rca agentd` implements the fix and opens a PR, which you review in Tandem
before anything reaches GitHub.

## Principles

- **Local-first:** SQLite and plain files on your disk; no hosted service in the loop.
- **You stay in control:** agents never post, approve or merge on their own — you click.
- **Bring any agent:** Claude, Codex, or a custom command.
- **One strict standard:** Rust with no `unsafe`, zero-warning builds, 400-line files, one CI
  `gate`.

## Repo layout

| Path                                | Purpose                                                                                      |
| ----------------------------------- | -------------------------------------------------------------------------------------------- |
| [`tandem/`](tandem/README.md)       | Tandem desktop app (Tauri 2 + Rust + React)                                                  |
| [`beagle/`](beagle/README.md)             | The one `beagle` command: every terminal tool linked into a single binary                    |
| [`scout/`](scout/README.md)         | scout (`beagle scout`): the TUI/CLI crate (`tui/`), and `ios/` for the iPhone app            |
| [`recap/`](recap/README.md)         | recap (`beagle recap`): PR recaps (`tui/`)                                                   |
| [`standup/`](standup/README.md)     | standup (`beagle standup`): standup reports (`tui/`)                                         |
| [`rca/`](rca/README.md)             | Beagle RCA: the `beagle rca` CLI/TUI and daemon crates, desktop app and postmortem site      |
| [`workflows/`](workflows/README.md) | agentd (`beagle agent`: CLI, TUI and daemon crates) and the Agentdesk desktop app            |
| [`utils/`](utils/README.md)         | `beagle-utils`, Rust helpers shared by two or more apps                                      |
| [`skills/`](skills/README.md)       | Agent skills: `/beagle`, `/beagle-review`, `/beagle-create-issue`, `/beagle-complete-issue`  |
| [`standards/`](standards/)          | Shared lint and compiler config every app uses unchanged                                     |
| [`scripts/`](scripts/)              | CI gate, dev, release and GitHub scripts                                                     |
| [`packaging/`](packaging/)          | Homebrew formula templates and casks for `beagle-tools/tap`                                  |
| [`docs/`](docs/)                    | Monorepo docs: development, testing, standards, SDLC, releasing                              |

## Develop

```sh
make bootstrap            # install toolchains (Rust, Node via nvm, pnpm, gh, tmux)
make dev APP=tandem       # run any app — `scripts/dev/run.sh --list` for targets
make check                # exactly what CI runs (or make check-<app>)
```

| Doc                                                    | What's in it                                                   |
| ------------------------------------------------------ | -------------------------------------------------------------- |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)           | Machine setup, starting each app, browser mode with mocked IPC |
| [`docs/TESTING.md`](docs/TESTING.md)                   | What tests a change needs, and how CI selects them             |
| [`docs/CODING_STANDARDS.md`](docs/CODING_STANDARDS.md) | The one set of rules every app follows                         |
| [`docs/SDLC.md`](docs/SDLC.md)                         | Issue → PR → `gate` → merge → release                          |
| [`docs/RELEASING.md`](docs/RELEASING.md)               | Per-app versions, tags, in-app updates and Homebrew            |
| [`docs/serve-api.md`](docs/serve-api.md)               | `beagle serve`'s read-only HTTPS API for the phone app         |
| [`CONTRIBUTING.md`](CONTRIBUTING.md)                   | How to send a change                                           |

## Working with agents

Coding agents start at [`AGENTS.md`](AGENTS.md). Work flows issue → PR → `gate` → squash-merge →
release ([`docs/SDLC.md`](docs/SDLC.md)), and two skills drive it: `/beagle-create-issue` specs the
work and `/beagle-complete-issue` takes an issue to a merged release ([`skills/README.md`](skills/README.md)).

## Support the project

If Beagle saves you time, help keep it going:

- **[Sponsor](https://github.com/sponsors/matthewmyrick)**
- **[Star the repo](https://github.com/beagle-tools/beagle-tools)**
- **[Report a bug](https://github.com/beagle-tools/beagle-tools/issues/new/choose)**
- **[Contribute](CONTRIBUTING.md)**

## License

Apache-2.0 — see [LICENSE](LICENSE).
