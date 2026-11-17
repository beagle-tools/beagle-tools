---
name: scout
description: Track multi-step agent work as a Scout project — one overarching goal broken into small, ordered, plainly-explained tasks that the user watches live in the `beagle scout` TUI. Use when starting any multi-step piece of work, when the user says "/scout", "start a scout project", "track this in scout", "what were we doing", or when resuming work on an existing scout project. Always keep the current task's status accurate while working.
---

# Scout — keep the user and the agent on the same page

The user watches a TUI (`beagle scout`) that shows every project, its tasks, which
task you are on **right now** (a spinner), and a live activity log. It exists
because agent and user lose track of what is being done and why. Your job:
keep that picture true, at all times, and write it so someone who does not
know the stack can follow it.

Everything lives in plain files under the store (`--root`, else
`$SCOUT_ROOT`, else `~/.scout`). Drive it through the `beagle scout` CLI — it
validates input, numbers tasks, stamps times, and logs every change.

```text
~/.scout/<project>/
  project.md        goal · problem · how the stack works · plan · done-when
  log.md            timestamped activity (Log tab)
  tasks/NN-slug.md  one file per task, NN = order
```

## Words

- **Project** — one overarching goal ("One noisy client can't take the API down").
- **Task** — one small, precise step toward it ("Add a per-key rate limit
  to the gateway config in `gateway/limits.yaml`"). Tasks run in order.
  Separate deliverables are separate tasks: *change the code* and *release
  the code* are two tasks, not one.

## Rule zero: the user knows nothing about this stack

Write every project and task for a smart person who has never seen this
system. Do not start at the low-level detail; start with what the piece *is*,
where it sits, and why it matters, then narrow down.

Bad (assumes context):

> Add a token bucket because one key is hammering us.

Good (gives the missing picture first):

> The API gateway is the front door of the service: every request from the
> internet passes through it before reaching an app server. Today it has no
> per-client limit, so one client sending thousands of requests a second
> fills every server's queue and everyone else starts getting timeouts. A
> per-key rate limit at the gateway turns that client's excess into fast
> 429s, before it reaches anything else.

Bad project framing: "Add rate limiting." Good: "All traffic enters through
the API gateway, which forwards it to the app servers. Today nothing stops one
client from using all of their capacity; we want each API key held to a fair
share, and the right place to enforce that is the gateway, before traffic
reaches anything else."

Define each component in two or three sentences the first time it appears.
Prefer a small ASCII diagram for any flow with more than two hops.

## Starting a project (the kickoff flow)

1. **Understand the goal.** If the outcome or its scope is unclear, ask the
   user one or two sharp questions before writing anything, using the
   **AskUserQuestion** tool with concrete options (recommended first), not
   prose. Then continue with their answer. Otherwise go.
2. **Research enough to explain it.** Read the code, configs, docs, tickets
   until you can explain how this part of the stack works in plain words.
3. **Create the project:**
   ```sh
   beagle scout new <project-slug> --title "One-line goal" [--link ABC-123]...
   ```
   Slug: lowercase `a-z0-9-`, ≤ 64 chars (e.g. `rate-limit-the-api`).
4. **Write `project.md`** (path printed by `beagle scout new`; also
   `beagle scout path <project>`). Replace every `<placeholder>`; keep the headings:
   - `## Goal` — one or two sentences: what is true when this is finished.
   - `## The problem` — what is wrong today, who it hurts, why now.
   - `## How this part of the stack works` — the guided tour (rule zero).
   - `## The plan` — the approach in a few sentences.
   - `## Done when` — `- [ ]` observable outcomes that prove the goal is met.
5. **Plan the tasks, in order** (next section), adding each with
   `beagle scout task add` and filling its file.
6. **Show the user the plan** — run `beagle scout show <project>` and summarize it —
   and get a go-ahead (AskUserQuestion: `Start building (Recommended)` ·
   `Change the plan` · `Stop here`) before starting the first task, unless
   they already told you to just proceed. Executing the plan task by task is the
   `/scout-build` skill (`/scout-build <project>`); mention it.

## Writing tasks

```sh
beagle scout task add <project> <task-slug> --title "Precise one-line objective" [--link <ref>]...
```

The command prints the task file's path. Fill in every section:

- `## Context` — what the piece of the system this task touches is, in two or
  three sentences, and where it sits in the stack (rule zero).
- `## Objective` — exactly what changes and **where**: repo, file, resource,
  command. "Update the gateway" is too vague; "Set
  `requests_per_minute = 600` per API key on the `public` route in
  `gateway/limits.yaml`" is a task.
- `## Why this moves the goal` — the justification: how finishing this gets
  the project closer to its goal. If you cannot write this, the task does not
  belong in the project.
- `## Steps` — a `- [ ]` checklist of the concrete steps. Tick them (`- [x]`)
  as you go; the TUI shows the count.
- `## Done when` — a `- [ ]` check anyone could run to confirm it is finished.
- `## Outcome` and `## Next steps` — left as-is until the task closes or
  work pauses. Together they are the task's handoff, shown on the TUI's
  **Outcome** tab.

Granularity: one task = one deliverable a person could verify on its own
(a PR, a deploy, a config change, a finding written down, a decision).
Prefer more, smaller tasks — the user monitors progress through them.
Typical shapes: *investigate X and write down what you find* · *change the
code in Y* · *open the PR* · *release / deploy it* · *verify it in
production*.

Links: attach related tickets, PRs, docs, or the user's backlog
tickets (`backlog:TASK-12`) with `--link` or `beagle scout link <project> [--task N] <ref>`.

## While working — keep the TUI true

Before touching anything for a task:

```sh
beagle scout task start <project> <task>     # task = number (3), id, or slug
```

`start` also **attaches this Claude session** to the task: the user's TUI
then shows your whole session (prompts, thinking, commands, output) live on
the task's Session tab. Nothing else to do; the TUI handles the rest.
Continuing a task in a new session (after `/clear` or a restart)? Run
`beagle scout task attach <project> <task>` once so the new session shows up too.

Then, as you go:

- **Narrate** every meaningful step, one line each — the user reads this in
  the Log tab as it happens:
  ```sh
  beagle scout log <project> --task <task> "found the route table in gateway/limits.yaml; 3 routes today"
  ```
  Log decisions and surprises especially. Say what you found, not just what
  you ran.
- **Tick steps** in the task file's checklist as they complete.
- **Finish** by writing the handoff, then closing the task:
  - `## Outcome`: what was done, PR links, anything surprising.
  - `## Next steps`: what should happen next, and anything the next task
    or session needs to know (a decision made, a gotcha found, a command to
    run first). `beagle scout show` prints the latest next steps, so this is how
    the next agent knows where to pick up.

  ```sh
  beagle scout task done <project> <task>
  ```

  Stopping mid-task? Write `## Next steps` anyway: exactly where you
  stopped and what to do first when resuming.
- **Blocked?** `beagle scout task block <project> <task> --reason "<the question
  the user must answer>"`. The reason is logged and sent to the user's Mac
  as a "Needs you" notification. Then ask the user with AskUserQuestion
  (concrete options, recommended first). When they answer, log the decision,
  `beagle scout task start` again, and keep going.

The user may also be watching your terminal commands live in a side pane
(`beagle scout watch`, fed by Claude Code hooks). There, your `beagle scout task start`
calls show as task banners and your `beagle scout log` lines as narration between
your commands, so log *before* a burst of commands to say what they are for.
Prefer one clear command over a long chained one-liner, and give each
command a meaningful description.

If a task you are working shows `paused`, scout paused it because it was
quiet for a while (`[auto] pause_after`, default 30 minutes). Your next
`beagle scout log --task` on it, or `beagle scout task start`, sets it back to `working`.
Log at each meaningful step so long silences really do mean "stalled".

Work one task at a time. The spinner means "the agent is on this right now",
so never leave a task `working` when you stop — close it, block it, or set it
back with `beagle scout task status <project> <task> todo` before ending the session.

## Replanning

Plans change; the record should show why.

- New work discovered → `beagle scout task add` (it is numbered last; mention the
  intended order in its Context if that matters) and log why it was added.
- A task turns out to be unnecessary → `beagle scout task skip` (keeps the record)
  and log why. Use `beagle scout task rm` only for mistakes.
- The goal itself changed → update `project.md` and log it; tell the user.

## Resuming ("where were we?")

At the start of a session on existing work:

```sh
beagle scout list                      # every project and what is being worked
beagle scout show <project>            # goal + ordered tasks with status
```

Read the `working` (or next `todo`) task's file and the tail of `log.md`
before doing anything, then `beagle scout task start` and continue.

## Closing a project

When every `## Done when` item in `project.md` holds, tick them, log a
one-line summary, and run `beagle scout status <project> done`. Abandoned instead?
`beagle scout status <project> dropped`, with the reason logged.

## Reference

| Command | Does |
|---|---|
| `beagle scout new <p> --title T [--link R]...` | create a project (planning) |
| `beagle scout task add <p> <slug> --title T [--link R]...` | append a task |
| `beagle scout task start\|done\|block\|skip <p> <t> [--reason R]` | change a task's status (`--reason` is logged; on `block` it is the notification text) |
| `beagle scout task status <p> <t> <todo\|working\|blocked\|done\|skipped>` | same, explicit |
| `beagle scout task rm <p> <t>` | delete a task file |
| `beagle scout task attach <p> <t>` | attach this Claude session (`start` does it) |
| `beagle scout log <p> [--task <t>] <message>` | narrate a step |
| `beagle scout link <p> [--task <t>] <ref>` | attach a ticket / PR / URL |
| `beagle scout status <p> <planning\|active\|paused\|done\|dropped>` | project status |
| `beagle scout list [--json]` · `beagle scout show <p>` · `beagle scout path <p> [<t>]` | read |

Files are Markdown with TOML frontmatter between `+++` fences. Edit the body
freely; leave the frontmatter to the CLI (it rejects unknown fields, and
times must be quoted RFC 3339 strings). Statuses: project
`planning → active → paused → done | dropped` (starting a task moves a
planning project to active); task `todo → working → done`, or `paused`
(started, nobody on it; set automatically after a quiet spell), `blocked`,
or `skipped`.
