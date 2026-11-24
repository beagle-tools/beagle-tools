# AGENTS.md — scout (`beagle scout`)

Read [`../../beagle/AGENTS.md`](../../beagle/AGENTS.md) (the beagle tool contract) and
[`../../docs/CODING_STANDARDS.md`](../../docs/CODING_STANDARDS.md) first.

`beagle-scout` is a library crate behind `beagle scout`: a TUI + CLI that
shows what agents are working on, as projects (one goal) made of ordered
tasks. If you are **using** scout to track work, follow the `/scout` skill
(`skills/scout/SKILL.md`) to plan and `/scout-build`
(`skills/scout-build/SKILL.md`) to execute. This file is for working on
scout's own code.

## Layout

```text
src/lib.rs            ABOUT + run(): the beagle tool contract
src/app/              command dispatch and printing (all I/O for commands)
src/cli/              argument parsing and the usage text — pure, no filesystem or env
src/model.rs          Slug, ProjectStatus/TaskStatus, frontmatter structs
src/frontmatter.rs    `+++` TOML frontmatter split/parse/render
src/gitignore.rs      keeps session recordings out of git (init, and the first recording in a git-backed store)
src/store/            the on-disk store: list/load (mod), mutations (mutate), file helpers
src/markdown.rs       Markdown via beagle_tui::markdown (placeholders dimmed) + the section splitter
src/skill.rs          the /scout and /scout-build skills, embedded with include_str!
src/session/          recorded agent sessions: hook payload → JSONL event, list, pick
src/watch/            `beagle scout watch`: follow a recording, render it, spot scout calls
src/hooks/            `beagle scout hook install|uninstall|status`: edit ~/.claude/settings.json
src/editor.rs         $VISUAL / $EDITOR launch
src/config.rs         ~/.config/beagle/scout.toml (or the old ~/.config/scout/config.toml), defaults on error
src/autopause.rs      `[auto] pause_after`: pause working tasks that went quiet
src/transcript/       Claude transcript → scout entries (parse); incremental sync (sync)
src/transcript_view/  render sessions (Session tab, `beagle scout session show`), ANSI both ways
src/notify.rs         macOS notifications on status changes; click focuses the Herdr pane
src/notify_app.rs     `beagle scout notify setup`: build the dog-iconed Scout.app notifier
src/picker.rs         the session picker (fzf, or a numbered prompt)
src/ui/               App state (mod), pane loading, keys, drawing (view/), event loop
skills/               the two skills, compiled in
assets/               scout-dog.png (Noto Emoji, Apache-2.0; see assets/NOTICE)
```

## Rules

- The on-disk format (README "On disk"), store resolution (`src/app/root.rs`:
  `--root`, `$BEAGLE_SCOUT_ROOT` then `$SCOUT_ROOT`, `.scout-store`, config
  `root`, `~/.scout`), the config (`~/.config/beagle/scout.toml`, else the
  older `~/.config/scout/config.toml`; names in `config::DIRS`, resolved by
  `beagle_tui::config`) and the CLI are the agents' API. Changing
  any of them means updating the skills, the README and this file together.
- The skills are compiled into the binary; editing a `SKILL.md` changes the
  binary. Reinstall with `beagle scout skill install` after building.
- User-facing text says `beagle scout …`. Agents' own calls are recognised in
  command lines both as `beagle scout …` and as a bare `scout …`.
- `beagle scout hook` runs inside every Claude Code tool call: it must stay
  fast, never print to stdout, and always exit 0, whatever the input.
- `beagle scout hook install` edits the user's global Claude settings: keep it
  idempotent, order-preserving, backed up, and touching only scout's entries
  (`<beagle> scout hook`, plus the legacy standalone `<scout> hook`).
- State changes go through `App` methods and are unit-tested without a
  terminal (`ui/keys.rs` tests); drawing gets a `TestBackend` smoke test.
- Notifications are macOS-only (`terminal-notifier` / `osascript`); every
  other platform is a silent no-op.
- Keep key bindings aligned with the rca TUI's where an equivalent exists.
