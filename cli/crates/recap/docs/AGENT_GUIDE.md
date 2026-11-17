# beagle recap — agent guide

How to create, write and answer questions about **PR recaps**. The binary
carries this file: run `beagle recap guide` from anywhere to print it.

---

## 1. What this is

`beagle recap` is a CLI and terminal UI. A **recap** is one folder per pull
request holding the PR's metadata, its diff pulled from GitHub, and a
plain-language **write-up** of what the PR is about.

A person reads it in the TUI (`beagle recap`):

- the PR list on the left, grouped drafts → open → merged → closed;
- on the right, **1 Summary** (the write-up), **2 Diff** (side by side via
  `delta` when it is installed; `n`/`N` jump between files) and
  **3 Comments** (people's review threads, with agent reviews, §8).

The TUI re-renders live as you write.

**The goal:** the reader gets up to speed on a PR fast — what problem it
solves (one layer down), why it's needed, what the change does, and how to
review it. A recap is not a code review: it explains the PR, it doesn't
grade it.

```text
 beagle recap new <PR> ──gh──▶ recaps/<repo>-<n>/{recap.toml, diff.patch, summary.md}
 agent ──writes──▶ summary.md + diagrams/   ◀──watches── TUI ──▶ reader
```

---

## 2. Quick reference

```sh
beagle recap guide                                 # this document
beagle recap new <PR> [--ticket ID] [--ticket-url URL]   # pull metadata + diff (needs gh), scaffold
beagle recap refresh <ID|PR> [--ticket-url URL]    # re-pull metadata + diff; never touches the write-up
beagle recap sync                                  # ask GitHub for every PR's state now
beagle recap list [--json] [--archived]            # every recap, grouped by state
beagle recap show <ID|PR> [--diff]                 # one recap as markdown (+ the full diff)
beagle recap comments <ID|PR>                      # reviews, inline comments, conversation (read-only)
beagle recap comments sync                         # fetch open/draft PRs' threads into comments/ (local)
beagle recap comments pending [--json]             # threads waiting on an agent review (§8)
beagle recap comments review <ID> <THREAD> <FILE>  # install your review of a thread (§8)
beagle recap path [ID|PR]                          # its folder (no arg: recaps/)
beagle recap archive <ID> / unarchive <ID>         # move a recap to / from the archive (local)
beagle recap merge-check <ID|PR>                   # read-only: can it merge now, and how
beagle recap [ID|PR]                               # the TUI
```

- **PR** is `https://github.com/<owner>/<repo>/pull/<n>` or
  `<owner>/<repo>#<n>`. **ID** is the folder name `<repo>-<n>`, e.g.
  `widgets-42`. Every command accepts either.
- Commands that print a path print only the path.
- Exit codes: `0` ok, `1` runtime error, `2` bad arguments.

---

## 3. Where recaps live

The **store root** is a folder containing `recaps/`, chosen in this order:

1. `--root DIR`
2. `$RECAP_ROOT`
3. `root = "…"` in `~/.config/recap/config.toml`
4. the nearest folder at or above the working directory that has `recaps/`
5. the working directory itself

Setting `root` in the config makes `beagle recap` work from any folder (for
example right after `gh pr create`). Run `beagle recap path` to confirm.

---

## 4. The files

```text
recaps/
  widgets-42/
    recap.toml        metadata (written by `new` / `refresh`; don't hand-edit)
    diff.patch        `gh pr diff` output
    summary.md        THE WRITE-UP — yours to write
    diagrams/01-*.txt ASCII diagrams (optional, encouraged)
    comments/         local only (keep it out of version control):
      threads.json      the PR's threads from people, as last synced
      read.json         which threads the reader has seen (the TUI keeps it)
      reviews/<t>.md    an agent's review of thread <t> (§8)
  _archive/<id>/      archived recaps, still readable
```

`recap.toml` fields: `repo`, `number`, `title`, `url`, `state`
(`open|draft|merged|closed`), `author`, `branch`, `base`, `ticket`,
`ticket_url`, `additions`, `deletions`, `files`, `created`, `updated`,
`merged_at`. Unknown keys are rejected.

- `ticket` comes from a title prefix (`ABC-123: …`) automatically.
- Pass `--ticket-url` so `t` in the TUI opens the ticket.
- Diagrams are `*.txt` files in name order, never wrapped; ANSI colour is
  allowed.

---

## 5. Writing the recap

### Gather first (read-only)

1. **The PR:** `beagle recap show <ID> --diff`; `gh pr view <n> -R <repo>`
   for the description; `beagle recap comments <ID>` for reviews and inline
   comments with their replies.
2. **The ticket**, if there is one: most of the "why" lives there.
3. **The system:** read the docs for each system the PR touches, if the
   team keeps any. They change, so read them fresh.
4. **The code around the change:** read it rather than guessing what a
   function does from its name.

Never comment on, approve, edit or push to the PR. You only read.

### `summary.md`: eight sections, in order

Write for a smart engineer who has never seen this part of the system. Keep
the real terms but explain each in a few words the first time it appears,
and step back to the big picture before zooming in. Short beats complete.

```markdown
## TL;DR
2–3 sentences: what's wrong, and what this PR does.

## Where this sits in the stack
**Step back:** the big picture in 1–3 sentences.
**Zooming in:** a short numbered flow to the piece this PR touches.

## The problem
2–5 bullets: what goes wrong and how, in plain words.

## Why this PR is needed
2–3 bullets: who or what it hurts, and what happens if it's left alone.

## The fix
2–5 bullets: what the change does. What's out of scope, in one line.

## How to review it
2–4 bullets, most important first. The one section for detail: the file or
line to look at, a reviewer's concern and how it was answered, a risk.

## Where it stands
1–3 bullets: draft / open / approved / merged; do the checks pass.

## What we need to do
- [ ] remaining steps, including human ones (review, merge, verify)
- [x] done steps
```

- Code references, queries and config keys go only in "How to review it",
  and at most 2–3 of them.
- About 350–700 words in total.
- Before saving, re-read the TL;DR and "Where this sits" as a newcomer.

### Diagrams

For a PR that changes a flow, add `diagrams/01-*.txt` showing the path
before and after, and mark where the change lands (`◀── this PR`). Use
box-drawing ASCII, ≤ ~110 columns. Colour is optional via ANSI SGR codes:
red broken, green fixed; close with `\e[0m`.

### Never write into a recap

Secret values, tokens, account ids, internal hostnames or customer data. The
names of variables the code reads are fine.

---

## 6. Answering questions about a recap

`beagle recap show <ID>` gives the write-up; add `--diff` for the change.
Answer from it first, then the PR, the ticket and the code. Don't edit the
recap while answering unless asked.

---

## 7. Behaviour worth knowing

- **`new` refuses an existing recap** (and an archived one). `refresh`
  re-pulls the metadata and diff and never touches the write-up.
- **`gh` must be installed and logged in.** Its errors are shown verbatim.
- **Diffs over 16 MiB aren't loaded**; the Diff tab shows a warning.
- **PR state is live.** While the TUI is open, a background poller asks `gh`
  for every PR's state, review decision and CI on start and every 3 minutes.
  A state change is written to `recap.toml` and the list re-sorts.
  `beagle recap sync` does the same once. "Where it stands" is still yours
  to update.
- **The archive** is `recaps/_archive/<id>/`. Merged recaps archive
  themselves 24 hours after the PR merged. Don't restore or edit an archived
  recap unless asked.
- **Merging, closing and marking ready happen only in the TUI**, by a
  person, after a confirmation. Agents never do them.
- **Markdown that renders:** `#`–`###` headings, `-` bullets (nested by
  indent), `- [ ]` / `- [x]` checklists, code fences (never wrapped), `>`
  quotes, `---` rules, `**bold**` and `` `code` ``. Put tables in a fence.

---

## 8. Reviewing comments (local only)

People's comments on open and draft PRs are synced into each recap's
`comments/` (the TUI's **3 Comments** tab, or `beagle recap comments sync`).
What is ignored is configurable in `~/.config/recap/config.toml`:

```toml
[comments]
ignore_authors = ["dependabot[bot]", "github-actions[bot]"]  # never count
ignore_bots = true                                           # every GitHub bot
automated_markers = ["Automated reply"]                      # hide replies with this text
```

Threads where only the PR's author spoke, and threads resolved on GitHub,
are dropped. An agent reviews each new thread **locally**; the author reads
the review in the TUI and decides what to say on GitHub themselves.

**Never post, reply, resolve, approve or push anything on GitHub.** The
review goes to a file, through the CLI:

```sh
beagle recap comments pending --json                  # what needs a review
beagle recap show <ID>                                # what the PR does, in context
beagle recap comments <ID>                            # the threads as GitHub has them
beagle recap comments review <ID> <THREAD> <file.md>  # install your review
```

A thread is pending only when all of these hold:

- someone other than the PR's author wrote its newest comment;
- that comment is at least 10 minutes old (one review per burst);
- no review answers that comment yet;
- it is not an approval whose words ask nothing (no `?`).

A newer comment makes a reviewed thread pending again; the next review
replaces the old one.

### The review

Same voice as the summary: plain language, real terms explained once,
short. Five headings, in order — the first is shown on its own in the TUI's
right-hand pane:

```markdown
## In short
3–5 short bullets: who is asking what, where the conversation stands, your
verdict in a word (valid / partly / not / can't tell), and the one thing to do.

## What they're saying
One or two sentences: the reviewer's point, and who said it.

## What it's about
The part of the PR and the system it touches, one layer down.

## Is it right?
**Valid / Partly valid / Not valid / Can't tell** — then why, with the
evidence (file and line, doc, metric). Say what is confirmed and what is
inferred.

## What to do
The concrete next step: change X at `path:line`, or a draft reply the
author can adapt (never posted), or a specific question for the reviewer.
```

One review per thread, under two screens, with no secrets.
