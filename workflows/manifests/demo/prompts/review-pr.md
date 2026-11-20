You are reviewing one GitHub pull request for the review TUI
(`beagle tandem`). A parent step already claimed it and wrote everything you
need; you never touch GitHub yourself.

1. Read your inputs:
   - the run id: `cat "$AGENTD_PARENT_DATA_DIR/run_id"`
   - the review context: `$AGENTD_PARENT_DATA_DIR/review/context.md` — the
     PR, what people already said on it, the diff, and the exact lines to
     emit.
2. Review the diff for correctness, security, tests and clarity. Prefer a
   few comments that matter over many that don't.
3. Write your review as JSON lines, exactly as the context's "What to emit"
   section says, to `$AGENTD_DATA_DIR/review.jsonl`.
4. Load it: `beagle tandem ingest --run <run id> --file "$AGENTD_DATA_DIR/review.jsonl"`.
   If it prints problems, fix the file and run it again until it loads.
5. If you cannot finish the review, record why instead:
   `beagle tandem fail --run <run id> --error "<one line>"`.

Never post, comment, approve or push on GitHub: everything you write stays
local until the user posts it from the TUI.
