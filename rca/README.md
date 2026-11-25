# Beagle RCA

**Your coding agent does the sniffing. Beagle shows you what it found.**

Beagle RCA (`beagle rca`) is a root-cause-analysis workspace for AI-assisted
debugging. An agent such as Claude Code or Codex digs through logs, metrics
and code, and writes what it learns into an **RCA workspace**: a folder of
markdown files and ASCII diagrams. You keep Beagle open beside it, in the
terminal or as a desktop app, and watch the write-up appear live, tab by tab:
*what broke, why it broke, and how to fix it*.

When the root cause is nailed down, Beagle can hand the RCA to a background
agent that writes the fix and opens the pull request. Beagle keeps sniffing
until that PR merges and the fix is verified.

```text
┌ Incidents (3) ──────────┐┌ Payments API p99 latency 40x regression ─────────┐
│ HIGH  Payments API p99… ││ ● review · HIGH · payments-api, redis-sess…      │
│  ◐ review               ││                                                  │
│ CRIT  Ledger export st… ││ 1 Summary·2 Timeline·3 Root Cause·4 Impact·5 Fix │
│  ● investigating        ││ ·6 Final Review·7 Diagrams·8 Notes·9 Log         │
│ LOW   Cron drift on ba… ││ ┌ Root Cause ───────────────────────────────────┐│
│  ✔ finished             ││ │ Causal chain, symptom → root                  ││
│                         ││ │  1. checkout requests time out (ELB 504)      ││
│                         ││ │  2. handlers block waiting for a free Redis…  ││
└─────────────────────────┘└──────────────────────────────────────────────────┘
  j/k select · enter open · tab/1-9 tabs · r reload · ? help · q quit
```

## Why you'd want it

- **See the investigation as it happens.** Beagle watches the files and
  redraws when they change. A Log tab streams each step the agent takes, and
  a liveness read tells you when it has gone quiet.
- **A write-up with a fixed shape.** Every RCA has the same tabs (Summary,
  Timeline, Root Cause, Impact, Fix, Final Review, Diagrams, Notes, Log), so
  you know where to look, and so does the next agent.
- **Plain files, no lock-in.** A workspace is a directory you can `git`
  commit, `grep`, or edit by hand. No server, no database.
- **From diagnosis to merged fix.** Attach fix PRs, let merges move the RCA
  along, and check the agent's own predictions before you call it done.
- **Memory.** `beagle rca similar` ranks past incidents on the same systems,
  so the second time something breaks the same way, the agent already knows.

## Install

Beagle RCA ships inside the single `beagle` command:

```sh
brew install beagle-tools/tap/beagle                       # the beagle command (macOS, Linux)
brew install --cask beagle-tools/tap/beagle-rca-desktop    # optional: the desktop app (macOS)
```

No Homebrew? Download `beagle-cli-<target>.tar.gz` from the
[`beagle-cli-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-cli-latest)
release, or build from a checkout with `cargo install --locked --path beagle`.
Linux desktop builds (AppImage, `.deb`) are on the
[`beagle-rca-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-rca-latest)
release. More options are in [`../beagle/README.md`](../beagle/README.md#install).

`beagle update` keeps the command current; the desktop app updates itself.
Coming from the old standalone `beagle-rca` formula? See
[the switch](docs/reference.md#the-standalone-binary-deprecated).

## Set up

**1. Teach your agent the trade.** Install the `/beagle` skill (how to
investigate and write an RCA) and `/beagle-review` (how to answer questions
about one) for Claude Code and Codex:

```sh
beagle setup                 # every tool's one-time steps; asks before each
# or just this one:
beagle rca skill install
beagle rca skill status      # check
```

**2. Pick a home for your RCAs.** In the repo or folder you debug from:

```sh
beagle rca init
```

It asks where the store should live, writes a `.beagle` file pinning it (so
`beagle rca` finds it from any subdirectory), and creates `toolbox.md` and
`systems/`. Fill those in: `toolbox.md` lists the dashboards, log tools and
CLIs the agent may use, and `systems/<name>.md` describes each service. The
agent reads them before every investigation, which makes it a lot faster.

Prefer one store for everything? Put `root = "/path/to/oncall"` in
`~/.config/beagle/rca.toml` (`beagle rca config` opens it). RCAs live under
`<root>/rcas/<id>/`. The full lookup order is in
[the reference](docs/reference.md#finding-the-store).

**3. Optional: start the fix agent.** See
[hand it to the daemon](#5-hand-it-to-the-daemon-for-a-fix-pr) below.

## Your first RCA

### 1. Open the viewer

```sh
beagle rca               # the TUI; or open the Beagle desktop app
```

Just exploring? From a checkout, `make run` in `rca/` opens the example
store.

### 2. Put your agent on the scent

In Claude Code (or Codex), from the same project:

```text
/beagle checkout requests started timing out around 14:30 UTC; find out why
```

The agent scaffolds a workspace, checks history, then writes as it goes:

```sh
beagle rca new 2026-07-05-checkout-timeouts --title "Checkout requests time out" \
  --severity high --system payments-api
beagle rca similar 2026-07-05-checkout-timeouts
beagle rca status 2026-07-05-checkout-timeouts investigating
beagle rca log 2026-07-05-checkout-timeouts "checking redis pool saturation"
```

### 3. Watch it live

The new incident appears in the sidebar with a spinner. Tabs with fresh
content get a `●`. Open the Log tab and press `F` to follow it like
`tail -f`. `T` shows the toolbox the agent is working from, `R` shows related
past incidents, and `?` lists every key.

Once the root cause is pinned down, the agent writes the Fix tab
(`remediation.md`) and a Final Review checklist of things that should be true
once the fix works, and moves the status along (`fixing`, then `review`).
`review` is your cue to read it.

To ask about it later, in a fresh session: `/beagle-review <id>` (`i` in the
TUI copies the id).

### 4. Ship the fix and verify it

```text
investigating ─▶ fixing ─▶ review ─▶ agent ─▶ final-review ─▶ finished
```

Fix it yourself, or with your agent, and attach the PR:

```sh
beagle rca pr add 2026-07-05-checkout-timeouts https://github.com/acme/widgets/pull/123
```

With `gh` installed and logged in, the header shows each PR's state. When
every attached PR has merged, Beagle moves the RCA to `final-review` on its
own. Work the Final Review tab, then press `V` to sign it off as `finished`.
`beagle rca archive <id>` moves it out of the sidebar; it still counts for
`similar` and search.

### 5. Hand it to the daemon for a fix PR

Rather not write the fix yourself? The agent daemon takes RCAs at status
`agent`, implements `remediation.md` in an isolated git worktree with a
headless `claude` session, pushes a branch, opens the PR, attaches it, and
moves the RCA to `final-review`. Git and GitHub are handled by the daemon;
`claude` only edits and commits code.

```sh
$EDITOR ~/.config/beagle/agents.toml    # which repo, which prompt (see the guide)
beagle rca agent install                # run it as a login service (launchd / systemd --user)
beagle rca agent status                 # is it up?
beagle rca status 2026-07-05-checkout-timeouts agent
```

Watch it with `m` in the TUI, or **Agents** in the desktop sidebar. To try it
once in the foreground: `beagle rca agentd once`. Config, prompts, logs and
the control socket are in [docs/agent-runtime.md](docs/agent-runtime.md).

For a one-off hand-off to any CLI agent instead, set `[handoff]` in the config
and run `beagle rca handoff <id>`
([config](docs/reference.md#config-rcatoml)).

## Everyday commands

```sh
beagle rca list --status investigating      # or --severity, --archived, --json
beagle rca similar <id>                     # past incidents on the same systems
beagle rca ticket add <id> ABC-123          # attach a GitHub or Linear issue
beagle rca export <id>                      # one markdown file → exports/<id>.md
beagle rca publish <id>                     # include it in the postmortem site
beagle rca config                           # edit and validate ~/.config/beagle/rca.toml
```

All of them, with flags: [docs/reference.md](docs/reference.md#commands).

## Troubleshooting

| Symptom | Check |
|---|---|
| The TUI is empty or shows another store | The store is found from your working directory. Pass `--root`, set `$BEAGLE_RCA_ROOT`, or run `beagle rca init` ([lookup order](docs/reference.md#finding-the-store)) |
| The desktop app shows no RCAs | It ignores `.beagle` files. Set `root` in `~/.config/beagle/rca.toml` |
| `/beagle` is not a command in my agent | `beagle rca skill status`, then `beagle rca skill install`, and restart the agent session |
| A workspace is missing from the list | Press `!` for load warnings; its `rca.toml` probably has an unknown key or a bad date |
| PRs show no state, no auto-advance | `gh auth status`. Without `gh`, PR links still work, without live state |
| `beagle rca config` reports an error | Top-level keys must come before the first `[table]` header |
| The daemon never picks up an RCA | `beagle rca agent status`; see [the daemon's troubleshooting](docs/agent-runtime.md#troubleshooting) |
| Anything else | `beagle doctor` checks skills, services and old binaries on your `PATH` |

Filing a bug? Include your OS and terminal, `beagle --version`, the workspace
that triggers it (or a trimmed copy), and for a panic the message with
`RUST_BACKTRACE=1`.

## Reference

- [docs/reference.md](docs/reference.md): every command, the data format and
  `rca.toml` manifest, the lifecycle, the config file, TUI keys, export
- [docs/agent-runtime.md](docs/agent-runtime.md): the agent daemon
- [`../skills/beagle/SKILL.md`](../skills/beagle/SKILL.md): exactly what the
  agent is told to write, and how
- [desktop/README.md](desktop/README.md) and [web/README.md](web/README.md):
  the desktop app and the public postmortem site

## For agents

- Debugging an incident or writing one up: follow
  [`../skills/beagle/SKILL.md`](../skills/beagle/SKILL.md). Answering
  questions about an RCA: [`../skills/beagle-review/SKILL.md`](../skills/beagle-review/SKILL.md)
  and `beagle rca context <id>`.
- Use the CLI rather than hand-editing `rca.toml`: `new`, `status`, `log`,
  `pr add`, `ticket add`. Read state with `beagle rca list --json`.
- Changing this code: read [`AGENTS.md`](AGENTS.md) first. The on-disk format
  is a public API; changing it means updating the skill and
  [the data format](docs/reference.md#data-format) in the same PR.

## What's in this folder

| Part | What it is | Run it (from `rca/`) |
|---|---|---|
| [`tui/`](tui/) | The `beagle rca` TUI, CLI and data layer (crate `beagle`) | `make run` (example store) |
| [`daemon/`](daemon/) | The agent daemon (`beagle rca agentd`) | `make agentd` |
| [`lib/agent/`](lib/agent/) | The daemon's engine: jobs, worktrees, the `claude` runner | — |
| [`desktop/`](desktop/README.md) | The Beagle desktop app (Tauri 2 + React) | `make desktop` |
| [`web/`](web/README.md) | Static site of published postmortems | `make web` |
| [`examples/rcas/`](examples/rcas/) | The example store | — |
| `deploy/`, `scripts/` | Helm chart for the site ([web/DEPLOY.md](web/DEPLOY.md)), release version stamping | — |

## Development

The quality gate (fmt, clippy, tests and docs for the Rust crates, plus both
frontends) is `make check-rca` from the repo root, or `make check` here. CI
runs the same scripts. Start with [`AGENTS.md`](AGENTS.md) for architecture,
then [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md) for running apps
locally and [`../CONTRIBUTING.md`](../CONTRIBUTING.md) for the workflow.

The CLI, daemon, desktop app and web image share one version and release as
`beagle-rca-vX.Y.Z` when something under `rca/` changes
([`../docs/RELEASING.md`](../docs/RELEASING.md)).

## License

Apache-2.0, see [`LICENSE`](../LICENSE). Maintained by
[Matthew Myrick](https://github.com/matthewmyrick); issues and PRs welcome.
