# beagle standup

A terminal UI for **standup reports**: an agent gathers what you did since
the last standup (tracker tickets, PRs, commits), writes it up as plain
files, and `beagle standup` renders it — one big-picture **overview**, then a
short write-up per **ticket** with ASCII diagrams and one-key links to the
ticket and its PRs.

It is a catch-up, not a record: it exists so you get the context back in a
couple of minutes.

```text
 standup  Tuesday 2026-10-27  since 2026-10-24   report 1/2  [ older · ] newer
┌ Items · 3 tickets ──────────┐┌ ABC-123 ────────────────────────────────────────┐
│ ★ Overview                  ││ Gateway returns 502s on pod restart             │
│   the big picture           ││ ● In Review  ·  api-gateway, web-server         │
│ ● ABC-130 In Progress       ││ ticket https://tracker.example.com/…  [o]       │
│   Release prep              ││                                                 │
│▌● ABC-123 In Review         ││ TL;DR                                           │
│   Gateway returns 502s on…  ││ The gateway returned 502s every time a pod…     │
└─────────────────────────────┘└─────────────────────────────────────────────────┘
 j/k move · enter/esc focus · v select · y copy · i id · o ticket · p PRs · d reports · ? help
```

## Use

```sh
beagle standup                 # open the TUI on the newest report (live-reloads)
beagle standup guide           # the agent guide: CLI, on-disk format, workflow
```

Ask an agent to write today's standup: it follows the guide
([`docs/AGENT_GUIDE.md`](docs/AGENT_GUIDE.md), printed by
`beagle standup guide`), finds the window (`beagle standup last`), gathers
your tickets and PRs since then, and writes the report while you watch.

```sh
beagle standup new [DATE] [--since DATE] [--title TEXT]
beagle standup ticket ABC-123 --title "…" --url … --state "In Review" \
  [--system api-gateway]... [--pr URL]... [--update]
beagle standup list [--json]   # reports, newest first
beagle standup show [DATE]     # the whole report as one markdown document
beagle standup last            # newest report date
beagle standup path [DATE]     # where it lives
```

## Where reports live

In a `standups/` directory under a store root, one folder per date. The root
is `--root DIR`, else `$STANDUP_ROOT`, else `root = "…"` in
`~/.config/standup/config.toml`, else the nearest parent directory that has
`standups/`, else the current directory.

## Keys

`j/k` move (the list, or a line cursor in the write-up) · `enter` open
the write-up · `esc` back · `J/K` `space` `ctrl-d/u` page · `g/G` top/bottom
· `v` select lines · `y` copy the line/selection (list: the ticket URL) ·
`Y` copy the ticket URL · `i` copy the id (the date, or `<date> <ticket>`) ·
`h/l` pan wide diagrams · `d` pick a report by day · `[`/`]` older/newer
report · `o` open the ticket · `p` open its PRs · `w` resize the list (←/→,
`0` default, `Esc` done) · `r` reload · `?` help ·
`q`/`Q` quit.

## Develop

```sh
make check-cli     # from the repo root: fmt, clippy, tests, docs
```

Shared TUI pieces (markdown, wrapping, the scrolling pane, widgets) live in
`utils/tui` (`beagle-tui`).
