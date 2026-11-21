# demo/writeup

Writes a sample chain write-up into `~/.local/state/agentd/summaries/` so the
summary viewers (agenttop, Agentdesk) have something to render. No real
change sits behind it.

## How it starts

Manual only: `beagle agent -j writeup`. No params.

## Steps

| step | runner | reads | writes | triggers next |
|---|---|---|---|---|
| `writeup` | script (`scripts/writeup.sh`) | — | `~/.local/state/agentd/summaries/<run-id>.md` | — |

## Where things live

- Manifest and script: `manifests/demo/writeup.yaml`, `scripts/writeup.sh`
  under `workflows/manifests/demo/`.
- Output: `~/.local/state/agentd/summaries/` (pruned by agentd's retention).

## Known failure modes

- Cannot write the summaries dir → permissions or disk; **Human-only** if
  the disk is full, otherwise **Auto-fixable** (`mkdir -p` is already there).

## Rules a repair must keep

- Sample content only; never write anything that looks like a real chain's
  result.

## Verify a fix

`sh -n scripts/writeup.sh`, then `beagle agent -j writeup` and open the summary.
