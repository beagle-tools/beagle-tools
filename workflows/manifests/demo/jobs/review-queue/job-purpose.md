# demo/review-queue

The reviewer behind Tandem's review TUI (`beagle tandem`): takes the oldest
PR queued with `beagle tandem add` (or `a` in the TUI), has an agent review
it, and loads the review into the TUI's store as local comments, a summary
and a verdict. Nothing is posted to GitHub; the user does that from the TUI.

## How it starts

Manual: `agentctl -j review-queue`, no params. The TUI starts it right after
a PR is queued when `~/.config/beagle/tandem.toml` has
`kick = ["agentctl", "run", "demo/review-queue"]`. Add a `schedule` to also drain
the queue on a clock.

## Steps

| step | runner | reads | writes | triggers next |
|---|---|---|---|---|
| `review-queue` | script (`scripts/review-queue.sh`) | the TUI's queue, the PR on GitHub (`beagle tandem claim`) | `$AGENTD_DATA_DIR/run_id`, `$AGENTD_DATA_DIR/review/context.md` | `review-pr` (only when a PR was claimed) |
| `review-pr` | claude (`prompts/review-pr.md`; Bash, Read, Write) | parent's `run_id` and `review/context.md` | `$AGENTD_DATA_DIR/review.jsonl`, loaded with `beagle tandem ingest` | — |

## Where things live

- Manifests: `manifests/demo/review-queue.yaml`, `review-pr.yaml`,
  `scripts/review-queue.sh`, `prompts/review-pr.md` under `workflows/`.
- The queue and reviews: the review TUI's store (`beagle tandem paths`,
  `review_tui.store`).
- Run data: `~/.local/state/agentd/data/<run-id>/` (kept a week).

## Known failure modes

- `claim` fails with a GitHub auth error → `gh` is not logged in, or the
  config's `github_user` is not a logged-in `gh` account; **Human-only**
  (`gh auth login`).
- `claim` prints `could not fetch …` → the PR is gone or unreadable; the job
  is failed and shown in the TUI, the next one is tried. Nothing to fix.
- `ingest` keeps refusing the review → the agent did not follow the
  context's contract; **Auto-fixable** in `prompts/review-pr.md`.
- `beagle` not found → it is not on the PATH agentd runs with;
  **Human-only**.

## Rules a repair must keep

- No step writes to GitHub. Posting and approving stay explicit keypresses
  in the TUI.
- The output contract is the context `claim` writes; change it only together
  with `ingest` in `tandem/crates/tandem-tui`.

## Verify a fix

`sh -n scripts/review-queue.sh`, then `beagle tandem add acme/widgets#1
--no-kick` with a PR you can read, `agentctl -j review-queue`, and check the
PR turns `ready` in `beagle tandem list`.
