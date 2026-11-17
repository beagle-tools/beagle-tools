# beagle standup — agent guide

How to write, update and answer questions about standup reports. The binary
carries this file: run `beagle standup guide` from anywhere to print it. Read
it once; after that the **Quick reference** is enough.

---

## 1. What this is

`beagle standup` is a CLI and terminal UI. A **report** is a folder of plain
files, one per standup date: a big-picture **overview**, then one short
write-up per **ticket** (an issue in whatever tracker the team uses), with
optional ASCII diagrams.

- **Agents write the files**, using the CLI to scaffold them.
- **A person reads them in the TUI** (`beagle standup`). It watches the
  folder and re-renders live, so the report fills in as you write it.
- **The goal is a catch-up, not a record.** A reader should understand what
  happened since the last standup in a couple of minutes. It is not a spec,
  a runbook or a status report for anyone else.

```text
 agent ──writes──▶ standups/2026-10-27/…  ◀──watches── TUI ──▶ reader
   ▲                                                     │ o → ticket
   └──── beagle standup show / list --json (read back) ──┘ p → PRs
```

---

## 2. Quick reference

```sh
beagle standup guide                  # this document
beagle standup last                   # newest report date (= "since" for the next one)
beagle standup list [--json]          # every report, newest first
beagle standup show [DATE]            # a whole report as one markdown document
beagle standup path [DATE]            # the report folder (no DATE: the standups/ dir)
beagle standup new [DATE] [--since DATE] [--title TEXT]
beagle standup ticket <ID> --title TEXT [--date DATE] [--url URL] [--state STATE]
                     [--system NAME]... [--pr URL]... [--update]
beagle standup [--date DATE]          # the TUI
```

- `DATE` is always `YYYY-MM-DD`. `--root DIR` works on every command.
- Commands that print a path print only the path, so
  `cd "$(beagle standup path)"` works.
- Exit codes: `0` ok, `1` runtime error, `2` bad arguments.

---

## 3. Where reports live

The **store root** is a folder containing `standups/`. It is chosen in this
order, first match wins:

1. `--root DIR`
2. `$STANDUP_ROOT`
3. `root = "…"` in `~/.config/standup/config.toml`
4. the nearest folder at or above the working directory that has `standups/`
5. the working directory itself

Run `beagle standup path` before writing anything to confirm the store. In an
unrelated folder, rule 5 quietly creates a new `./standups/` there.

---

## 4. The on-disk format

```text
standups/
  2026-10-27/                  one folder per standup, named by date
    report.toml                title, since, created
    overview.md                the big picture (read first)
    diagrams/01-name.txt       big-picture ASCII diagrams
    tickets/
      ABC-123/                 one folder per ticket, named by id
        ticket.toml            id, title, url, state, systems, prs
        summary.md             the ticket write-up
        diagrams/01-name.txt   that ticket's diagrams
```

`report.toml`:

```toml
title = "Gateway errors + release prep"   # optional one-line headline
since = "2026-10-26"                      # previous standup; `new` fills it
created = "2026-10-27T15:10:00-04:00"     # set by `new`
```

`ticket.toml`:

```toml
id = "ABC-123"                                       # upper-cased; = folder name
title = "Gateway returns 502s on pod restart"
url = "https://tracker.example.com/issue/ABC-123"   # what `o` opens
state = "In Review"                                  # the tracker's state name
systems = ["api-gateway", "web-server"]              # parts of the stack it touches
prs = ["https://github.com/acme/widgets/pull/42"]    # what `p` opens
```

- **Unknown keys are rejected.** A typo makes the TUI skip that ticket and
  show a red load warning on the Overview. Prefer the CLI to hand-editing.
- Ticket ids are 1–32 chars of `A-Z a-z 0-9 - _`, stored upper-cased.
- Diagrams are `*.txt` files, shown in file-name order: number them.
- Files over 2 MiB are refused with a warning; bad UTF-8 is repaired.
  Nothing you write can crash the TUI.

How the TUI shows it:

- The sidebar lists `★ Overview`, then the tickets sorted by state
  (in progress → review → blocked → done → todo → other).
- The Overview shows the headline, state counts, `overview.md`, its
  diagrams, then "tickets at a glance": each ticket with the **first line
  under its `## TL;DR` heading**, so make that line stand on its own.
- A ticket shows its header (title, state, systems, url, PRs), then
  `summary.md`, then its diagrams.
- `d` picks a report by day; `[` / `]` step to the older / newer report.

---

## 5. Writing a report

### Who is reading

Assume the reader knows the team but not every corner of the system. Each
ticket says **where in the system** the work happens before it says what
changed, and explains how a well-known tool is wired in rather than what it
is. Short beats complete.

### Steps

1. **Find the window.** `beagle standup last` gives the previous report
   date: everything since then is in scope. With no previous report, use the
   last working day.
2. **Gather what happened.** Use whatever sources you have: the tracker
   (tickets assigned to the author and updated since the cutoff, with their
   URLs), merged and open PRs (`gh search prs --author @me --updated
   ">=<since>"`), commits, and anything the reader points you at. A PR with
   no ticket is still a task; give it a short made-up id (`TASK-1`).
3. **Read before writing.** Check the code or the team's docs for each
   system a ticket touches. If docs and code disagree, trust the code and
   say so.
4. **Scaffold.**

   ```sh
   beagle standup new --title "one-line headline for the day"
   beagle standup ticket ABC-123 --title "…" \
     --url https://tracker.example.com/issue/ABC-123 \
     --state "In Review" --system api-gateway --pr https://github.com/acme/widgets/pull/42
   ```

5. **Write the overview first** (the reader may be watching live), then each
   ticket's `summary.md`.
6. **Add diagrams** (see §6), then **check your work** (§7).

On a busy day, group the work into 3–5 **threads** in the overview and give
a ticket section only to the ones that moved most; name the rest inside the
threads.

### `overview.md`

```markdown
# Since last standup (Mon 10-26 → Tue 10-27)

3–6 sentences: the main threads of work, how they relate, what changed.

## Threads
- **Gateway errors**: root cause found, fix in review. (ABC-123)
- **Release prep**: changelog done; smoke tests in progress. (ABC-130)

## Needs attention
- Anything blocked, waiting on a decision, or waiting on review, with PR links.
```

### `tickets/<ID>/summary.md`

Keep the scaffolded headings, in order:

- `## TL;DR` — 2–3 sentences: what's wrong and what's being done. The first
  line must stand alone (the Overview shows it).
- `## Where this sits in the stack` — the big picture in 1–3 sentences, then
  a short numbered flow down to the piece this ticket touches.
- `## The problem` — how it actually breaks, step by step.
- `## Why this ticket is needed` — who or what it hurts if left alone.
- `## The fix` — what the change does and why that fixes it; PR links.
- `## Where it stands` — PR state, CI, what's verified.
- `## What we need to do` — `- [ ]` / `- [x]` items, human steps included.

Keep real terms and explain each the first time it appears ("a **liveness
probe** (the health check that decides whether to restart a pod)"). Keep the
evidence light: at most a few paths, queries or numbers per ticket. Aim for
2–5 bullets per section.

---

## 6. Diagrams

- Put them in `diagrams/NN-name.txt` (report or ticket level), or in a
  ```` ```text ```` fence. Both render unwrapped; the reader pans with
  `h`/`l`, but keep lines ≤ 100 columns.
- Show the path a request or job takes and mark where the change lands
  (`◀── this ticket`, `◀── broke here`). A before/after pair explains a fix
  best.
- Colour is optional, via ANSI SGR codes (zero-width, so alignment
  survives): red broken, green fixed, yellow degraded. Close with `\e[0m`.

```text
 browser ──▶ api-gateway ──▶ web-server pods ──▶ database
               │ keepalive pool        ▲
               └─ reused a dead conn ──┘ ◀── broke here (502)
```

---

## 7. Checking and changing a report

```sh
beagle standup list        # right ticket count and since?
beagle standup show | less # read it as the reader will
```

- [ ] the overview reads in under a minute and names every thread
- [ ] every ticket has a url, a state and a TL;DR that stands alone
- [ ] every ticket says where it sits before what changed
- [ ] nothing load-bearing: no runbooks, no exhaustive evidence

| To… | Do |
|---|---|
| change a ticket's state, url or PRs | re-run `beagle standup ticket <ID> … --update` with **every** flag: it rewrites `ticket.toml` from the flags and never touches `summary.md` |
| write a ticket | edit `$(beagle standup path DATE)/tickets/<ID>/summary.md` |
| backfill an older day | `beagle standup new 2026-10-23 --since 2026-10-22`, then `--date 2026-10-23` on every `ticket` call |
| remove a ticket | delete its folder, only when asked |

Reports are snapshots: don't edit a past report unless asked. If the store is
a git repo, follow its own contributing rules for committing.

---

## 8. Answering questions about a report

`beagle standup show DATE` prints the overview, every ticket and every
diagram (ANSI removed). Answer from the report first, then go deeper through
the ticket URLs, the PRs and the code, and say which you used.
`beagle standup list --json` returns
`[{ "date": "2026-10-27", "title": "…", "since": "2026-10-26", "tickets": 8 }]`.

---

## 9. Markdown that renders

Headings (`#`–`###`), `-` bullets (nested by indent), `- [ ]` / `- [x]`
checklists, code fences (never wrapped), `>` quotes, `---` rules, `**bold**`
and `` `code` ``. Single-line `<!-- comments -->` are hidden. Tables and links
show as plain text, so put tables in a code fence.
