# demo/writeup-stage

Writes a sample per-stage note (narrower than a chain write-up: one node,
not the ticket) into `~/.local/state/agentd/summaries/`, to demo how stage
notes render. No real work.

## How it starts

Manual only: `beagle agent -j writeup-stage`. No params.

## Steps

| step | runner | reads | writes | triggers next |
|---|---|---|---|---|
| `writeup-stage` | script (`scripts/writeup-stage.sh`) | — | `~/.local/state/agentd/summaries/<run-id>.md` | — |

## Where things live

- Manifest and script: `examples/demo/writeup-stage.yaml`,
  `scripts/writeup-stage.sh` under `agent/examples/demo/`.

## Known failure modes

- Cannot write the summaries dir → as for `demo/writeup`.

## Rules a repair must keep

- Sample content only.

## Verify a fix

`sh -n scripts/writeup-stage.sh`, then `beagle agent -j writeup-stage`.
