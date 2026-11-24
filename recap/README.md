# recap

`beagle recap`: PR recaps, the diff plus a plain-language write-up of what
it fixes and why.

One folder per part you can run ([`docs/LAYOUT.md`](../docs/LAYOUT.md)):

| Part | What it is | Docs |
|---|---|---|
| [`tui/`](tui/) | The `beagle-recap` crate: the TUI and CLI that `beagle recap` runs, and the agent guide it embeds (`tui/docs/`) | [`tui/README.md`](tui/README.md) |

recap ships inside the one `beagle` binary ([`beagle/`](../beagle/README.md));
it has no release of its own.
