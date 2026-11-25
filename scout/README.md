# scout

`beagle scout`: track multi-step agent work as projects of small, ordered,
plainly explained tasks, and watch them live.

One folder per part you can run ([`docs/LAYOUT.md`](../docs/LAYOUT.md)):

| Part | What it is | Run it | Docs |
|---|---|---|---|
| [`tui/`](tui/) | The `beagle-scout` crate: the TUI and CLI that `beagle scout` runs, its embedded `/scout` skills and hooks | `beagle scout` | [`tui/README.md`](tui/README.md), [`tui/AGENTS.md`](tui/AGENTS.md) |

Scout's phone screens are in the Beagle iPhone app, [`ios/`](../ios/README.md)
(`ios/Beagle/Tools/Scout/`), which reads the store through `beagle serve`
([`docs/serve-api.md`](../docs/serve-api.md)).

scout ships inside the one `beagle` binary ([`beagle/`](../beagle/README.md));
it has no release of its own.
