# `beagle serve` API (v1)

`beagle serve` answers read-only JSON over HTTPS so a phone app can show what
the beagle TUIs show: scout projects, RCAs, PR recaps, standups and Tandem
reviews. It reads each store through that tool's own crate, the same way
`beagle <tool>` does, so the phone sees exactly what the terminal sees.

Nothing here writes. Posting comments, approving PRs and changing statuses
stay on the Mac in v1. Push notifications, and the device endpoints they
need, are designed in [`serve-push.md`](serve-push.md).

## Transport

```text
 phone (Tailscale)  ──HTTPS + Bearer token──▶  beagle serve (Mac, Tailscale IP)
                                                 │ reads the same stores as
                                                 ▼ beagle scout/rca/recap/…
```

- **Tailscale only.** The server binds to the Mac's Tailscale address
  (`tailscale ip -4`) by default, never to every interface. Binding to
  `0.0.0.0` / `::` needs an explicit `--any-addr` flag.
- **Real HTTPS.** The certificate is the one `tailscale cert` issues for the
  Mac's MagicDNS name (`<mac>.<tailnet>.ts.net`), so iOS needs no App
  Transport Security exceptions. The app connects to
  `https://<mac>.<tailnet>.ts.net:<port>` (default port `8787`). HTTPS
  certificates must be enabled for the tailnet (admin console → DNS → HTTPS
  Certificates).
- Plain HTTP exists only for local testing: `--no-tls` is refused unless the
  address is loopback (`127.0.0.1`, `::1`).
- `tailscale cert` certificates last 90 days; re-run it and restart
  `beagle serve` to renew.

### Configuration

`~/.config/beagle/serve.toml` (`$XDG_CONFIG_HOME/beagle/serve.toml`), mode
`0600`, written by `beagle serve pair`:

```toml
token = "4f0c…64 hex chars…"          # the bearer token (secret)
host = "mac.example-tailnet.ts.net"   # MagicDNS name, from `tailscale status`
port = 8787
# Optional overrides (defaults shown):
# addr = "100.64.0.1"                  # the address to bind; default: `tailscale ip -4`
# cert = "~/.config/beagle/serve/mac.example-tailnet.ts.net.crt"
# key  = "~/.config/beagle/serve/mac.example-tailnet.ts.net.key"
```

The certificate and key are created with:

```sh
mkdir -p ~/.config/beagle/serve
tailscale cert \
  --cert-file ~/.config/beagle/serve/<host>.crt \
  --key-file  ~/.config/beagle/serve/<host>.key \
  <host>
```

When either file is missing, `beagle serve` exits with that exact command.

## Authentication

Every endpoint except `GET /v1/health` needs:

```http
Authorization: Bearer <token>
```

- The token is 32 random bytes as 64 lowercase hex characters, made by
  `beagle serve pair`. `beagle serve pair --rotate` replaces it, which logs
  out every paired phone.
- The server compares tokens in constant time. A missing or wrong token is
  `401` with `WWW-Authenticate: Bearer`.
- The phone keeps the token in the iOS Keychain.

### Pairing

`beagle serve pair` prints a QR code of:

```text
beagle://pair?url=https%3A%2F%2Fmac.example-tailnet.ts.net%3A8787&token=4f0c…
```

plus the URL and token as plain text. `url` and `token` are
percent-encoded query values. The first `pair` makes the token and saves
it (with the MagicDNS name) to `serve.toml`; later runs show the same
code. `pair --rotate` makes a new token and restarts the background
service so it takes effect.

### Running it

```sh
beagle serve                 # foreground, logs to stderr
beagle serve install         # launchd agent com.beagle.serve: starts at login,
                             # restarts if it exits (30 s throttle); re-run to restart
beagle serve status          # running? (also shown by `beagle doctor`)
beagle serve uninstall
```

The agent runs `beagle serve` from your home directory with your `PATH`,
plus any of `BEAGLE_SCOUT_ROOT`, `BEAGLE_RECAP_ROOT`, `BEAGLE_STANDUP_ROOT`,
`BEAGLE_RCA_ROOT`, `BEAGLE_TANDEM_ROOT`, `SCOUT_ROOT`, `RECAP_ROOT`,
`STANDUP_ROOT`, `BEAGLE_CONFIG`, `TANDEM_TUI_DB`, `XDG_CONFIG_HOME` and
`XDG_DATA_HOME` set when you ran
`install`, so it reads the stores your shell's `beagle` commands do. Logs:
`~/.local/state/beagle/serve/serve.{out,err}.log`; the first lines name the
store each tool resolved to.

## Conventions

- Only `GET` (and `HEAD`) exist. Anything else is `405`.
- Responses are `application/json; charset=utf-8`, UTF-8, never chunked;
  every response closes the connection.
- Timestamps are RFC 3339 strings. Missing values are `null`; empty lists
  are `[]`. Every documented key is always present.
- **Markdown is returned raw**; the app renders it.
- **Diagrams** come as `{ "name", "text", "ansi" }`: `text` has ANSI escape
  codes stripped (render it monospaced, unwrapped), `ansi` is the raw file
  with its SGR colour codes for clients that can colour it.
- Path segments are validated by each tool's own id type (scout slugs, RCA
  ids, recap ids, `YYYY-MM-DD` dates, `owner/repo` slugs). An invalid id is
  `400`, an unknown one `404`.
- Large text is capped: a diff past 2 MiB is cut and `truncated` is `true`;
  log tails return at most `?lines=` lines (default 200, max 2000).
- Each request is logged on one line to stderr: time, client address,
  method, path (never the query or headers), status, milliseconds.

### Errors

Every non-2xx response has this body:

```json
{ "error": { "code": "not_found", "message": "no scout project `rate-limit-the-api`" } }
```

| Status | `code` | When |
|---|---|---|
| 400 | `bad_request` | Malformed request or invalid id |
| 401 | `unauthorized` | Missing or wrong bearer token |
| 404 | `not_found` | Unknown path or item |
| 405 | `method_not_allowed` | Not `GET`/`HEAD` |
| 413 | `too_large` | Request head over 16 KiB |
| 500 | `internal` | A store failed to read; `message` says which file |

## Server

### `GET /v1/health`

No token. For liveness checks (`curl`, launchd, the app before pairing).

```json
{ "ok": true }
```

### `GET /v1/version`

```json
{ "beagle": "0.3.2", "api": 1, "tools": ["scout", "rca", "recap", "standup", "tandem"] }
```

`api` changes only on a breaking change to this document.

## Scout

Reads the store `beagle scout root` names (`$BEAGLE_SCOUT_ROOT` or `$SCOUT_ROOT`, else the
nearest `.scout-store`, else `root` in `~/.config/beagle/scout.toml`, else
`~/.scout`). The server never runs
scout's auto-pause sweep: a read never changes a task.

### `GET /v1/scout/projects`

Same order and fields as `beagle scout list --json` (open projects first,
then most recently active), plus `goal` and `last_activity`.

```json
{
  "projects": [
    {
      "id": "rate-limit-the-api",
      "title": "Rate-limit the public API",
      "status": "active",
      "goal": "Every public endpoint answers 429 past 100 requests a minute per key.",
      "tasks_closed": 1,
      "tasks_total": 4,
      "working": ["02-add-allowlist"],
      "links": ["ABC-123"],
      "created": "2026-11-12T09:00:00Z",
      "updated": "2026-11-12T10:30:00Z",
      "last_activity": "2026-11-12T10:42:10Z"
    }
  ],
  "warnings": []
}
```

`status`: `planning` | `active` | `paused` | `done` | `dropped`.
`warnings`: files that failed to load (never fatal).

### `GET /v1/scout/projects/{id}`

The project, its `project.md` body, its tasks in order, and the handoff
(the `## Next steps` an agent picking the project up reads first).

```json
{
  "id": "rate-limit-the-api",
  "title": "Rate-limit the public API",
  "status": "active",
  "goal": "Every public endpoint answers 429 past 100 requests a minute per key.",
  "links": ["ABC-123"],
  "created": "2026-11-12T09:00:00Z",
  "updated": "2026-11-12T10:30:00Z",
  "last_activity": "2026-11-12T10:42:10Z",
  "markdown": "## Goal\n\nEvery public endpoint …",
  "tasks": [
    {
      "id": "01-map-request-path",
      "number": 1,
      "slug": "map-request-path",
      "title": "Map the request path",
      "status": "done",
      "checklist": { "done": 3, "total": 3 },
      "links": [],
      "created": "2026-11-12T09:00:00Z",
      "updated": "2026-11-12T09:40:00Z"
    }
  ],
  "handoff": { "task": "01-map-request-path", "markdown": "Next is 02: …" },
  "warnings": []
}
```

`handoff` is `null` when no task has written next steps.

### `GET /v1/scout/projects/{id}/tasks/{task}`

`{task}` is a number (`2`, `02`), a full id (`02-add-allowlist`) or a slug.

```json
{
  "project": "rate-limit-the-api",
  "id": "02-add-allowlist",
  "number": 2,
  "slug": "add-allowlist",
  "title": "Add the allowlist",
  "status": "working",
  "checklist": { "done": 1, "total": 4 },
  "links": ["https://github.com/acme/widgets/pull/42"],
  "created": "2026-11-12T09:00:00Z",
  "updated": "2026-11-12T10:30:00Z",
  "markdown": "## Context\n\n…\n\n## Steps\n\n- [x] …",
  "next_steps": null
}
```

`status`: `todo` | `working` | `paused` | `blocked` | `done` | `skipped`.
`markdown` is the whole body (frontmatter removed). Attached agent sessions
are deliberately not served: they hold raw command output.

### `GET /v1/scout/projects/{id}/log?lines=200`

The tail of the project's `log.md`, oldest first.

```json
{
  "project": "rate-limit-the-api",
  "lines": [
    "- 2026-11-12 10:30 [02-add-allowlist] starting: add the allowlist table"
  ],
  "truncated": true
}
```

## RCA

Reads the store `beagle rca` resolves (config `root`, else the nearest
directory holding `rcas/`), active and archived workspaces alike.

### `GET /v1/rca`

`rcas` is exactly what `beagle rca list --json` prints for active and
archived workspaces (sidebar order: active first, open before closed, then
severity, then newest). `broken` lists directories that failed to load.

```json
{
  "rcas": [
    {
      "id": "2026-05-12-example-checkout-timeouts",
      "title": "Checkout requests time out under load",
      "severity": "high",
      "status": "finished",
      "created": "2026-05-12T14:03:00Z",
      "updated": null,
      "systems": ["checkout-api", "postgres"],
      "tags": [],
      "prs": [],
      "published": false,
      "published_at": null,
      "archived": false
    }
  ],
  "broken": [{ "dir": "half-written", "reason": "rca.toml: missing field `title`" }],
  "warnings": []
}
```

`severity`: `critical` | `high` | `medium` | `low` | `info`.
`status`: `investigating` | `fixing` | `review` | `agent` | `final-review` | `finished`.

### `GET /v1/rca/{id}`

The manifest entry, tickets, which sections exist (in tab order) and the
diagram names.

```json
{
  "rca": { "id": "2026-05-12-example-checkout-timeouts", "title": "…", "…": "same keys as /v1/rca" },
  "tickets": ["https://tracker.example.com/ABC-123"],
  "sections": [
    { "name": "summary", "title": "Summary", "present": true },
    { "name": "timeline", "title": "Timeline", "present": true },
    { "name": "root-cause", "title": "Root Cause", "present": true },
    { "name": "impact", "title": "Impact", "present": true },
    { "name": "remediation", "title": "Fix", "present": true },
    { "name": "commands", "title": "Commands", "present": false },
    { "name": "final-review", "title": "Final Review", "present": true },
    { "name": "notes", "title": "Notes", "present": true },
    { "name": "log", "title": "Log", "present": true }
  ],
  "diagrams": ["01-request-path.txt"]
}
```

### `GET /v1/rca/{id}/sections/{name}`

`{name}` is a section's file stem from the list above. `404` when the file
does not exist yet.

```json
{ "id": "2026-05-12-example-checkout-timeouts", "name": "summary", "title": "Summary", "markdown": "Checkout requests …" }
```

### `GET /v1/rca/{id}/diagrams/{n}`

`{n}` is a 1-based index into `diagrams` or the file name.

```json
{ "name": "01-request-path.txt", "text": "client ──▶ api ──▶ db\n", "ansi": "client \u001b[31m──▶\u001b[0m api ──▶ db\n" }
```

## Recap

Reads the store `beagle recap` resolves (`$BEAGLE_RECAP_ROOT` or `$RECAP_ROOT`, else `root` in
`~/.config/beagle/recap.toml`, else the nearest directory holding
`recaps/`).

### `GET /v1/recap?archived=false`

Exactly the rows `beagle recap list --json` prints (drafts, open, merged,
closed). `?archived=true` lists the archive instead.

```json
{
  "recaps": [
    {
      "id": "widgets-42",
      "repo": "acme/widgets",
      "number": 42,
      "title": "ABC-123: retry webhook deliveries",
      "state": "open",
      "ticket": "ABC-123",
      "created": "2026-11-18T15:20:00Z",
      "url": "https://github.com/acme/widgets/pull/42"
    }
  ],
  "warnings": []
}
```

`state`: `draft` | `open` | `merged` | `closed`.

### `GET /v1/recap/{id}`

The manifest, the write-up and its diagrams; the diff and the comments are
separate calls because they can be large.

```json
{
  "id": "widgets-42",
  "archived": false,
  "manifest": {
    "repo": "acme/widgets",
    "number": 42,
    "title": "ABC-123: retry webhook deliveries",
    "url": "https://github.com/acme/widgets/pull/42",
    "state": "open",
    "author": "octocat",
    "branch": "retry-webhooks",
    "base": "main",
    "ticket": "ABC-123",
    "ticket_url": "https://tracker.example.com/ABC-123",
    "additions": 120,
    "deletions": 14,
    "files": 6,
    "created": "2026-11-18T15:20:00Z",
    "updated": "2026-11-19T09:02:00Z",
    "merged_at": null
  },
  "summary": "## TL;DR\n\n…",
  "diagrams": [{ "name": "01-retry-loop", "text": "…", "ansi": "…" }],
  "warnings": []
}
```

### `GET /v1/recap/{id}/diff`

```json
{ "id": "widgets-42", "diff": "diff --git a/src/hooks.rs b/src/hooks.rs\n…", "truncated": false }
```

### `GET /v1/recap/{id}/comments`

The threads the Comments tab shows (as last synced; the server never calls
GitHub), each with whether you have read it and the agent's review.

```json
{
  "id": "widgets-42",
  "fetched": "2026-11-19T09:05:00Z",
  "threads": [
    {
      "id": "t-901",
      "kind": "inline",
      "path": "src/hooks.rs",
      "line": 88,
      "resolved": false,
      "outdated": false,
      "state": null,
      "place": "src/hooks.rs:88",
      "read": false,
      "comments": [
        {
          "id": "901",
          "author": "octocat",
          "bot": false,
          "context": false,
          "created": "2026-11-19T08:50:00Z",
          "body": "Should this back off?",
          "url": "https://github.com/acme/widgets/pull/42#discussion_r901"
        }
      ],
      "review": { "in_short": "Yes: …", "markdown": "…", "current": true }
    }
  ]
}
```

`kind`: `inline` | `conversation` | `review` (`state` is the review state,
e.g. `APPROVED`, for `review`). `review` is `null` when no agent review
exists; `current` is `false` when people replied after it was written.

## Standup

Reads the store `beagle standup` resolves (`$BEAGLE_STANDUP_ROOT` or `$STANDUP_ROOT`, else `root` in
`~/.config/beagle/standup.toml`, else the nearest directory holding
`standups/`).

### `GET /v1/standup`

Exactly the rows `beagle standup list --json` prints, newest first.

```json
{
  "reports": [
    { "date": "2026-11-20", "title": "Webhook retries land", "since": "2026-11-19", "tickets": 3 }
  ]
}
```

### `GET /v1/standup/{date}`

`{date}` is `YYYY-MM-DD` or `latest`.

```json
{
  "date": "2026-11-20",
  "weekday": "Friday",
  "title": "Webhook retries land",
  "since": "2026-11-19",
  "created": "2026-11-20T11:00:00-05:00",
  "overview": "## Big picture\n\n…",
  "diagrams": [{ "name": "01-flow", "text": "…", "ansi": "…" }],
  "tickets": [
    {
      "id": "ABC-123",
      "title": "Retry webhook deliveries",
      "url": "https://tracker.example.com/ABC-123",
      "state": "In Review",
      "state_kind": "review",
      "systems": ["webhooks"],
      "prs": ["https://github.com/acme/widgets/pull/42"],
      "summary": "## What happened\n\n…",
      "diagrams": []
    }
  ],
  "warnings": []
}
```

Tickets come most active first, as in the TUI. `state_kind`: `progress` |
`review` | `blocked` | `done` | `todo` | `other`.

## Tandem

Reads the review TUI's store (`beagle tandem paths` → `review_tui.store`).
Posting comments and approving stay on the Mac.

### `GET /v1/tandem/queue`

The rows `beagle tandem list` prints, in the same order (ready first, newest
request first within a stage).

```json
{
  "prs": [
    {
      "repo": "acme/widgets",
      "number": 42,
      "title": "ABC-123: retry webhook deliveries",
      "stage": "ready",
      "verdict": "mergeable-with-suggestions",
      "open_comments": 2,
      "requested_at": "2026-11-19T08:00:00Z",
      "started_at": "2026-11-19T08:01:00Z",
      "approved": false,
      "error": null
    }
  ]
}
```

`stage`: `ready` | `reviewing` | `queued` | `failed` | `done` | `merged` |
`closed`. `verdict`: `mergeable` | `mergeable-with-suggestions` |
`mergeable-with-unknowns` | `not-mergeable` | `null`. An absent store is an
empty queue.

### `GET /v1/tandem/prs/{owner}/{repo}/{number}`

The agent's summary and its local review comments (roots first, replies
nested), as the TUI's Summary and Comments tabs show them.

```json
{
  "repo": "acme/widgets",
  "number": 42,
  "title": "ABC-123: retry webhook deliveries",
  "url": "https://github.com/acme/widgets/pull/42",
  "state": "open",
  "author": "octocat",
  "verdict": "mergeable-with-suggestions",
  "summary": {
    "markdown": "**Mergeable with suggestions** — …",
    "agent": "claude",
    "head_sha": "4be1c0d",
    "updated_at": "2026-11-19T08:20:00Z"
  },
  "comments": [
    {
      "id": "6f1e…",
      "path": "src/hooks.rs",
      "line": 88,
      "end_line": null,
      "severity": "issue",
      "status": "open",
      "posted": false,
      "author": "claude",
      "tag": "correctness",
      "body": "This retries without a cap …",
      "suggestion": null,
      "created_at": "2026-11-19T08:19:00Z",
      "replies": []
    }
  ]
}
```

`summary` is `null` before a review has run. `title`, `url`, `state` and
`author` are `null` until the PR's details have been fetched. `severity`:
`info` | `suggestion` | `issue` | `blocker`. `status`: `open` | `accepted` |
`rejected` | `resolved` | `archived`. Replies have the same keys (their own
`replies` is always `[]`). A PR with no store yet is `404`.
