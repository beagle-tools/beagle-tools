//! The `beagle scout --help` text.

/// Printed by `beagle scout help` and after every usage error.
pub(crate) const USAGE: &str = "\
beagle scout — watch what your agents are working on

USAGE
  beagle scout [--root <dir>]                     open the TUI
  beagle scout list [--json]                      every project, one per line
  beagle scout show <project>                     the goal and the ordered tasks
  beagle scout path <project> [<task>]            where a project or task file lives
  beagle scout root                               which store is in use, and why
  beagle scout init [<dir>]                       make <dir> (default: here) a store

  beagle scout new <project> --title <t> [--link <ref>]...
                                                  start a project (status: planning)
  beagle scout status <project> <status>          planning|active|paused|done|dropped
  beagle scout link <project> [--task <task>] <ref>
                                                  attach a ticket, PR, or URL

  beagle scout task add <project> <slug> --title <t> [--link <ref>]...
                                                  append a task (numbered in order)
  beagle scout task start|done|block|skip|pause <project> <task> [--reason <text>]
  beagle scout task status <project> <task> <status>
                                                  todo|working|paused|blocked|done|skipped
  beagle scout task attach <project> <task> [--session <id>]
                                                  attach this Claude session (default:
                                                  $CLAUDE_CODE_SESSION_ID) to the task;
                                                  the TUI's Session tab shows it live.
                                                  `task start` attaches automatically.
  beagle scout task rm <project> <task>           delete a task file
  beagle scout session show <project> <task> [--follow]
                                                  print the task's attached sessions

  beagle scout log <project> [--task <task>] <message...>
                                                  narrate a step (the Log tab)

  beagle scout watch [<session>] [--auto] [--history <n>] [--commands-only] [--full] [--once]
                                                  stream an agent's terminal commands live.
                                                  No arguments: fuzzy-pick a session (fzf),
                                                  or auto to follow this Herdr tab's Claude.
                                                  <session>: pin one (an id or its first
                                                  chars). --auto: follow without asking.
                                                  --once: print the recent commands and exit.
  beagle scout watch --list                       recorded sessions, newest first
  beagle scout hook install | uninstall | status  record commands via Claude Code hooks
  beagle scout hook                               (called by the hooks; reads stdin)

  beagle scout notify setup                       build Scout.app so banners show the dog icon
  beagle scout notify test                        send a sample notification (macOS)
  beagle scout skill install | status             the /scout and /scout-build skills for
                                                  Claude Code + Codex
  beagle scout version | help

A <task> is its number (3), id (03-add-allowlist), or slug (add-allowlist).
The store is --root, else $SCOUT_ROOT, else the nearest folder above you with
a .scout-store file (see `init`), else `root` in ~/.config/scout/config.toml,
else ~/.scout.
";
