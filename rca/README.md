# beagle

An **AI-first debugging TUI**. Claude debugs a system from its telemetry, then
writes what it found into an **RCA workspace** — plain files on disk. This TUI
renders those workspaces as a tabbed incident view so a human can immediately
see *what broke, why it broke, and how to fix it*.

Think `hunk` for git diffs, but for incidents: the AI does the investigation,
the TUI is how it explains itself.

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

## How it works

1. Claude (or you) investigates an incident and scaffolds a workspace:
   `beagle rca new 2026-06-30-sendgrid-webhooks --title "..." --severity high`
2. The investigation is written into that workspace as markdown sections and
   ASCII diagrams — see the data format below and
   [`../skills/beagle/SKILL.md`](../skills/beagle/SKILL.md)
   for the authoring guide Claude follows.
3. You keep `beagle rca` open in a terminal. It **watches the filesystem** and
   re-renders live as the investigation is written — no refresh needed.

Every RCA gets nine tabs: **Summary · Timeline · Root Cause · Impact · Fix ·
Final Review · Diagrams · Notes · Log**, plus an opt-in **Commands** tab
(after Fix) once the workspace has a `commands.md` — the TUI and the desktop
app both show it only then. The Log tab is the live investigation stream — the
agent appends a timestamped line at every step (`beagle rca log <slug> "..."`),
and `f` (follow mode) keeps the tab pinned to the newest line, tail-f style.
Tabs whose files changed since you last looked get a `●` marker, workspaces
in `investigating` status show a live spinner (red; `fixing` shows the same
spinner in orange), ticking elapsed time, and a
liveness read — `active 2m ago`, turning yellow (`quiet 12m`) when the agent
has gone silent — and
new workspaces announce themselves in the status bar as agents scaffold them.

## Repository layout

```text
cli/        the Rust TUI + CLI (everything documented below)
crates/     the agent runtime: beagle-agent (engine) + beagle-agentd (daemon)
desktop/    Tauri 2 + React desktop app (see desktop/README.md)
web/        Astro static site: public postmortems (see web/README.md)
deploy/     Helm chart for the web app (see web/DEPLOY.md)
docs/       the agent-runtime guide
```

The CLI, daemon, desktop app and web image ship together as one version,
released from `main` as `beagle-rca-vX.Y.Z` in the
[beagle-tools monorepo](https://github.com/beagle-tools/beagle-tools/releases)
(only when something under `rca/` changed). The rolling `beagle-rca-latest`
release always holds the newest installers under stable names.

## Install & run

rca ships inside the single `beagle` command (`../cli/`), as `beagle rca`:

```sh
brew install beagle-tools/tap/beagle                       # the beagle command (rca, scout, tandem, …)
brew install --cask beagle-tools/tap/beagle-rca-desktop    # the desktop app (macOS)
```

or a `beagle-cli-<target>.tar.gz` from the
[`beagle-cli-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-cli-latest)
release, or `cargo install --locked --path cli/crates/beagle` from a checkout
(see [`../cli/README.md`](../cli/README.md)). `beagle update` keeps it current.

The agent daemon ships inside `beagle` too: `beagle rca agentd run` (or
`once`), installed as a login service with `beagle rca agent install`. The
standalone `beagle-agentd` binary (in the `beagle-rca-<target>.tar.gz`
release tarball, beside the standalone CLI) runs the same daemon until it is
retired.

**The standalone rca `beagle` binary** (`brew install beagle-tools/tap/beagle-rca`,
v0.x) still ships while it is retired. It installs a `beagle` too, so the two
formulas conflict: `brew uninstall beagle-rca && brew install beagle-tools/tap/beagle`
switches. The standalone binary accepts the `beagle rca …` spelling below
as well as its old `beagle …` one.

Then:

```sh
beagle rca                    # open the TUI against ./rcas
beagle rca --root ~/oncall    # or point it anywhere
beagle rca list               # print workspaces to stdout
beagle rca list --status investigating --severity high       # filtered
beagle rca list --json        # the same listing as a JSON array, for scripts/jq
beagle rca new <slug> --title "..." [--severity high] [--system payments-api]...
beagle rca status <slug> investigating   # flip status; a running TUI updates live
beagle rca handoff <slug>     # launch the configured agent on a reviewed RCA
beagle rca log <slug> "checking redis pool"  # append to the live Log tab
beagle rca similar <slug>     # past RCAs sharing systems/tags, ranked (R in the TUI)
beagle rca archive <slug>     # move a finished RCA to rcas/archive/ (a shows them)
beagle rca list --archived    # include archived workspaces in the listing
beagle rca pr add <slug> https://github.com/org/repo/pull/123  # attach a fix PR
beagle rca pr list <slug>     # attached PRs, with live state when gh is available
beagle rca export <slug>      # one markdown file → exports/<slug>.md
beagle rca export <slug> --out ~/vault/incidents/<slug>.md   # e.g. an Obsidian vault
beagle rca banner             # print the BEAGLE banner
beagle rca init               # scaffold toolbox.md + systems/ agent context
beagle rca config             # edit + validate ~/.config/beagle/rca.toml
beagle rca skill status       # is the /beagle rca skill installed for Claude Code / Codex?
beagle rca skill install      # install (or refresh) the /beagle rca skill for both
beagle rca version            # print rca's version
beagle update                 # update beagle (and with it rca) to the latest release
```

## Give agents context: the toolbox

An investigating agent works much faster when it knows what telemetry exists
before it starts. `beagle rca init` scaffolds two things at the store root:

- `toolbox.md` — the tools available here: Grafana dashboards, Loki/Sentry,
  CLIs the agent may shell out to, runbooks, escalation paths.
- `systems/<name>.md` — one file per service (names match `systems` in
  `rca.toml`): its dashboards, log labels, dependencies, known failure modes.

The `/beagle` Claude Code skill reads these before every investigation and
updates them when it learns something durable. Press `T` in the TUI to see
the toolbox plus the systems docs for the selected incident — you and the
agent share the same source of truth.

## Config & updates

`beagle rca config` opens `~/.config/beagle/rca.toml` in your editor (config
`editor`, then `$VISUAL`/`$EDITOR`, then vim) and **validates it when the
editor closes** — typos and unknown keys are reported immediately. An older
`~/.config/beagle/config.toml` is still used while `rca.toml` doesn't exist;
move it with `mv ~/.config/beagle/config.toml ~/.config/beagle/rca.toml`. A
file that breaks later only warns: rca runs on defaults. Every setting is
optional and overridden by flags. The store root is `--root`, else
`$BEAGLE_RCA_ROOT`, else the nearest `.beagle`, else config `root`, else the
nearest folder holding `rcas/`, else the working directory:

```toml
root = "/path/to/oncall"   # default --root, so `beagle rca` works from anywhere
notify = true              # desktop pings: new incidents, status changes

[notify_events]            # optional: only fire these (omit = all events)
final_review = true        # e.g. only ping me when a fix merges to verify
investigating = true
editor = "code -w"         # editor for `beagle rca config`
timezone = "America/New_York"  # TUI timestamps (DST-aware; EST, PST, UTC,
                               # Europe/London, or a POSIX rule). Files stay UTC.
clock = "12h"              # 3:08 PM instead of 15:08 (default "24h")

[handoff]                   # `beagle rca handoff <slug>`: the agent to launch
command = ["codex", "exec"] # argv; the prompt + write-up go to its stdin
prompt = "~/.config/beagle/handoff-prompt.md"
```

Updating is `beagle update` (see [`../cli/README.md`](../cli/README.md)):
it replaces the whole `beagle` binary, so `beagle rca update` and
`beagle rca version list` only point you there. The standalone rca binary
still updates itself: its `beagle update` downloads
`beagle-rca-<target>.tar.gz` from the `beagle-rca-vX.Y.Z` release,
**verifies its sha256**, and atomically swaps the installed binary (and a
`beagle-agentd` installed beside it); `beagle version list` picks any
release, older or newer.

Every beagle binary bundles the `/beagle` skill (the authoring guide agents
follow — [`../skills/beagle/SKILL.md`](../skills/beagle/SKILL.md)).
After a standalone update, beagle checks whether the copy installed for **Claude Code**
(`~/.claude/skills/beagle/SKILL.md`) and **Codex**
(`~/.codex/prompts/beagle.md`) is stale and offers to refresh it — so the
skill tracks the binary. A skill that is a symlink (e.g. into a checkout
of this repo's `skills/`) is left alone. Run it anytime with `beagle rca skill install`, or check
with `beagle rca skill status`.

Keys: `j/k` navigate · `enter` open · `b` back to the list · `←/→` / `tab` /
`1`–`9` switch tabs · `/` search the incident (all tabs, `n`/`N` between
hits) · `\` find everywhere (fuzzy across every incident, tab, and line —
enter jumps straight there) · `f` filter the list (i/x/r/a/v/f status ·
c/h/m/l severity · `/` free text,
stacking + toggling) · `F` follow (tail -f) · `s` collapse/expand the
sidebar · `w` resize the sidebar (←/→, `0` default, `Esc` done) · `a`
show/hide archived · `T` toolbox ·
`o` open links/PRs · `R` related incidents · `V` sign off final-review · `S` settings ·
`u` attach a PR · `I` attach a ticket · `d` delete (list, asks y/n) · `m`
agents screen ·
`c` copy tab / `C` copy whole RCA (pbcopy or OSC 52) · `y` copy the line
(or selection) · `i` copy the incident id · `x` export to
`exports/<slug>.md` · `E` open this tab's file in your editor (config
`editor` → `$VISUAL` → `$EDITOR` → vim) · `n/p` cycle diagrams · `h/l`
pan diagrams · `ctrl-d`/`ctrl-u` half page · `r` reload · `?` help ·
`q` / `Q` / `ctrl-c` quit. The mouse works too: the wheel
scrolls whatever is under the cursor, click selects an incident,
switches tabs, or focuses the content pane — keys stay primary.

## Track the fix: attached PRs

Remediation lands as pull requests, and a merged PR isn't a verified fix.
The lifecycle follows the fix all the way:

```text
investigating ──▶ fixing ──▶ review ──▶ agent ──▶ final-review ──▶ finished
   (digging)    (root cause  (fix reviewed) (an agent  (PR merged —      (verified,
                 found;                     polls +    verify it!)       signed off)
                 writing fix)               remediates)
```

`agent` is an optional hand-off: an automated agent polls
`beagle rca list --status agent`, does the remediation work (opens PRs,
applies fixes) from its configured prompt, and its merged PRs
auto-advance the RCA to `final-review`. Skip it entirely and go
straight `review → final-review` if you don't run an agent.

`beagle rca pr add <slug> <url>` attaches a PR to the manifest; the workspace
header shows `fixes: ○ #123 open · ✓ #124 merged`, refreshed by a background
`gh` poll every 30 minutes (plus whenever the set changes). **When every
attached PR has merged, beagle automatically moves the RCA from `review` to
`final-review`** — time to work the Final Review tab, the checklist of
checkable predictions the agent wrote *during* the investigation ("p99 back
under 200ms for 24h"). Confirmed it held? Press **`V`** to sign off →
`finished`. Viewing never changes state; only `V` (or `beagle rca status <slug>
finished`) does.

No `gh` installed? PR links still show — just without live state or the
auto-transition. Press `o` to open any attached PR or URL in your browser.

## Automate the fix: the agent runtime

An always-on daemon (`beagle-agentd`) can turn an RCA you mark ready into a
remediation PR: move an RCA to `status = "agent"` and it implements
`remediation.md` in an isolated git worktree via a headless `claude` session,
opens a PR, attaches it, and advances the RCA — deterministic Rust owns
git/GitHub, `claude` only edits code. Configure it in `~/.config/beagle/agents.toml`,
autostart it with `beagle rca agent install` (launchd on macOS, `systemd --user` on
Linux), and watch it from the **Agents screen** (`m` in the TUI, or the
sidebar's **Agents** button in the desktop app).

Full guide — config, custom prompts, commands and the control socket — in
[docs/agent-runtime.md](docs/agent-runtime.md).

## Export

`e` (or `beagle rca export`) renders a workspace to a **single markdown file**:
YAML frontmatter (title, severity, status, dates, systems, tags) followed by
every section and the diagrams in code fences (ANSI colors stripped). It is
**deterministic** — same files in, same document out, no LLM involved — so
you can diff it, script it, and sync it anywhere. The frontmatter `tags` come
straight from `rca.toml`, so tools like Obsidian index the export natively.

## Data format (the API)

A workspace is a directory — no database, no lock-in, `git`-friendly:

```text
rcas/
  2026-06-30-sendgrid-webhook-signature-failures/   # id: lowercase slug [a-z0-9-]
    rca.toml          # title, severity, status, created, systems, tags
    summary.md        # what broke, in three sentences
    timeline.md       # what happened when
    root-cause.md     # why it broke, symptom → root
    impact.md         # who/what was affected, quantified
    remediation.md    # the Fix tab: mitigation + durable fixes
    final-review.md   # verification checklist, worked after the fix merges
    notes.md          # raw evidence, queries, links
    diagrams/
      01-topology.txt # ASCII diagrams, rendered unwrapped; ANSI colors supported
  archive/            # finished RCAs moved aside by `beagle rca archive` — hidden
    <slug>/           # by default in the TUI (`a` shows them, dimmed), still
                      # exportable and still mined by `similar`
```

`rca.toml`:

```toml
title = "Payments API p99 latency 40x regression"
severity = "high"          # critical | high | medium | low | info
status = "review"          # investigating | fixing | review | agent | final-review | finished
created = "2026-07-05T14:32:00Z"   # RFC 3339, quoted
systems = ["payments-api", "redis-sessions"]
tags = ["latency"]
prs = ["https://github.com/org/repo/pull/123"]   # optional; `beagle rca pr add`
```

Any section may be absent (the tab shows a hint instead); a corrupt manifest
skips that workspace with a status-bar warning and never crashes the TUI.

## Design goals

- **Airtight:** no `unsafe`, no panics on user data, pedantic clippy at
  `-D warnings`, terminal always restored — even on panic.
- **Memory-efficient:** manifests only at startup; section content loads
  lazily per tab and is evicted on switch; markdown renders once per change,
  not per frame.
- **I/O-efficient:** event-driven redraws via filesystem notifications
  (coalesced), blocking input loop (~0% idle CPU), bounded reads (4 MB cap),
  atomic scaffold writes.
- **Type-safe:** validated newtypes (`RcaId`), closed enums for
  severity/status/tabs, `deny_unknown_fields` manifests, one `thiserror`
  error type with path context.

## Development

This app lives in the beagle-tools monorepo. The quality gate (fmt, clippy
`-D warnings`, tests, docs for both Rust workspaces, plus the desktop and web
frontends) is one command from the repo root:

```sh
make check-rca        # or `make check` from rca/
```

CI runs the same scripts. Architecture and app rules are in
[`AGENTS.md`](AGENTS.md); the shared standards are in
[`../docs/CODING_STANDARDS.md`](../docs/CODING_STANDARDS.md), and the
contribution workflow is in [`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## License & maintainer

Apache-2.0 — see [`LICENSE`](../LICENSE). Maintained by
[Matthew Myrick](https://github.com/matthewmyrick); issues and PRs welcome
(read the contributing guide first).
