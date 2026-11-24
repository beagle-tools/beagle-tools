# beagle scout

A TUI for watching what your agents are working on — and why. Part of the
single `beagle` command (`beagle scout …`); this crate is `beagle-scout`.

Agent and human lose the thread on long, multi-step work. Scout gives the work
a shape: a **project** has one overarching goal, explained so someone new to
the stack can follow it, and an ordered list of small, precise **tasks** that
get there. The agent keeps that picture current through the `beagle scout` CLI (the
`/scout` skill teaches it how); you keep `beagle scout` open in a terminal and see,
live, which task it is on, what it just did, and how far along the goal is.

```text
╭ ‹ One noisy client can't take the API… ╮  1 Task  ·  2 Project  ·  3 Log
│✔ 01 Map the request path from the int…  │╭ 02-add-rate-limit · Add a per-key rate limit to the gateway…╮
│     done · 3/3 ☑                        ││⠧ working · task 2 of 3 · checklist 1/4 · active 2m ago        │
│⠧ 02 Add a per-key rate limit to the …   ││                                                               │
│     working · 1/4 ☑ · active 2m ago     ││Context                                                        │
│○ 03 Roll the gateway config out to pr…  ││All traffic enters through the API gateway, which forwards…    │
│     todo · 0/3 ☑                        ││                                                               │
╰─────────────────────────────────────────╯╰───────────────────────────────────────────────────────────────╯
j/k select · enter read · tab/1-4 tabs · b back · F follow · E edit · ? help · q quit
```

## Install

Scout ships inside the `beagle` binary. From a checkout of this repo:

```sh
cargo install --path beagle   # puts `beagle` on your PATH
beagle scout skill install               # the /scout and /scout-build skills for Claude Code (+ Codex)
```

## The two skills

- **`/scout <goal>`** plans: researches, writes the project (goal, problem,
  plain-language stack tour, done-when) and its ordered tasks, and shows you
  the plan. ([skill](skills/scout/SKILL.md))
- **`/scout-build [<project>] [<task>]`** builds: works the project task by
  task. It starts each task, narrates, ticks steps, verifies done-when,
  writes the outcome, and marks it done, so the TUI always shows exactly
  where it is. With no project it asks which one. It stops and asks before
  anything outward-facing (push, PR, deploy) and when blocked, and never
  leaves a task spinning when it stops.
  ([skill](skills/scout-build/SKILL.md))

Both skills are compiled into the binary, so the instructions always match
the CLI they drive; `beagle scout skill status` says whether the installed
copies are current.

A typical loop: `/scout rate-limit the public API` → review the plan in
`beagle scout` → `/scout-build rate-limit-the-api` in a fresh session → watch
it in `beagle scout` (and its commands in `beagle scout watch`).

## Use

```sh
beagle scout                  # the TUI (which store: see "Where projects live")
beagle scout list             # every project, one per line (--json for scripts)
beagle scout show <project>   # the goal and the ordered tasks — "where were we?"
```

Agents drive the rest through the skills above; `beagle scout help` lists every
command:

```sh
beagle scout new rate-limit-the-api --title "One noisy client can't take the API down" --link ABC-344
beagle scout task add rate-limit-the-api add-rate-limit --title "Add a per-key rate limit in gateway/limits.yaml"
beagle scout task start rate-limit-the-api 2
beagle scout log rate-limit-the-api --task 2 "found 3 existing routes in gateway/limits.yaml"
beagle scout task done rate-limit-the-api 2
```

### Keys

Beagle's bindings: `j`/`k` move · `g`/`G` first/last · `enter`/`l` open a
project, then read a task · `b`/`esc` back · `tab`/`]`, `shift-tab`/`[`,
`←`/`→` or `1`–`5` switch the Task · Outcome · Session · Project · Log tabs · `space`/`pgdn`,
`pgup`, `ctrl-d`/`ctrl-u` page · `F` follow (tail the content) · `w` resize the left list (←/→, `0` default,
`Esc` done) · `E` open the
shown file in `$VISUAL`/`$EDITOR` · `r` reload · `?` help · `q`/`Q`/`ctrl-c` quit.

The view reloads by itself whenever a file under the store changes. A task
being worked shows Beagle's spinner and how long since its files last changed
(`active 2m ago`, turning yellow `quiet 12m` after ten silent minutes).

## Watch an agent's commands

The TUI shows *which task* an agent is on. `beagle scout watch` shows *what it is
doing right now*: every terminal command a Claude Code session runs, and what
came back, streamed into a pane next to it like a terminal someone is typing
into.

```text
 ┌ Herdr tab ────────────────────────────────────────────────────────────────┐
 │ claude                              │ beagle scout watch                  │
 │                                     │ ▶ task 02-switch-algorithm · Set …  │
 │ > fix the flaky uploader test       │                                     │
 │                                     │ # Find the retry logic              │
 │ ● Reading src/uploader.rs …         │ $ rg -n "retry" src/uploader.rs     │
 │                                     │ 42:    let retries = 3;             │
 │                                     │ ✔ 120ms                             │
 │                                     │ ✎ retry count is hard-coded to 3    │
 │                                     │                                     │
 │                                     │ $ cargo test uploader               │
 │                                     │ test retry_gives_up ... FAILED      │
 │                                     │ ✖ exit 101 · 4.2s                   │
 └─────────────────────────────────────┴─────────────────────────────────────┘
```

Set it up once:

```sh
cargo install --path beagle   # the hooks call this binary by its absolute path
beagle scout hook install                # adds scout's hooks to ~/.claude/settings.json (backup kept)
```

Then, next to any Claude Code session started after that, open a pane and run:

```sh
beagle scout watch                  # pick a session, like `claude --resume` (see below)
beagle scout watch --auto           # skip the picker: follow this tab's newest Claude session
beagle scout watch 4f85a782         # pin a session by id, or just its first characters
beagle scout watch --list           # recorded sessions, newest first
beagle scout watch --commands-only  # just commands and ✔/✖, no output
beagle scout watch --full           # never cut long output (default: first 12 + last 28 lines)
beagle scout watch --history 0      # skip the replay, show only what happens next
```

`beagle scout watch` with no arguments opens a fuzzy picker (fzf, styled like the
`ffg` finder). Each session shows its Claude Code title (the one
`claude --resume` shows), your last prompt, its directory, and `← this tab`
for sessions in your Herdr tab. The preview pane shows its latest commands.
Type to filter, then press Enter:

- **auto** (the first row) follows the newest session in this tab (or
  directory) and switches to the new one after `/clear`.
- **Any session** pins it: the watcher stays on that session no matter
  what else runs.

Without fzf you get a numbered list instead.

How it works: Claude Code's `PreToolUse`, `PostToolUse` and
`PostToolUseFailure` hooks run `<path>/beagle scout hook` around every Bash command it
runs (subagents' too, tagged like `[Explore]`). `beagle scout hook` appends one JSON
line per event to `~/.scout/.sessions/<session-id>.jsonl` and always exits 0,
so it can never slow down or break Claude. `beagle scout watch` replays the last 20
commands from that file, then follows it as it grows. With **auto** (or `--auto`),
it picks the newest session recorded from its own Herdr tab (`HERDR_TAB_ID`),
else from its own directory, else the newest, and switches when a newer one
appears (e.g. after `/clear`). The agent's `beagle scout task start/done` calls show
as `▶`/`✔` banners and its `beagle scout log` lines as `✎` narration, so you see
both the task and the commands it runs for it.

Recordings hold command output, so anything a command prints (including
secrets) is stored in `~/.scout/.sessions/`. They are pruned after 14 days
of inactivity. Remove the hooks with `beagle scout hook uninstall`; check them with
`beagle scout hook status`. Hooks written by the older standalone `scout`
binary (`…/scout hook`) are recognised: `beagle scout hook install` rewrites
them in place and `uninstall` removes them.

## Session logs: what the agent did, on the task

Each task has a **Session** tab (`3`) showing the part of the Claude Code
session that worked it. It opens with a summary (what it made, then how
much it did), followed by a readable timeline:

```text
━━ session 1163b504 · 16:48 UTC → 17:06 UTC ━━
2 prompt(s) · 32 command(s) (1 failed) · 5 file(s) changed
  ✎ gateway/limits.yaml                                         +5 −1     what it made
  ✚ gateway/tests/limits_test.go                                +151 −5
── timeline ──
› You · 16:48 UTC                                 cyan     your prompts
  okay lets do task 4 please and start it
∴ thought for 6.7s                                magenta  thinking (the text, when Claude kept it)
● Task 3 found nothing that needs changing…       white    Claude's replies (Markdown)
▸ Check how routes declare their limits          green    what a command is for…
  $ cd ~/src/… && grep -n 'rate_limit' gateway/…  dim      …and the command, on one line
  │ 12:  rate_limit: { per_key: 600 }            gray     output, at most 6 lines
  ✖ exit 2                                        red      failures
✎ edited  limits.yaml  +5 −1                      yellow   file changes, with line counts
▸ looked at limits.yaml, routes.yaml              blue     reads and searches, grouped
▶ task 03-roll-out · PR: ship …                   orange   scout task changes
✎ limits load: config check OK                    cyan     scout log narration
```

The agent does one thing: attach its session. `beagle scout task start` does it
automatically (via `$CLAUDE_CODE_SESSION_ID`), or `beagle scout task attach
<project> <task>` explicitly. Everything after that is an ETL pipeline, and
the TUI only displays its result:

```text
~/.claude/projects/…/<session>.jsonl   Claude's transcript
   │ extract   new lines since last run (remembered in <session>.cursor)
   │ transform → <project>/sessions/<session>.jsonl         scout's own entries
   │ transform → per task: its slice, summary + timeline
   ▼ load      → <project>/sessions/<session>.<task>.log    colored text (`less -R` works)
                                         ▲
                     the TUI's Session tab just reads this
```

- **When it runs:** after every tool call and at the end of every Claude
  turn, from the installed hooks (`beagle scout hook install`; they run in the
  background and never slow Claude down). As a fallback, a background
  thread in the TUI runs it about once a second for tasks being worked.
  Nothing parses or renders on the TUI's drawing thread.
- **Which part of the session belongs to a task:** from the prompt before
  the agent first started or attached it, to its last mention of the task
  (a status change, a `beagle scout log --task`). A task still being worked runs to
  the end.
- **Stays local:** the copies live in the store's `<project>/sessions/`, which `beagle scout init` keeps out of git. They
  survive Claude Code's own transcript cleanup and don't depend on Claude's
  internal format staying the same.
- `beagle scout session show <project> <task> [--follow]` prints the same view in
  any terminal pane.

Thinking: Claude Code usually keeps only a signature and a duration for
thinking blocks, so most show as `∴ thought for 2.1s`; the text is shown
whenever the transcript has it.

## Notifications (macOS)

The `beagle scout` CLI notifies you when an agent changes a status, even when the
TUI isn't open:

```text
┌────────────────────────────────────────────────────┐
│ ╭────╮  Scout · rate-limit-the-api           🔔   │
│ │ 🐶 │  ■ Needs you: 03-open-pr · Open the PR      │
│ ╰────╯  Push branch fix-retry and open a PR?       │
└────────────────────────────────────────────────────┘
```

By default: **blocked** (the agent needs you; plays a sound, and the text is
the agent's `--reason`), **project started**, and **project done**. Clicking
a notification focuses the agent's Herdr pane (`herdr agent focus`) and
brings your terminal forward. A project's banners replace each other instead
of piling up.

The icon is scout's dog (`assets/scout-dog.png`, see `assets/NOTICE`). macOS
always shows the icon of the app that posts a notification, so
`beagle scout notify setup` (also run by `beagle scout skill install`) builds a small
`Scout.app` in `~/Library/Application Support/scout/` from Homebrew's
`terminal-notifier` (`brew install terminal-notifier`), with its own bundle
id and the dog icon. Without it you get `terminal-notifier`'s Terminal icon,
or plain `osascript` banners if that isn't installed either.

Tune it in `~/.config/beagle/scout.toml` (the older
`~/.config/scout/config.toml` still works when the new file doesn't exist;
a broken file warns and the defaults apply). Every key is optional, and these
are the defaults:

```toml
notify = true

[notify_events]
blocked = true
project_started = true
project_done = true
task_done = false
task_started = false
task_paused = true   # a working task went quiet and was auto-paused

sound = "Glass"   # for `blocked`; "" for silent

[auto]
pause_after = "30m"  # quiet this long → a working task becomes `paused`
                     # ("45s", "30m", "2h", or "off")
```

**Auto-pause.** A `working` task nobody has touched for `pause_after`
becomes `paused` (`‖`, yellow) with a log line (`auto-paused: quiet 34m`)
and a notification. That usually means the agent stalled, died, or moved on
without saying so. "Touched" is the newest change to the task's file, the
project's log, or the transcript of a Claude session attached to the task,
so an agent that is thinking or reading still counts as active. The agent
narrating the task again (`beagle scout log --task`) or starting it again sets it
back to `working`. The check runs in the TUI (every 30 seconds) and on
`beagle scout list` / `beagle scout show`. One caveat: a single command running longer
than `pause_after` with no output pauses the task until the agent next
touches it.

`beagle scout notify test` sends a sample and prints the settings in effect.
`SCOUT_NOTIFY=0` turns notifications off for one command or shell.

## Roadmap

- **Command policy (V2):** the `PreToolUse` hook can refuse a command, so
  beagle scout could block or require approval for specific commands.
- **File changes with intent (V2):** show which folders, files and
  functions an agent creates, and *why*: what each is for and which task
  it serves.
- **Commands in the TUI:** a Commands tab next to the other task tabs.

## On disk

### Where projects live

A store is a folder of projects. `beagle scout root` prints the one in use and why. First match wins:

1. `--root <dir>`
2. `$BEAGLE_SCOUT_ROOT` (then the older `$SCOUT_ROOT`)
3. the nearest folder at or above where you are that holds a `.scout-store` file
4. `root` in `~/.config/beagle/scout.toml` (or the older `~/.config/scout/config.toml`)
5. `~/.scout`

To keep projects in their own git repo:

```sh
cd ~/src/my-scout-projects && beagle scout init   # writes .scout-store and a .gitignore
```

Inside that folder, `beagle scout` uses it. Agents work from other repos, so to make it your default everywhere, also set `root = "~/src/my-scout-projects"` in `~/.config/beagle/scout.toml`.

`init` adds `/.sessions/` and `/*/sessions/` to the store's `.gitignore`, and so does the first recording into a store inside a git repo (one picked by `$BEAGLE_SCOUT_ROOT` or config `root` never ran `init`). Session recordings hold raw command output, which can include secrets, so they never belong in a repo. `beagle scout hook install` backs up `settings.json` once, to `settings.json.scout-backup`, and never overwrites that first backup.

### File format

Plain Markdown with TOML frontmatter (between `+++` fences) — no database,
`git`-friendly, readable without scout:

```text
<store>/                            # see "Where projects live"
  rate-limit-the-api/               # project id: lowercase [a-z0-9-], ≤ 64 chars
    project.md                      # Goal · The problem · How this part of the stack works · The plan · Done when
    log.md                          # timestamped activity; every CLI change is logged too
    tasks/
      01-map-request-path.md        # NN orders the tasks
      02-add-rate-limit.md          # Context · Objective · Why this moves the goal · Steps · Done when · Outcome
  .sessions/                        # recorded agent sessions for `beagle scout watch` (hidden from the project list)
    <session-id>.jsonl
```

```toml
+++
title = "Add a per-key rate limit to the gateway config in gateway/limits.yaml"
status = "working"                  # todo | working | paused | blocked | done | skipped
created = "2026-11-12T15:30:05Z"    # RFC 3339, quoted
updated = "2026-11-12T15:41:10Z"
links = ["ABC-344", "backlog:TASK-12", "https://github.com/acme/widgets/pull/12"]
+++
```

Projects use the same fields with `status = planning | active | paused | done | dropped`.
Unknown fields are rejected, so typos surface as a warning instead of
vanishing. A file that fails to load is skipped (the status bar counts them;
`beagle scout list` prints why) and never crashes the TUI.

## Development

See [AGENTS.md](AGENTS.md) for the module layout and the rules for working
on scout. From the repo root, `make check-beagle` runs exactly what CI runs:
`cargo fmt --check`, pedantic `cargo clippy --all-targets -D warnings`,
`cargo test`, and `cargo doc` with warnings denied.
