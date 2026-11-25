# Agentdesk

A macOS desktop view of [agentd](../README.md) — the same jobs, runs,
write-ups and job triggers the `beagle agent` dashboard shows in the
terminal, in a window.

```
agentd (daemon, the only writer)
        │
        ▼
  ~/.local/state/agentd/state.db ──┬── beagle agent  (the terminal dashboard)
                                   └── Agentdesk     (this app)
```

Both are views of one SQLite file, so they cannot disagree. Agentdesk does
not run agents itself: install `beagle` and start the daemon first
([setup](../README.md#set-it-up)), or the window shows an "agentd stopped"
banner and queued runs never start.

## Install

```sh
brew install --cask beagle-tools/tap/beagle-agentdesk
```

Or download `Agentdesk-macos-arm64.dmg` / `Agentdesk-macos-x64.dmg` from
the rolling
[`beagle-workflows-latest`](https://github.com/beagle-tools/beagle-tools/releases/tag/beagle-workflows-latest)
release. After that it updates itself: an _Update to vX.Y.Z_ pill appears
in the top bar when a release is out, and the version button beside it
can install any release, including rolling back.

## Three pages

```
┌──────────────────────────────────────────────────────────────┐
│ Agentdesk  [ Jobs | All runs | Activity ]   v…   [ Run job ] │
├──────────────────────────────────────────────────────────────┤
│ (daemon banner, errors)                                      │
│                                                              │
│ Jobs: a section per project, each job with its flow drawn    │
│ beneath it. Open one for its Shape and its Runs.             │
└──────────────────────────────────────────────────────────────┘
```

- **Jobs** (the landing page) — a section per project, each job with its
  whole flow drawn beneath it. Open a job for two tabs:
  - **Runs** — every run of the job, its steps beneath it; click a step
    to see its log, links and write-up right beside the list.
  - **Shape** — every branch the job has taken, as a tree.
- **All runs** — the Runs tab for every job at once, newest first. Its
  running-only filter answers "what is this machine doing right now".
- **Activity** — the DAG, full width, in three views of the same
  structure:
  - **graph** — the chain as a picture, a column per depth, laid out so a
    parent centres on its children. Click a node to select that run; the
    root carries the chain total.
  - **flows** — the dashboard's indented lineage forest, better for
    scanning many chains and for seeing run ids.
  - **topology** — the _declared_ structure from the manifests, grouped
    by project, with each agent's last run and its manifest. True even
    when nothing has run yet, which the lineage cannot show.

## What you can do

- **Run a job** — _Run job_ in the top bar, or a job's `⋯` → _Run now…_.
  It asks for the manifest's declared parameters and enforces the
  required ones before anything is queued, exactly like
  `beagle agent -j <job> -p k=v`.
- **Restart a failed step** — _Restart…_ on a failed, timed-out or killed
  step re-runs it in its chain, like `beagle agent resume`.
- **Kill** a running run.
- **Read write-ups rendered**, not as Markdown source, and **logs** with a
  line filter, tailing live.
- **Follow links** — parameters, model, tmux session, and the run's
  links: chain write-up, stage write-up, tracker ticket (from `ui.yaml`'s
  `ticket_url`), draft PR, `result.md`.

### Keys

|           |                                                                                              |
| --------- | -------------------------------------------------------------------------------------------- |
| `d`       | toggle the Activity page                                                                     |
| `D`       | pin the selected run's chain, and go to Activity                                             |
| `esc`     | unpin; else close the open job; else back to Jobs                                            |
| `x`       | kill the selected run                                                                        |
| `/`       | Jobs page: fuzzy-search jobs, steps and projects (`enter` opens the top match, `esc` clears) |
| `f` / `a` | Jobs page: only jobs with something running / all jobs (also the All · Running toggle)       |
| `H`       | Jobs page: show / hide the projects ui.yaml hides (also the "Show hidden" box)               |

The keys follow the terminal dashboard's, because that is muscle memory.
`esc` is layered on purpose — one press never skips a level and loses
where you were. Keys are ignored while you type in a field.

## Running it from source

From `agent/desktop/` (Node and pnpm setup:
[`../../docs/DEVELOPMENT.md`](../../docs/DEVELOPMENT.md)):

```sh
make install     # pnpm install, once
make dev         # debug build, window titled "Agentdesk Dev"
make build       # release .app + .dmg under target/release/bundle
pnpm e2e         # Playwright against a mocked backend (no Tauri, no agentd)
```

A dev build reads the same store as the release app, so the window title
is the only thing telling two open windows apart.

Quality gate (what CI runs), from the repo root: `make check-agent`.
`make help` lists the rest. Architecture and rules for contributors:
[AGENTS.md](AGENTS.md).
