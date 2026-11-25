# standup

`beagle standup` digs up what you did since the last standup so you don't
have to. An agent gathers your tickets, PRs and commits since then and
writes them up as plain files; the TUI shows one big-picture **overview**,
then a short write-up per **ticket**, with ASCII diagrams and one-key links
to the ticket and its PRs.

It's a catch-up, not a record: two minutes before standup, you get the
context back.

## Quick start

standup ships inside the one `beagle` binary; it has no release of its own.

```sh
brew install beagle-tools/tap/beagle   # or from source: ../beagle/README.md
```

Pick where reports live (optional, recommended). Without this, they land in
`./standups/` under whatever folder you run the command in:

```toml
# ~/.config/beagle/standup.toml
root = "~/notes"          # reports go in ~/notes/standups/<date>/
```

Then ask your agent: "write today's standup". It follows the guide the
binary carries (`beagle standup guide`): it finds the window
(`beagle standup last`), gathers what changed through whatever tools it
has (`gh` for PRs and commits, your tracker's CLI or MCP for tickets),
scaffolds the report with `beagle standup new` and `beagle standup ticket`,
and writes it. Open the TUI to watch it fill in:

```sh
beagle standup                         # the newest report; `?` lists the keys
```

standup needs no `beagle setup` step, and talks to no service itself: the
agent does the gathering.

## Learn more

| Doc | What's in it |
|---|---|
| [`tui/README.md`](tui/README.md) | Every command, where reports live, the keys |
| [`tui/docs/AGENT_GUIDE.md`](tui/docs/AGENT_GUIDE.md) | How an agent writes a report: the on-disk format, what goes in the overview and each ticket, diagrams (also `beagle standup guide`) |

## Troubleshooting

| Symptom | Fix |
|---|---|
| The TUI says there are no reports, but the agent wrote one | They used different stores: `beagle standup path` in both places; set `root` in `~/.config/beagle/standup.toml`. |
| A `standups/` folder appeared somewhere odd | Same cause: no `root` set, so the current folder was used. |
| A config warning at startup | The file didn't parse (unknown keys are rejected); standup runs on defaults until it's fixed. |
| `o` / `p` open nothing | The ticket has no `--url` / `--pr`. Re-run `beagle standup ticket <ID> --title … --url … --pr … --update` with every field: `--update` rewrites the whole `ticket.toml` (the write-up is kept). |

## For agents

Run `beagle standup guide` and follow it; run `beagle standup path` before
writing anything to confirm the store. Read reports back with
`beagle standup show [DATE]` or `beagle standup list --json`.
