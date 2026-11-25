# Beagle RCA reference

The details behind [`../README.md`](../README.md): every command, the
on-disk format, the config file, and the TUI keys. The agent daemon has its
own guide, [`agent-runtime.md`](agent-runtime.md).

- [Commands](#commands)
- [Data format](#data-format)
- [Lifecycle](#lifecycle)
- [Config: `rca.toml`](#config-rcatoml)
- [Finding the store](#finding-the-store)
- [Agent skills](#agent-skills)
- [TUI keys](#tui-keys)
- [Export](#export)
- [The standalone binary (deprecated)](#the-standalone-binary-deprecated)
- [Design goals](#design-goals)

## Commands

`beagle rca --help` prints the same list. Every command that touches a store
takes `--root <dir>` (or `--root=<dir>`), before or after the command.

| Command | What it does |
|---|---|
| `beagle rca` | Open the TUI |
| `beagle rca new <id> --title <t> [--severity <sev>] [--system <name>]...` | Scaffold a workspace. Severity defaults to `medium`; `--system` repeats |
| `beagle rca list [--status <s>] [--severity <sev>] [--archived] [--json]` | Print workspaces; `--json` gives a JSON array for scripts |
| `beagle rca status <id> <status>` | Set the lifecycle status; a running TUI updates live |
| `beagle rca log <id> <message...>` | Append a timestamped bullet to `log.md` (the Log tab) |
| `beagle rca similar <id>` | Past RCAs that share systems or tags, ranked (archived ones included) |
| `beagle rca context <id>` | Print the write-up plus the toolbox, for `/beagle-review` |
| `beagle rca pr add <id> <url>` / `pr list <id>` | Attach a fix PR / list them, with live state when `gh` works |
| `beagle rca ticket add <id> <ref>` / `ticket list <id>` | Attach a GitHub or Linear issue (URL or an id like `ABC-123`) |
| `beagle rca ticket sync [<id>]` | Poll attached tickets and attach the PRs linked to them (all RCAs when `<id>` is omitted) |
| `beagle rca handoff <id>` | Pipe the write-up to the agent in `[handoff]` (see [config](#config-rcatoml)) |
| `beagle rca archive <id> [--force]` / `unarchive <id>` | Move a `finished` RCA to `rcas/archive/` and back. `--force` skips the status check |
| `beagle rca publish <id>` / `unpublish <id>` | Mark an RCA public for the [postmortem site](../web/README.md) |
| `beagle rca export <id> [--out <file>]` | One markdown file, default `exports/<id>.md` |
| `beagle rca init` | Set up a store: asks where the root lives, writes a `.beagle` pinning it, scaffolds `toolbox.md` + `systems/` |
| `beagle rca config` | Edit the config file in your editor and validate it on close |
| `beagle rca skill [status\|install]` | Check or install the `/beagle` and `/beagle-review` skills |
| `beagle rca agent install\|uninstall\|start\|stop\|status` | Manage the agent daemon as a login service |
| `beagle rca agentd [run\|once]` | Run the daemon in the foreground (what the service runs) |
| `beagle rca version` / `--version` | Print rca's version |
| `beagle rca banner` | Print the BEAGLE banner |

`<id>` is a lowercase slug, `[a-z0-9-]`, at most 64 characters. It becomes
the directory name under `<root>/rcas/`. Updating is `beagle update`;
`beagle rca update` only points you there, because every tool ships in the
one binary.

## Data format

A workspace is a directory of plain files. There is no database, so it works
with `git`, `grep` and any editor. This format is the public API: the TUI,
the desktop app, the web site, the daemon and the skills all read it.

```text
<root>/
  toolbox.md            # tools the investigating agent may use (beagle rca init)
  systems/<name>.md     # one per service; names match `systems` in rca.toml
  rcas/
    2026-06-30-webhook-signature-failures/
      rca.toml          # the manifest
      summary.md        # what broke, in a few sentences
      timeline.md       # what happened when
      root-cause.md     # why it broke, symptom → root
      impact.md         # who and what was affected, quantified
      remediation.md    # the Fix tab: mitigation and durable fixes
      commands.md       # optional: adds a Commands tab after Fix
      final-review.md   # checkable predictions, worked after the fix merges
      notes.md          # raw evidence, queries, links
      log.md            # the live investigation log (beagle rca log)
      diagrams/
        01-topology.txt # ASCII, shown unwrapped; ANSI colors allowed
    archive/<slug>/     # archived workspaces: hidden by default, still searched
  exports/<slug>.md     # beagle rca export output
```

Tabs, in order: Summary, Timeline, Root Cause, Impact, Fix, (Commands),
Final Review, Diagrams, Notes, Log. Any file may be missing; its tab shows a
hint instead. A corrupt manifest skips that workspace with a warning (`!` in
the TUI lists them) and never crashes the viewer.

`rca.toml` rejects unknown keys:

```toml
title = "Payments API p99 latency 40x regression"
severity = "high"            # critical | high | medium | low | info
status = "review"            # see Lifecycle below
created = "2026-07-05T14:32:00Z"   # RFC 3339, quoted
updated = "2026-07-05T16:10:00Z"   # optional
systems = ["payments-api", "redis-sessions"]
tags = ["latency"]           # `skip-final-review` and `security` are special
prs = ["https://github.com/acme/widgets/pull/123"]   # beagle rca pr add
tickets = ["ABC-123"]        # beagle rca ticket add
published = false            # beagle rca publish; sets published_at too
```

Special tags:

- `skip-final-review`: when every attached PR merges, go straight to
  `finished` instead of `final-review`.
- `security`: the `s` filter facet in the TUI narrows to these.

## Lifecycle

```text
investigating ──▶ fixing ──▶ review ──▶ agent ──▶ final-review ──▶ finished
   digging        root cause   write-up   the daemon  PR open or    verified,
                  found,       ready for  writes the  merged:       signed off
                  fix drafted  a human    fix PR      verify it
```

- `agent` is optional. It hands the RCA to the
  [agent daemon](agent-runtime.md), which opens a fix PR, attaches it and
  moves the RCA to `final-review` itself. Without a daemon, go
  `review → final-review` (or let merged PRs do it).
- **Merged PRs advance the RCA.** While a TUI is open, a background `gh`
  poll (every 5 minutes, and whenever the set of PRs changes) watches every
  attached PR. When all of them have merged, an RCA at `review` or `agent`
  moves to `final-review` (or `finished` with `skip-final-review`). An RCA
  with no attached PRs never auto-advances.
- `final-review` is the checklist the agent wrote during the investigation
  ("p99 back under 200ms for 24h"). Press `V` in the TUI, or run
  `beagle rca status <id> finished`, once it holds. Viewing never changes
  state.
- Without `gh`, PR links still show and `o` opens them; there is just no
  live state and no auto-advance.

## Config: `rca.toml`

`beagle rca config` opens `~/.config/beagle/rca.toml` in your editor (config
`editor`, then `$VISUAL`, then `$EDITOR`, then vim) and validates it when the
editor closes. `S` in the TUI shows and edits the same file. An older
`~/.config/beagle/config.toml` is still read while `rca.toml` is absent
(`mv` it to switch). `$BEAGLE_CONFIG` names a different file. A config that
breaks later only warns; rca runs on defaults. Every key is optional.

```toml
root = "/path/to/oncall"     # the store (the dir holding rcas/), so beagle rca works from anywhere
editor = "code -w"           # for `beagle rca config` and `E`
timezone = "America/New_York"  # TUI timestamps: IANA name, EST/PST/UTC, or a POSIX rule.
                             # Files on disk stay UTC.
clock = "12h"                # "12h" or "24h" (default)
notify = true                # desktop notifications from the TUI (off by default)

[notify_events]              # optional: fire only these (absent = every event)
new_incident = true
final_review = true          # also: investigating, fixing, review, agent, finished

[handoff]                    # beagle rca handoff <id>
command = ["codex", "exec"]  # argv; the prompt + write-up go to its stdin
prompt = "~/.config/beagle/handoff-prompt.md"   # optional

[tickets.github]             # how `ticket sync` finds linked PRs
mode = "cli"                 # "cli" (default, uses gh) or "api"
# token = "..."              # only for mode = "api"

[tickets.linear]
mode = "cli"
command = ["linear", "issue", "view", "{id}", "--json"]   # output is scanned for PR URLs
```

Keep top-level keys above the first `[table]`; TOML puts anything after a
table header inside that table. `handoff` runs the command in the store root
with `BEAGLE_RCA_SLUG` and `BEAGLE_RCA_DIR` set, and logs the hand-off to the
workspace's Log tab.

## Finding the store

The store root (the directory that holds `rcas/`) is the first of:

1. `--root <dir>`
2. `$BEAGLE_RCA_ROOT`
3. the nearest `.beagle` file at or above the working directory (same format
   as `rca.toml`; an empty one pins its own directory, like `.git`)
4. `root` in `~/.config/beagle/rca.toml`
5. the nearest directory at or above the working directory that holds `rcas/`
6. the working directory

`beagle rca new` creates `rcas/` if it is missing. The desktop app skips
step 3 (a GUI app has no useful working directory), so give it a `root` in
the config. The agent daemon adds `$BEAGLE_AGENT_RCA_ROOT` in front of the
list ([agent-runtime.md](agent-runtime.md#how-it-works-contracts)).

## Agent skills

The `beagle` binary embeds two skills from this repo's
[`skills/`](../../skills/):

- `/beagle` ([`SKILL.md`](../../skills/beagle/SKILL.md)): how an agent
  investigates and writes an RCA workspace.
- `/beagle-review` ([`SKILL.md`](../../skills/beagle-review/SKILL.md)): load
  an RCA by id and answer questions about it.

`beagle rca skill install` (or `beagle setup`) writes them for Claude Code
(`~/.claude/skills/<name>/SKILL.md`) and Codex (`~/.codex/prompts/<name>.md`).
`beagle rca skill status` says whether each copy is current. A copy that is a
symlink, for example into a checkout of this repo, is left alone. Re-run
`install` after `beagle update` (`beagle doctor` warns when they are stale).

## TUI keys

`?` shows them all, filterable with `f`. The mouse works too: the wheel
scrolls what is under the cursor, a click selects or switches tabs.

| Keys | Action |
|---|---|
| `j`/`k`, `↓`/`↑` | Select an incident; move the line cursor in content |
| `enter`/`l`, `b`/`esc` | Focus the content pane / back to the list |
| `tab`, `[`/`]`, `←`/`→`, `1`–`9` | Switch tabs |
| `f` | Filter: `i x r a v f` status, `c h m l` severity, `p` has a PR, `s` security, `/` text (they stack) |
| `F` | Follow: stick to the bottom on reload, like `tail -f` |
| `/`, `n`/`N` | Search this incident, every tab |
| `\` | Find everywhere: fuzzy across every incident and line; `enter` jumps |
| `s`, `w` | Hide the sidebar / resize it |
| `a` | Show archived incidents (dimmed) |
| `T`, `R` | Toolbox and systems docs / related incidents |
| `t`, `V` | Set status / sign off `final-review` as `finished` |
| `u`, `I`, `#` | Attach a PR / attach a ticket / edit tags |
| `o` | Open attached PRs and links |
| `c`/`C`, `y`, `v`, `i` | Copy tab / whole RCA, yank line, visual select, copy the id |
| `x`, `E` | Export to `exports/<id>.md` / open this tab's file in your editor |
| `n`/`p`, `h`/`l` | Next/previous diagram, pan wide diagrams |
| `S`, `m`, `!` | Settings / the agents screen / load warnings |
| `d` | Delete the selected incident (asks first) |
| `r`, `q` | Reload, quit |

Live cues: a `●` on tabs that changed since you looked; a spinner on
`investigating` (red) and `fixing` (orange) workspaces with elapsed time and
a liveness read (`active 2m ago`, yellow `quiet 12m` when the agent goes
silent); new workspaces announce themselves in the status bar.

## Export

`x` in the TUI, or `beagle rca export <id> [--out <file>]`, writes one
markdown file: YAML frontmatter (title, severity, status, dates, systems,
tags), then every section, then the diagrams in code fences with ANSI colors
stripped. It is deterministic (same files in, same document out, no LLM), so
you can diff it, script it, or drop it into a notes vault, which will index
the frontmatter tags.

## The standalone binary (deprecated)

Before the unified `beagle` command, rca shipped as its own `beagle` binary
(`brew install beagle-tools/tap/beagle-rca`, plus `beagle-agentd`). It still
works and accepts the `beagle rca …` spelling, but brew warns on it and the
formula is going away. The two formulas both install `beagle`, so switch with:

```sh
brew uninstall beagle-rca && brew install beagle-tools/tap/beagle
beagle rca agent install     # repoint the daemon service at the new binary
```

## Design goals

- **Never break the terminal:** no `unsafe`, no panics on user data, the
  terminal is restored on every exit path, panics included.
- **Light:** manifests load at startup; section content loads per tab and is
  dropped on switch; markdown renders once per change, not per frame.
- **Idle when idle:** redraws are driven by filesystem events (coalesced) and
  key presses, about 0% CPU while waiting; reads are capped at 4 MB.
- **Strict types:** validated ids, closed enums for severity, status and tabs,
  manifests that reject unknown keys.
