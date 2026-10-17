#!/bin/sh
# Demo chain stage. Writes a per-stage write-up keyed to ITS OWN run id, so
# the overlay on this run lists two links: the chain handover from the root,
# listed first, and this narrower one.
set -eu

summaries="$HOME/.local/state/agentd/summaries"
mkdir -p "$summaries"

cat > "$summaries/$AGENTD_RUN_ID.md" <<'MD'
# Demo stage — what this one node did

A per-stage note. Narrower than the chain handover by design: it covers one
node, and says nothing about the ticket as a whole.

## Did

Read the parent's data dir, confirmed the batched lookup returns the same
label set as the loop it replaced, on 2,000 sampled queries.

## Found

Two queries disagreed. Both turned out to be **retired** labels that the old
loop resolved through an alias table the batch query does not join.

## Left for the chain

The alias gap is written up as an open question in the chain handover rather
than fixed here — it is a second bug, not a regression in this change.
MD

echo "writeup-stage: wrote $summaries/$AGENTD_RUN_ID.md"
