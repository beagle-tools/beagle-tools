# recap

`beagle recap` is a good boy that fetches the PR and explains it. Point it
at a pull request and it pulls the metadata and diff from GitHub; an agent
writes a plain-language recap of what the PR fixes and why; the TUI shows
the write-up, the diff and the open review threads side by side. It's for
the moment you need to get up to speed on a PR fast: your own after a week
away, or someone else's before you review it.

- **1 Summary:** the problem (one layer down), why the PR is needed, what
  the fix does, how to review it, what's left.
- **2 Diff:** rendered by your own `delta` when it's installed, else a
  built-in coloured diff.
- **3 Comments:** the open threads from people (bots left out), each next to
  an agent's local review of it. Nothing is ever posted to GitHub.

A background poller keeps each PR's state, review decision and CI current
while the TUI is open.

## Quick start

recap ships inside the one `beagle` binary; it has no release of its own.

```sh
brew install beagle-tools/tap/beagle   # or from source: ../beagle/README.md
gh auth login                          # recap talks to GitHub through the gh CLI
```

Pick where recaps live (optional, recommended). Without this, they land in
`./recaps/` under whatever folder you run the command in:

```toml
# ~/.config/beagle/recap.toml
root = "~/notes"          # recaps go in ~/notes/recaps/
```

Then:

```sh
beagle recap new acme/widgets#42       # or the PR's URL
```

Ask your agent: "write the recap for acme/widgets#42". It follows the guide
the binary carries (`beagle recap guide`). Watch it fill in:

```sh
beagle recap                           # the TUI; `?` lists the keys
```

recap needs no `beagle setup` step. Optional: install
[`delta`](https://github.com/dandavison/delta) for side-by-side diffs.

## Learn more

| Doc | What's in it |
|---|---|
| [`tui/README.md`](tui/README.md) | Every command, where recaps live, `recap.toml` (store root, which comment authors to ignore), the keys |
| [`tui/docs/AGENT_GUIDE.md`](tui/docs/AGENT_GUIDE.md) | How an agent writes a recap and reviews comment threads (also `beagle recap guide`) |

## Troubleshooting

| Symptom | Fix |
|---|---|
| `new` fails talking to GitHub | `gh auth status`; `gh auth login`. `beagle doctor` checks this too. |
| The TUI is empty, or recaps appear in odd folders | Run `beagle recap path` to see which store is in use; set `root` in `~/.config/beagle/recap.toml`. |
| A config warning at startup | The file didn't parse (unknown keys are rejected); recap runs on defaults until it's fixed. |
| The Diff tab isn't side by side | Install `delta`. |

## For agents

Run `beagle recap guide` before writing a recap and follow it. Never edit
`recap.toml` or `diff.patch` by hand (`beagle recap refresh` re-pulls them),
and never post to GitHub on the reader's behalf. Use `--json` for lists.
