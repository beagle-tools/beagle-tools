# Tandem

**Local-first code review that works with your agents.**

Tandem is a desktop app (Tauri 2 + Rust + React) for reviewing GitHub pull
requests with AI agents riding along. It pulls everything about a PR from
GitHub — diff, checks, reviews, approvers, comments — renders a fast,
pretty differ, and lets a locally-run agent (Claude, Codex, or any custom
command) review the diff with you.

The twist: **agent comments never go to GitHub on their own.** They
land as local comments in Tandem, where you triage them (accept / reject
/ archive), discuss them in threads (@mention any agent), and — only
when you explicitly click post — push a chosen comment or an approval
back to GitHub. Two riders, one bike: the agents pedal with you, but
you steer.

## How it works

```mermaid
flowchart LR
    GH["GitHub"]
    subgraph core["Rust core · tokio"]
        direction TB
        client["tandem-github<br/>auth: gh CLI · PAT · GHES"]
        cache[("tandem-cache<br/>SQLite, cache-first")]
        runners["tandem-agents<br/>headless runners"]
    end
    UI["React UI<br/>differ · threads · agent feed"]
    proc["agent process<br/>claude -p · codex exec · custom"]

    GH -- "reads: PRs, diffs, checks, comments" --> client
    client --> cache
    cache -- "instant renders" --> UI
    UI -- "Tauri IPC + events" --> core
    runners -- spawns --> proc
    proc -- "JSONL events · tandem_comment" --> runners
    runners -- "local comments" --> cache
    UI -- "explicit click only: post / approve" --> GH
```

And the review loop itself:

```mermaid
sequenceDiagram
    actor You
    participant T as Tandem
    participant A as Agent (claude/codex)
    participant G as GitHub

    You->>T: open a PR
    T->>G: sync diff, checks, comments
    T-->>You: instant render from cache
    You->>T: run review (or @mention an agent)
    T->>A: prompt: PR context + diff + rules
    A-->>T: tandem_comment lines (stay local)
    You->>T: triage — accept / reject / archive / reply
    A-->>T: replies in-thread
    You->>G: post chosen comment / approve (explicit click)
```

- **Cache-first**: the UI renders instantly from SQLite, background syncs
  refresh from GitHub, and sync state is always visible.
- **Bring your own AI**: each agent runs a CLI you are already logged in
  to (claude, codex, grok, or a custom command); extra environment for it
  (a key, a custom endpoint variable) goes in the agent's env. Configure
  everything in-app.
- **Agents know they're reviewing**: each run gets a context block (PR,
  diff, rules) and a simple contract — emit
  `{"type":"tandem_comment", ...}` lines — inspired by
  [hunk](https://github.com/modem-dev/hunk).
- **Obvious logging**: every run writes a JSONL event log you can tail in
  the UI or grep on disk.

## Install

Install once; after that Tandem updates itself in-app.

**macOS — Homebrew**

```sh
brew install --cask beagle-tools/tap/beagle-tandem
```

**macOS / Linux — one-liner**

```sh
curl -fsSL https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.sh | sh
```

**Windows — one-liner**

```powershell
powershell -c "irm https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.ps1 | iex"
```

**Direct downloads** (stable links that always point at the latest
release, served from the rolling `beagle-tandem-latest` release):

| Platform            | Asset                                                                                                                                              |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS Apple silicon | [`Tandem-macos-arm64.dmg`](https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-macos-arm64.dmg)             |
| macOS Intel         | [`Tandem-macos-x64.dmg`](https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-macos-x64.dmg)                 |
| Windows x64         | [`Tandem-windows-x64-setup.exe`](https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-windows-x64-setup.exe) |
| Linux AppImage      | [`Tandem-linux-x86_64.AppImage`](https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-linux-x86_64.AppImage) |
| Linux deb           | [`Tandem-linux-amd64.deb`](https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/Tandem-linux-amd64.deb)             |

Every version is also its own release, tagged `beagle-tandem-vX.Y.Z`:
[browse them](https://github.com/beagle-tools/beagle-tools/releases?q=beagle-tandem-v).

**Updating.** Tandem checks for a new release on launch, every 30
minutes and when the window regains focus. When one exists, an
**update** button appears in the header: one click downloads the signed
bundle, installs it and restarts. **Settings › Version** lists every
published release and can switch to any of them, including rolling back
to an older one. Every bundle is signature-checked either way.

> macOS note: builds aren't notarized yet. The install script and the
> Homebrew cask clear the quarantine flag for you; for a manual dmg
> install, right-click → Open on first launch.

## Scripting it: `tandem-cli`

The app ships a read-only JSON CLI over its local store (the cask puts
it on PATH): list cached PRs, comments (flat or threaded), summaries and
agent runs, see which requested reviews nothing has looked at yet, and
queue a review for Tandem to run with your agent config. See
[`docs/CLI.md`](docs/CLI.md).

```sh
tandem-cli app-queue | jq -r '.[] | "\(.repo.owner)/\(.repo.name) \(.number)"'
tandem-cli app-review --repo acme/widgets --pr 123
```

## In the terminal: the review TUI

`tandem-cli` (or `beagle tandem`) with no arguments opens a terminal
review queue. Queue PRs (`beagle tandem add acme/widgets#123`, or `a`
in the TUI); a reviewer — for example the agentd job in
`agent/examples/demo/review-queue.yaml` — claims each one and has
an agent review it into local comments, a summary and a verdict; you
read the diff, summary and comments in the terminal, post the comments
you pick and approve, each only after a confirmation. It keeps its own
store and an optional `~/.config/beagle/tandem.toml` (`github_user`,
`kick`; the older `~/.config/tandem-tui/config.yaml` still works), and uses the `gh` CLI for GitHub. See
[`docs/CLI.md`](docs/CLI.md#the-review-tui).

## Building from source

Prereqs: Rust (the toolchain pinned in the repo's `rust-toolchain.toml`
installs itself via rustup), Node (see `.nvmrc`), pnpm via `corepack
enable`, and optionally the `gh` CLI (easiest auth).

```sh
make install
make dev
```

In the app: **settings → GitHub auth** (defaults to your `gh` CLI
session) → add a repo (browse your orgs or type `owner/name`) → pick a
PR → add an agent → **run review**.

## Project layout

| Path                   | What                                                      |
| ---------------------- | --------------------------------------------------------- |
| `lib/core`   | Domain types: PRs, diffs, local comments, agent specs     |
| `lib/github` | GitHub REST client — reads + explicit user-action writes  |
| `lib/cache`  | SQLite cache, local review store, 3-day merged-PR archive |
| `lib/agents` | Agent runners (headless claude/codex/custom), JSONL logs  |
| `desktop/src-tauri`            | Tauri shell: commands, events, settings                   |
| `desktop/src/`                 | React frontend (differ, panels, settings)                 |
| `docs/`                | Architecture, sandboxing (v2 Docker+squid), roadmap       |

## Optional file config (dotfiles-friendly)

Everything is configurable in the GUI, but if you keep dotfiles you can
declare tracked orgs, a default repo, default filters/sort, and agents in
`~/.config/tandem/tandem.yaml` (or point `$TANDEM_CONFIG` at a file).
Present fields override GUI settings at startup and declared agents are
upserted by name; no file means no change. See
[examples/tandem.yaml](examples/tandem.yaml).

## Standards

Tandem follows the monorepo's shared standards
([docs/CODING_STANDARDS.md](../docs/CODING_STANDARDS.md)) and CI gate. Run
it from the repo root with `make check-tandem` (or `make check` inside
`tandem/`). Contributor notes: [CONTRIBUTING.md](../CONTRIBUTING.md);
agent guide: [AGENTS.md](AGENTS.md).

## Security model (v1 → v2)

v1 runs agents as local processes — same trust level as running the CLI
yourself. GitHub writes happen only behind explicit user clicks; agents
have no write path. **v2 moves agent execution into Docker with a squid
proxy** so each agent can only reach the endpoints you allowlist. The
`network_allowlist` field already exists on every agent spec; see
[docs/SANDBOXING.md](docs/SANDBOXING.md) for the plan.
