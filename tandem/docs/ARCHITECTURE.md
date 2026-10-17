# Architecture

## Overview

Tandem is a Tauri 2 desktop app. All logic lives in a Rust workspace; the
React frontend is a thin, cache-fed view layer.

```
crates/tandem-core     domain types + unified-diff parser (no I/O)
crates/tandem-github   GitHub REST client (reads + user-only writes), pluggable auth
crates/tandem-cache    SQLite: GitHub cache + local review store
crates/tandem-agents   headless agent runners + JSONL run logs
src-tauri            Tauri shell: commands, events, settings, state
src/                 React UI (differ, comments, agent panel, settings)
```

## Concurrency model

The Tauri process hosts a tokio runtime. Rules:

- Commands are `async`; anything slow (network, child processes) happens
  on the runtime, never on the UI thread.
- The SQLite connection is wrapped in a `tokio::sync::Mutex` and held
  only for short, non-await sections. If contention ever shows up, the
  escape hatch is a connection pool — not `block_on`.
- Agent processes stream stdout/stderr through unbounded channels into a
  single event loop per run (`tandem-agents::runner`), which serializes to
  a JSONL log and forwards to the UI. Every run has a timeout and a
  cancel handle; `kill_on_drop` guarantees no orphaned children.

## Cache-first data flow

1. UI calls `get_*` → SQLite → renders instantly (possibly stale).
2. UI calls `sync_*` → GitHub REST → cache updated → fresh data returned.
3. Backend emits `tandem://sync {key, phase}` around every sync so the UI
   can show spinners/staleness per resource, not globally.

Cache keys: PR lists per repo, PR detail per (repo, number), diffs per
(repo, number, head_sha) — a new push invalidates naturally because the
SHA changes. Local comments/agent runs are indexed by (repo, number).

## Agent review flow

1. User configures an `AgentSpec` (runner, auth, model, prompt, tool
   allowlist, network allowlist, timeout). Stored in SQLite.
2. `start_agent_review` builds a prompt: Tandem context block (identity,
   the tandem_comment contract, "never post to GitHub") + user
   instructions + PR description + raw diff. Fed via stdin to:
   - claude: `claude -p --output-format stream-json --verbose [...]`
   - codex: `codex exec --json --skip-git-repo-check -`
   - custom: `sh -c <command>`
3. Runner streams events; `{"type":"tandem_comment"}` lines (stdout or the
   `$TANDEM_COMMENTS_FILE` drop-box) become `LocalComment`s tied to the
   run id and head SHA.
4. UI shows the live event feed; comments appear inline in the diff for
   triage (accept / reject / archive / delete), threaded discussion
   (reply, @mention an agent), and — only when you explicitly click
   post/approve — pushing a chosen comment or an approval to GitHub.

## Frontend

- `lib/types.ts` mirrors Rust serde output (snake_case, `kind`-tagged
  enums). `lib/ipc.ts` is the only place command names exist.
- `state/store.ts` (zustand) owns all cross-component state and event
  subscriptions. Components are presentational.
- The differ renders structured `FileDiff`s parsed in Rust — the
  frontend never parses diff text.

## Deliberate v0 tradeoffs

- JSON-blob columns in SQLite (indexed keys only) — schema agility while
  domain types settle.
- No syntax highlighting in the differ yet (roadmap).
- One SQLite connection behind a mutex — fine at current scale.
- PAT lives in the settings file (0600), not the keychain yet (roadmap).
