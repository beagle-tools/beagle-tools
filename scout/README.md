# scout

`beagle scout` keeps a nose on your agents' trail. Long, multi-step agent
work loses the thread: the agent forgets where it was, and you can't tell
what it's doing or why. Scout gives the work a shape, a **project** with
one plainly explained goal and an ordered list of small **tasks**, and
shows it live in a terminal while the agent works.

- **Plan** with `/scout <goal>`: the agent researches and writes the project
  and its tasks, then shows you the plan.
- **Build** with `/scout-build <project>`: the agent works task by task,
  starting, narrating and finishing each one, and stops to ask before
  anything outward-facing (push, PR, deploy).
- **Watch** in `beagle scout`: which task it's on, what it just did, how far
  along the goal is. `beagle scout watch` streams the terminal commands it
  runs, and each task's Session tab shows the slice of the Claude Code
  session that worked it.
- **Get pinged** on macOS when a task is blocked and needs you, and follow
  along on your phone through [`beagle serve`](../beagle/serve/README.md)
  (phone push notifications are on the way).

Everything is plain Markdown on disk, in a folder you can keep in git.

## Quick start

Scout ships inside the one `beagle` binary; it has no release of its own.

```sh
brew install beagle-tools/tap/beagle     # or from source: ../beagle/README.md
beagle setup                             # skills, session hooks and the ~/.scout store
```

`beagle setup` runs the three scout steps for you (or run them yourself):

| What | Command | Where it goes |
|---|---|---|
| The `/scout` and `/scout-build` skills (plus the notifier app on macOS) | `beagle scout skill install` | `~/.claude/skills/scout*/SKILL.md`, `~/.codex/prompts/scout*.md` |
| Session hooks, for `watch` and the Session tab | `beagle scout hook install` | `~/.claude/settings.json` (backup: `settings.json.scout-backup`) |
| The default store | `beagle scout init ~/.scout` | `~/.scout/` |

Then, in Claude Code (start a new session so it sees the skills):

```text
/scout rate-limit the public API
```

Open `beagle scout` in another pane to watch the plan appear, review it, and
in a fresh session run `/scout-build rate-limit-the-api`. Add
`beagle scout watch` in a third pane to see every command it runs.

Scout works without the skills too: `beagle scout new`, `task add`,
`task start`, `log` and `task done` are plain commands
(`beagle scout help` lists them all).

## Learn more

| Doc | What's in it |
|---|---|
| [`tui/README.md`](tui/README.md) | The full guide: the skills, the TUI and its keys, `watch`, session logs, notifications and `scout.toml`, where projects live, the file format |
| [`tui/AGENTS.md`](tui/AGENTS.md) | Working on scout's code: module layout and rules |
| [`tui/skills/`](tui/skills/) | The `/scout` and `/scout-build` skills, as compiled into the binary |
| [`../ios/README.md`](../ios/README.md) | Scout on your iPhone (`ios/Beagle/Tools/Scout/`), read through `beagle serve` ([`docs/serve-api.md`](../docs/serve-api.md)) |

## Troubleshooting

| Symptom | Fix |
|---|---|
| `/scout` isn't offered in Claude Code | `beagle scout skill status`; `beagle scout skill install`; start a new session. |
| `beagle scout watch` shows no sessions | Hooks only record sessions started after `beagle scout hook install`; check with `beagle scout hook status`. |
| The TUI is empty, or shows other projects than the agent writes | Agent and TUI resolved different stores: run `beagle scout root` in both places. Set `root` in `~/.config/beagle/scout.toml` to use one store everywhere. |
| A project is missing from the list | A file failed to load: `beagle scout list` prints why. |
| No banners on macOS | `beagle scout notify test`; for the dog icon, `brew install terminal-notifier` then `beagle scout notify setup`. |

For more, see `beagle doctor` and the [`tui/README.md`](tui/README.md).

## For agents

Use the skills; they always match the CLI because both are compiled into
the same binary. Keep the current task's status true: start it before you
work, `beagle scout log --task <n> "…"` as you go, and never leave a task
`working` when you stop (`done`, `block --reason`, or `pause`).
