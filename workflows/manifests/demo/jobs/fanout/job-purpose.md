# demo/fanout

Shows agentd's DAG in its smallest form: one deterministic script step hands
a file to two model steps that run in parallel. No real work — it exists to
exercise and demo the orchestrator (fan-out, `--parent`, handover through
data dirs, mixed runners).

## How it starts

Manual only: `agentctl -j fanout`. No params.

## Steps

| step | runner | reads | writes | triggers next |
|---|---|---|---|---|
| `fanout` | script (`scripts/fanout.sh`) | `df -h /` | `$AGENTD_DATA_DIR/finding.txt` | `report-claude`, `report-codex` |
| `report-claude` | claude (`prompts/report.md`, Bash only) | parent's `finding.txt` | one sentence, stdout | — |
| `report-codex` | codex (same prompt) | parent's `finding.txt` | one sentence, stdout | — |

## Where things live

- Manifests: `manifests/demo/*.yaml` under `workflows/`, linked at
  `~/.config/agentd/manifests/demo`.
- Run data: `~/.local/state/agentd/data/<run-id>/` (kept a week).
- Logs: `agentctl logs <run-id>`.

## Known failure modes

- `report-codex` fails with codex not installed or not logged in →
  **Human-only** (`codex login`).
- `report-claude` refused a tool → its `allowed_tools` is `[Bash]` and the
  prompt only needs `cat`; **Auto-fixable** if the prompt drifted.

## Rules a repair must keep

- It stays a demo: no network, no writes outside its own data dir.
- The `demo` project is hidden in the job views by `~/.config/agentd/ui.yaml`.

## Verify a fix

`sh -n scripts/fanout.sh`, then `agentctl -j fanout` and check both children
succeed (`agentctl get runs | grep report-`).
