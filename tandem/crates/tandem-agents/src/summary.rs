//! The summary half of the agent contract, kept beside the review
//! context rather than inside it — context.rs was over the 400-line
//! rule and this is a self-contained piece of prompt.

use crate::context::ReviewContext;

/// The summary half of the contract, omitted entirely when the agent
/// has summaries turned off. On a re-review the current text is handed
/// over so the agent revises it — keeping what still holds, dropping
/// what the new commits fixed — rather than writing a fresh one that
/// loses earlier context.
pub(crate) fn summary_contract(ctx: &ReviewContext) -> String {
    if !ctx.wants_summary {
        return String::new();
    }
    let mut out = String::from(
        "## How to write the PR summary\n\
         As well as the comments, emit exactly ONE summary line — same \
         rules (own line, no code fences, in your final response text or \
         the comments file):\n\n\
         {\"type\":\"tandem_summary\",\"body\":\"<markdown>\"}\n\n\
         It is the reviewer's overview of the whole PR: what it does, how \
         sound it looks, and the themes behind your comments — not a \
         restatement of each one. Keep it short: a few sentences or a \
         handful of bullets. Link to the code you are discussing as \
         [src/x.rs:42](tandem://src/x.rs#L42) — the reviewer clicks it \
         and lands on that line of the diff, so prefer a link over \
         describing where something lives.\n\n",
    );
    if let Some(existing) = &ctx.existing_summary {
        out.push_str(
            "### The summary already on this PR\n\
             A previous run wrote the text below. Revise it rather than \
             replacing it wholesale: keep what still holds, update what \
             the new commits changed, drop what is now fixed, and add \
             what you found this time. Emit the full revised text as your \
             summary line.\n\n",
        );
        out.push_str(existing);
        out.push_str("\n\n");
    }
    out
}
