#!/bin/sh
# Demo chain root. Writes the CHAIN handover — the document keyed to the root
# run id, which is what another agent would read instead of re-running the
# work — then triggers one child stage.
#
# Deliberately has no PR link: retention therefore sweeps this on the ordinary
# 24h clock and the demo cleans up after itself. Add a line with a real PR URL
# and the same file survives until 24h after that PR merges.
set -eu

summaries="$HOME/.local/state/agentd/summaries"
mkdir -p "$summaries"

echo "writeup: composing the chain handover for $AGENTD_RUN_ID"
cat > "$summaries/$AGENTD_RUN_ID.md" <<'MD'
# Demo — cache warming for the ticket search endpoint

This is a **sample** write-up with no real change behind it. It exists to show
the shape the viewer renders: what another agent reads to pick work up.

## What was asked

Ticket asked why `/api/tickets/search` p99 sat near 4s while p50 was 40ms.

## What happened

The slow path was not the query. It was a cold cache on the **label** lookup:
every search resolved label ids one row at a time, and a cold process paid
that cost on the first request after each deploy.

## The change

- `search/labels.go` — batch the id resolution into one `IN (...)` query
- `search/cache.go` — warm the label map on boot instead of lazily
- `search/labels_test.go` — a test that fails on the N+1, not on the timing

## Why this approach

Adding a TTL cache in front of the slow call was the obvious fix and the wrong
one: it hides a cold-start cliff behind an average. Warming on boot means the
first request after a deploy is as fast as the thousandth, which is the
property the ticket actually wanted.

## Shape of it

```
  request ─┬─> parse query ──> 40ms  (never the problem)
           │
           └─> resolve labels
                 before:  N lookups, 1 per label   ← the 4s
                 after:   1 batched lookup
                          + warmed at boot
```

## Open questions

- The warm adds ~300ms to boot. Fine for a rolling deploy, maybe not for a
  scale-from-zero worker — nobody has measured that case.
- `label_aliases` is still resolved lazily. Same bug, smaller blast radius.

## Picking this up

1. Branch `demo/label-batching`, worktree at `~/.local/state/agentd/worktrees/`.
2. `go test ./search/...` passes; the benchmark is **not** wired into CI yet.
3. Next: decide the scale-from-zero question above, then wire the benchmark.
4. Do **not** re-run the investigation — the N+1 is confirmed, not suspected.
MD

echo "writeup: wrote $summaries/$AGENTD_RUN_ID.md"

echo "writeup: triggering the child stage"
agentctl run demo/writeup-stage --parent "$AGENTD_RUN_ID"
echo "writeup: done"
