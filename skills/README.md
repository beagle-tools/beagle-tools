# Skills

The single source of truth for every coding-agent skill in this repo. Each
skill is `skills/<name>/SKILL.md` with `name` / `description` frontmatter.
`.claude/skills/<name>` are symlinks here, so Claude Code finds them when
working inside this checkout; `rca` embeds `beagle` and `beagle-review` into
its binary from these files (`rca/cli/src/skill.rs`).

| Skill                                                     | What it does                                                                                                                      |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| [`beagle`](beagle/SKILL.md)                               | Create and maintain RCA workspaces rendered live by the Beagle TUI and desktop app.                                               |
| [`beagle-review`](beagle-review/SKILL.md)                 | Load an RCA by id (`beagle context <id>`) and answer questions about it, read-only.                                               |
| [`beagle-create-issue`](beagle-create-issue/SKILL.md)     | Turn a feature description into one-PR-sized, agent-ready GitHub issues with exact code paths, tests and release labels.          |
| [`beagle-complete-issue`](beagle-complete-issue/SKILL.md) | Take an issue to a merged PR: branch, implement, test, open the PR, wait for the `gate`, squash-merge, watch the per-app release. |

The issue → PR → release flow these drive is in
[`docs/SDLC.md`](../docs/SDLC.md).

## Install

```sh
scripts/skills/install.sh                    # all skills, Claude Code + Codex
scripts/skills/install.sh --agent claude     # only Claude Code
scripts/skills/install.sh beagle-create-issue beagle-complete-issue
scripts/skills/install.sh --copy             # Claude: snapshot instead of symlink
scripts/skills/install.sh --dry-run          # show what would change
scripts/skills/install.sh --uninstall        # remove what it installed
```

| Agent       | Installed at                 | How                                                      |
| ----------- | ---------------------------- | -------------------------------------------------------- |
| Claude Code | `~/.claude/skills/<name>/`   | symlink to `skills/<name>/` (`--copy` to snapshot)       |
| Codex       | `~/.codex/prompts/<name>.md` | copy of `skills/<name>/SKILL.md` without its frontmatter |

- The paths match `rca/cli/src/skill.rs`, so `beagle update` and this script
  agree on where a skill lives.
- Claude symlinks track your checkout: `git pull` updates the skills. Use
  `--copy` if the checkout may move or be deleted.
- Codex is always a copy (re-run the script after pulling). Codex injects
  prompts verbatim, so the frontmatter is dropped — exactly what
  `beagle update` writes. A symlink there would let `beagle update`, which
  writes through links, overwrite the repo's `SKILL.md`.
- Re-running is safe. The script never replaces or removes a file or
  directory it didn't install (or that you've edited); `--force` does.
- Caveat: with Claude symlinks, `beagle update` sees `beagle`/`beagle-review`
  as outdated whenever your checkout differs from the binary's snapshot, and
  accepting its refresh writes the snapshot into your checkout. Decline it,
  or use `--copy`.

## Adding a skill

1. Create `skills/<name>/SKILL.md` with `name` and `description` frontmatter
   (the description says what it does and when to trigger it).
2. Keep it under 400 lines; put long reference material in sibling files.
3. If it should be discoverable inside this repo, add
   `ln -s ../../skills/<name> .claude/skills/<name>`.
4. Add a row to the table above.
