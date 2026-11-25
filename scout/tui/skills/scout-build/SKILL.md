---
name: scout-build
description: Work through an existing Scout project task by task, in order, keeping every task's status true in the `beagle scout` TUI (start → narrate → tick steps → verify → outcome → done) until the project's goal is met. Use when the user says "/scout-build", "/scout-build <project>", "build the scout project", "work the project", "start on <project>", or "continue <project>". If no project is named, ask which one. Planning a new project is the /scout skill, not this one.
---

# Scout build — work a project, one task at a time

The user plans work as a Scout project (the `/scout` skill): one goal, broken
into small ordered tasks, watched live in the `beagle scout` TUI and, often, a
`beagle scout watch` pane showing every command you run. This skill **executes** that
plan: you take the tasks in order, and the TUI shows exactly where you are
at every moment. Follow the `/scout` skill's conventions throughout (plain
language for someone new to the stack, log narration, one task at a time).

## How you ask: interactively, then keep going

This skill is a conversation, not a batch job. Whenever it says *ask*, ask
with the **AskUserQuestion** tool (Claude Code's interactive picker), not as
a closing paragraph of prose. Put the real choices in it, recommended option
first and marked "(Recommended)", each with a one-line consequence. Add a
preview when comparing concrete things (diffs, commands, plans). The user can
always type their own answer.

Then **wait for the answer and continue in the same session**. Log the
decision (`beagle scout log … "decided: …"`), restore the task's status, and carry
on. Asking is a pause in the loop, not the end of it. Never end your turn
on a question you could have put in the picker. Where AskUserQuestion is not
available (e.g. Codex), ask the same options as a short numbered list and
wait.

While you wait, the task shows as `blocked` in the TUI, so the user can see at
a glance that the agent is waiting on them.

## 1. Pick the project

`/scout-build <project>` names it. If it doesn't, run:

```sh
beagle scout list
```

and **ask which project to work on** (AskUserQuestion): one option per
open project (`planning`, `active`, `paused`, up to four, most recently
active first), each described with its progress and current or next task,
e.g. `3/7 done · next: 04 Open the PR`. Never guess, even when only one is
open: confirm it. If there are no open projects, say so and offer to plan one
with `/scout`.

An optional second argument names a task to start from
(`/scout-build <project> 3`); otherwise you start where the project left off.

## 2. Get oriented ("where were we?")

```sh
beagle scout show <project>
```

Then read, before touching anything:

- **The handoff**: the `next steps (from …)` block at the end of
  `beagle scout show`. It is the latest task's `## Next steps`, written for you by
  whoever worked before. Follow it unless it conflicts with the plan; if it
  does, ask.
- `project.md` (`beagle scout path <project>`) — the goal, the stack tour, the
  plan, and `## Done when`. This is what every task is for.
- The tail of `log.md` in the same directory — what happened last time.
- The task you will work: the first `working` task, else the first
  `waiting` or `blocked` one, else the first `todo`.

Resolve leftovers from a previous session first:

- A task `paused` was started earlier and then went quiet (scout pauses
  quiet tasks automatically). Treat it like an interrupted `working` task:
  read its checklist, log, and `## Next steps`, then `beagle scout task start` it.
- A task already `working` was interrupted. Read its checklist and the log
  to see how far it got, run `beagle scout task attach <project> <task>` (so this
  session appears on its Session tab), then continue it.
- A `waiting` task: read what it waits on (its `--reason` in the log, e.g.
  `PR #12: waiting on CI`) and check it (`gh pr checks`, `gh pr view`). If
  the wait is over, `beagle scout task start` it and carry on (or close it). If
  not, leave it `waiting` and say so.
- A `blocked` task: read why (its log lines). If the blocker is gone, carry
  on. If not, ask (AskUserQuestion) with options such as: *it's resolved,
  continue* · *here is the input you need* · *skip ahead to the next
  unblocked task* · *replan this task*.

Tell the user, in two or three lines, where the project stands and which
task you are starting with.

## 3. The loop — for each task, in order

**a. Check the task is ready to execute.** Its `## Objective` must say
exactly what changes and where, and its `## Done when` must be checkable. If
it still has `<placeholders>`, is vague, or no longer makes sense given what
earlier tasks found, fix the task file first (and log why). If fixing it
needs a decision, ask (AskUserQuestion) with your proposed rewrite as the
recommended option.

**b. Start it** — before any other command for this task:

```sh
beagle scout task start <project> <task>
beagle scout log <project> --task <task> "starting: <one line on the approach>"
```

`start` attaches this Claude session to the task automatically; the user's
TUI shows it live on the Session tab. Nothing more to do for that.

**c. Do the steps.** Work the `## Steps` checklist top to bottom:

- `beagle scout log` before each meaningful step: what you are about to do and
  why, and afterwards what you found. Say findings, not just actions.
- Tick each step (`- [ ]` → `- [x]`) in the task file **as it completes**,
  not all at the end. The TUI shows the count.
- Discovered extra work that belongs to this task? Add a step to its
  checklist. Belongs elsewhere? `beagle scout task add` a new task (log why) and
  keep going.

**d. Verify.** Run the checks in `## Done when` for real (tests, commands,
looking at the result) and tick each only when it actually holds. If a check
fails, fix it and verify again. Never mark a task done on assumption.

**e. Write the handoff** (the TUI's **Outcome** tab):

- `## Outcome`: what was done, where (files, PRs, commits), how it was
  verified, anything surprising.
- `## Next steps`: what should happen next and what the next task needs to
  know, written for an agent with no memory of this session (decisions
  made, gotchas, the exact command or file to start from). If the next task
  in the plan should change because of what you learned, say so here, and
  update that task.

If the task file predates these sections, add them at the end.

**f. Close it:**

```sh
beagle scout log <project> --task <task> "done: <one-line result>"
beagle scout task done <project> <task>
```

**Waiting on CI, a review or a merge** (e.g. the PR is open and checks are
running, or it is green and waits for the user to merge): nothing is needed
from the user, so do not block. Mark it waiting (silent, never
auto-paused) and say what it waits on:

```sh
beagle scout task wait <project> <task> --reason "PR #12: waiting on CI"
```

When the wait ends, `beagle scout task start` it again to continue, or close it.
Never leave a task `working` while you are idle.

**g. Checkpoint with the user** in a few lines: what was done, how it was
verified, what's next. Then move to the next task. Go straight on by default.
If the user asked to pause after each task, or what comes next changes
direction, ask instead (AskUserQuestion): `Continue to 05 · <title>
(Recommended)` · `Stop here for now` · `Change the plan first`.

## 4. When to stop and ask

Set the task to `blocked` with the reason logged, ask with AskUserQuestion,
and continue once answered, when the user must answer or do something
(`blocked` sends a "Needs you" alert; waiting on CI, a review or a merge
is `wait`, not `block`, see step 3):

- **The next action is outward-facing or hard to undo**: pushing,
  opening or merging a PR, deploying, changing shared or production
  resources, sending messages, editing global config, deleting data. Ask
  first, even if the task says to do it. The plan is not permission.
- A decision is the user's: scope, trade-offs, naming, anything the task
  leaves open.
- You are stuck: the same check failed twice in a row with no new idea, or
  something outside the task is broken.

```sh
beagle scout task block <project> <task> --reason "<what is needed, from whom, as one question>"
```

The reason is logged *and* sent to the user's Mac as a "Needs you"
notification (with a sound). They may be in another window, so write it as
the question they need to answer, e.g. `"Push branch fix-retry and open a
PR against main?"`. Clicking the notification takes them to your pane.

Then ask. Examples of good questions:

- Approval: *"Push branch `fix-retry` and open a PR against `main`?"* →
  `Push and open PR (Recommended)` · `Push only, no PR` · `Not yet, keep
  going locally`. Put the diff summary or exact commands in the preview.
- Stuck: *"`cargo test uploader` still fails after two fixes: the mock
  returns 2 attempts, not 3. How should I proceed?"* → `Change the mock`
  · `Change the retry count` · `Skip this test for now and log it` ·
  `Stop here`.
- Decision: *"The objective allows either. Which?"* → the concrete
  alternatives, recommended first.

When the user answers:

```sh
beagle scout log <project> --task <task> "decided: <their answer, in a line>"
beagle scout task start <project> <task>
```

and carry straight on with the task. If they say stop, follow "Stopping
early" below.

## 5. Replanning as you go

Earlier tasks often teach you something that changes later ones. Keep the
plan true rather than following a stale one:

- New work → `beagle scout task add` (log why it is needed and where it belongs).
- A task is no longer needed → `beagle scout task skip` (log why). Never delete
  real work with `beagle scout task rm`.
- The goal itself looks wrong → stop and talk to the user before changing
  `project.md`.

## 6. Finishing

- **All tasks closed:** check every `## Done when` item in `project.md`
  for real, tick them, log a one-line summary, run
  `beagle scout status <project> done`, and give the user a short wrap-up: what was
  achieved, what was verified, anything left over (follow-ups, skipped tasks,
  risks).
- **Stopping early** (user says stop, session ending, out of time): never
  leave a task `working`. Write the task's `## Next steps`: exactly where you
  stopped, what is half-done, and what to do first when resuming. Log a
  line, then set it back to `todo` (`beagle scout task status <project> <task> todo`),
  `waiting` if it waits on CI, a review or a merge (`beagle scout task wait`), or
  `blocked` if it needs the user. Tell the user how to resume:
  `/scout-build <project>`.

## Rules

- One task `working` at a time, and it is always the one you are actually
  doing. The spinner in the TUI must never lie.
- Every status change goes through the `beagle scout` CLI (it stamps and logs it).
  Edit task bodies (ticks, outcome) directly; leave frontmatter to the CLI.
- Work in task order. Only skip ahead past a `blocked` task with the user's
  OK.
- `wait` for CI, reviews and merges; `block` only for questions the user
  must answer. Blocking to wait alerts the user for nothing.
- Prefer one clear command per step over long chained one-liners, and give
  each command a meaningful description. The user may be watching them live.
