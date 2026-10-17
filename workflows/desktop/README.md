# Agentdesk

A macOS desktop view of [agentd](../README.md) — the same runs, write-ups
and job triggers `agenttop` shows in the terminal, in a window.

```
agentd (daemon, the only writer)
        │
        ▼
  ~/.local/state/agentd/state.db ──┬── agenttop   (TUI)
                                   └── Agentdesk  (this app)
```

Both are views of one SQLite file, so they cannot disagree.

## Two pages

```
┌───────────────────────────────────────────────┐
│ Agentdesk        [ Runs | DAG ]      Run job  │
├───────────────────────────────────────────────┤
│ RUNS                 │ Detail · Log · Write-up│
│ ┌──────────────────┐ │ ┌────────────────────┐ │
│ │ active first,    │ │ │                    │ │
│ │ then newest      │ │ │                    │ │
│ └──────────────────┘ │ └────────────────────┘ │
└───────────────────────────────────────────────┘
```

**Runs** is one run at a time: what it is doing, what it printed, what it
wrote up. **DAG** is the shape of the work and what it cost, full width —
a different question, so it is a page rather than a tab, and it does not
share the window with a run list it has no use for.

## What it does

- **Runs list** — active first, then finished newest-first, with state,
  duration and cost.
- **Detail** — parameters, model, tool calls, tmux session, and the run's
  links: chain write-up, stage write-up, Linear ticket, `result.md`.
- **Write-ups rendered**, not shown as source — the same call `ag -i`
  makes when it pipes through glow instead of bat.
- **Logs** with a line filter, tailing live.
- **Run a job** — the manifest's declared parameters, required ones
  enforced before anything is queued, exactly like `ag -j`.
- **Kill** a running run.
- **DAG page** — three views of the same structure:
  - **graph** — the chain as a picture, a column per depth, laid out so a
    parent centres on its children (what GitHub Actions does for a
    workflow). Click a node to select that run; the root carries the
    chain total.
  - **flows** — agenttop's indented lineage forest, better for scanning
    many chains and for seeing run ids.
  - **topology** — the _declared_ structure from the manifests, grouped
    by project, with each agent's last run and its manifest. True even
    when nothing has run yet, which the lineage cannot show.

### Keys

|       |                                            |
| ----- | ------------------------------------------ |
| `d`   | toggle the DAG page                        |
| `D`   | pin the selected run's chain, and go there |
| `esc` | unpin; if nothing is pinned, back to Runs  |
| `x`   | kill the selected run                      |

The keys agenttop uses, because that is muscle memory. `esc` is layered
on purpose — one press never skips a level and loses where you were.

## Running it

```sh
make install     # once
make dev         # debug build, window titled "Agentdesk Dev"
make build       # release .app + .dmg under target/release/bundle
```

Quality gate (what CI runs), from the repo root: `make check-workflows`.
`make help` lists the rest.
