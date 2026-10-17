#!/bin/sh
# Demo DAG root: a deterministic node. Gathers a finding, writes it to this
# run's data dir, then fans out to TWO LLM children in parallel — one claude,
# one codex — linked to this run via --parent so they inherit the data dir
# through $AGENTD_PARENT_DATA_DIR.
set -eu

echo "fanout: gathering disk usage finding"
df -h / | tail -1 > "$AGENTD_DATA_DIR/finding.txt"
echo "fanout: wrote $AGENTD_DATA_DIR/finding.txt:"
cat "$AGENTD_DATA_DIR/finding.txt"

# Branching logic lives HERE, in plain code — an if/threshold/dedup could
# gate these triggers. The demo always fires both, in parallel.
echo "fanout: triggering demo/report-claude and demo/report-codex"
agentctl run demo/report-claude --parent "$AGENTD_RUN_ID"
agentctl run demo/report-codex  --parent "$AGENTD_RUN_ID"
echo "fanout: done — children are queued"
