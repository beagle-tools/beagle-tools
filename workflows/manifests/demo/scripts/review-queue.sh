#!/bin/sh
# Review-queue root: claims the oldest PR queued with `beagle tandem add` and
# starts one reviewer for it. `claim` fetches the PR into the review TUI's
# store and writes the reviewer's context; it prints `<run id> <context path>`,
# or nothing when the queue is empty.
set -eu

claimed=$(beagle tandem claim --out "$AGENTD_DATA_DIR/review")
if [ -z "$claimed" ]; then
    echo "review-queue: nothing queued"
    exit 0
fi

run_id=${claimed%% *}
printf '%s\n' "$run_id" > "$AGENTD_DATA_DIR/run_id"
echo "review-queue: claimed run $run_id (${claimed#* })"

if ! agentctl run demo/review-pr --parent "$AGENTD_RUN_ID"; then
    beagle tandem fail --run "$run_id" --error "could not start the reviewer"
    exit 1
fi
echo "review-queue: reviewer queued"
