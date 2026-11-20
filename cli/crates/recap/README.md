# beagle recap

Get up to speed on a pull request fast. `beagle recap` pulls a PR's metadata
and diff from GitHub, an agent writes a plain-language recap of it, and the
TUI shows them side by side:

- **1 Summary**: what problem the PR solves (one layer down), why it's
  needed, what the fix does, how to review it, and what's left.
- **2 Diff**: the change, rendered by your own `delta` when it is installed
  (side by side, like your git diffs), else a built-in coloured diff. A
  per-file list sits on top; `n`/`N` jump between files.
- **3 Comments**: the PR's open threads from people, one at a time as
  sub-tabs (`n`/`N`; `•` marks unread). Bots, ignored authors and
  automated replies are left out, and resolved threads disappear. Beside
  each thread sits an agent's local review of it (`J`/`K` scroll). Nothing
  is ever posted to GitHub.

While the TUI is open, a background poller checks each PR every 3 minutes —
state (the list stays grouped drafts → open → merged → closed), review
decision and CI — and shows them as badges. Comment threads sync every 5
minutes and on `r`.

```sh
beagle recap new https://github.com/<owner>/<repo>/pull/<n>   # pull a PR (needs gh)
beagle recap                                                  # open the TUI
beagle recap guide                                            # the agent guide
```

Then ask an agent to "write the recap for <PR>"; it follows
[`docs/AGENT_GUIDE.md`](docs/AGENT_GUIDE.md), printed by `beagle recap guide`.

## Commands

- `beagle recap new <PR>` / `beagle recap refresh <ID|PR>`
- `beagle recap list [--json] [--archived]` / `beagle recap sync`
- `beagle recap show <ID|PR> [--diff]` / `beagle recap path [ID|PR]`
- `beagle recap comments <ID|PR>` / `comments sync` / `comments pending` /
  `comments review <ID> <THREAD> <FILE>`
- `beagle recap archive <ID>` / `unarchive <ID>` / `merge-check <ID|PR>`

A PR is a URL or `owner/repo#n`; an ID is the recap's folder name,
`<repo>-<n>` (e.g. `widgets-42`).

## Where recaps live

In `recaps/` under a store root: `--root DIR`, else `$BEAGLE_RECAP_ROOT`
(or the older `$RECAP_ROOT`), else `root = "…"` in
`~/.config/beagle/recap.toml` (the older `~/.config/recap/config.toml` is
still read while the new file doesn't exist), else the nearest parent
directory that has `recaps/`, else the current directory. A config file that
doesn't parse prints a warning and recap runs on defaults.

The same config file controls what the Comments tab ignores:

```toml
root = "~/notes"

[comments]
ignore_authors = ["dependabot[bot]", "github-actions[bot]"]  # the default
ignore_bots = true                                           # the default
automated_markers = []   # e.g. ["Automated reply"]: hide replies a tool posted as you
```

## Keys

- `j/k` move (the PR list, or a line cursor in the pane) · `enter` focus in
  (list → pane → agent review) · `esc` back out
- `tab`/`]`, `shift-tab`/`[` or `1`/`2`/`3` Summary / Diff / Comments (and focus the pane)
- `n/N` next / previous file (or thread) · `J/K` `space` `ctrl-d/u` page ·
  `g/G` top / bottom · `h/l` pan sideways
- `v` select lines · `y` copy · `Y` copy the PR URL · `i` copy the recap id
- `o` open the PR · `t` open the ticket
- `D` draft ⇄ ready for review · `M` merge (only when approved, every check
  green and GitHub says it can; auto-merge while checks run) · `X` close
  without merging (and archive). Each asks first.
- `a` archive · `A` show the archive · `U` restore. Merged PRs archive
  themselves 24 hours after merging.
- `w` resize the PR list: ←/→ (h/l) width · `0` default · `Esc` done (not saved)
- `r` reload · `?` help · `q`/`Q` quit

## Develop

```sh
make check-cli     # from the repo root: fmt, clippy, tests, docs
```

Shared TUI pieces (markdown, wrapping, the scrolling pane, widgets) live in
`utils/tui` (`beagle-tui`).
