# standup

`beagle standup`: standup reports, one big-picture overview, then a short
write-up per ticket.

One folder per part you can run ([`docs/LAYOUT.md`](../docs/LAYOUT.md)):

| Part | What it is | Docs |
|---|---|---|
| [`tui/`](tui/) | The `beagle-standup` crate: the TUI and CLI that `beagle standup` runs, and the agent guide it embeds (`tui/docs/`) | [`tui/README.md`](tui/README.md) |

standup ships inside the one `beagle` binary ([`beagle/`](../beagle/README.md));
it has no release of its own.
