//! Continuing a thread: the context a reply run needs, and the prompt
//! built from it. Split out of context.rs, which was over the 400-line
//! rule once replies learned to carry the rest of the conversation.

use std::fmt::Write as _;
use tandem_core::github::PullRequest;

use crate::context::ThreadMessage;

/// Everything a reply run needs to continue a comment thread.
#[derive(Debug, Clone)]
pub struct ReplyContext {
    /// Run id the agent quotes back.
    pub run_id: String,
    /// JSONL drop-box file for the agent's reply.
    pub comments_file: String,
    /// Root comment id — the agent's reply must carry this as `parent_id`.
    pub parent_id: String,
    /// File the thread is anchored to; empty for PR-level threads.
    pub path: String,
    /// Diff side the anchor line counts on (`"old"` / `"new"`).
    pub side: String,
    /// Anchor line; 0 for PR-level threads.
    pub line: u64,
    /// Diff excerpt for the file under discussion.
    pub diff_text: String,
    /// Every other thread on this PR, rendered by
    /// [`crate::conversation`]. Tandem has them cached; an agent that
    /// can't see them ends up guessing at work already discussed.
    pub conversation: String,
    /// The PR's own description.
    pub pr_body: String,
}

/// Prompt for continuing a discussion on an existing local comment.
pub fn build_reply_prompt(
    ctx: &ReplyContext,
    pr: &PullRequest,
    thread: &[ThreadMessage],
    instructions: &str,
) -> String {
    let mut rendered_thread = String::new();
    for msg in thread {
        let _ = write!(rendered_thread, "### {}\n{}\n\n", msg.author, msg.body);
    }
    format!(
        "# Tandem review thread\n\
         You are running inside Tandem, a local code-review app, continuing \
         a discussion about one review comment. Your reply stays LOCAL — \
         never attempt to post to GitHub or call `gh`.\n\n\
         - run id: {run_id}\n\
         - repository: {repo}\n\
         - pull request: #{number} — {title}\n\
         - comment anchor: {path}:{line} (side: {side})\n\n\
         ## Pull request description\n\
         {pr_body}\n\n\
         ## The thread so far (oldest first)\n\
         {thread}\
         {conversation}\
         ## How to reply\n\
         Respond to the latest message. Emit EXACTLY ONE JSON object on \
         its own line in your final response (no code fences):\n\n\
         {{\"type\":\"tandem_comment\",\"path\":\"{path}\",\"side\":\"{side}\",\
         \"line\":{line},\"end_line\":null,\"severity\":\"info\",\
         \"parent_id\":\"{parent_id}\",\"body\":\"<your reply, markdown>\",\
         \"suggestion\":null}}\n\n\
         When the reviewer asks for a concrete code change, set \
         `suggestion` to the COMPLETE replacement for lines \
         line..=end_line of {path} (new side; preserve indentation; no \
         code fences; escape newlines as \\n). Re-anchor `line` and \
         `end_line` to the EXACT range your fix replaces — e.g. a whole \
         block, not just the thread's anchor line. To DELETE lines \
         outright, set `suggestion` to the empty string \"\" with \
         line/end_line covering the lines to remove. The reviewer gets a \
         one-click 'commit suggestion' button that commits your \
         suggestion verbatim to the PR branch — so prefer a committable \
         suggestion over prose instructions whenever the fix is exact.\n\n\
         The reviewer can edit any comment here, including yours, and \
         a message marked \"(edited by the reviewer)\" shows their \
         wording, not the original — treat it as what they meant and \
         don't restate the version you wrote. Tandem keeps the earlier \
         versions, so an edit is a correction, not a loss.\n\n\
         The `body` MUST be well-formatted GitHub-flavored markdown — \
         code spans, fenced code blocks, tables and lists where they help \
         (escape newlines as \\n inside the JSON string). Keep the reply \
         focused and conversational — you are talking with the reviewer. \
         Acknowledge if they are right; push back with evidence if not.\n\n\
         ## Original review instructions\n\
         {instructions}\n\n\
         ## Diff context for {path}\n\
         ```diff\n{diff}\n```\n",
        run_id = ctx.run_id,
        repo = pr.repo.slug(),
        number = pr.number,
        title = pr.title,
        path = ctx.path,
        side = ctx.side,
        line = ctx.line,
        parent_id = ctx.parent_id,
        thread = rendered_thread,
        conversation = ctx.conversation,
        pr_body = if ctx.pr_body.trim().is_empty() {
            "(none)"
        } else {
            ctx.pr_body.trim()
        },
        instructions = instructions,
        diff = ctx.diff_text,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::context::test_pr::fixture_pr;

    #[test]
    fn reply_prompt_teaches_suggestions() {
        let ctx = ReplyContext {
            run_id: "run-2".into(),
            comments_file: "/tmp/run-2/comments.jsonl".into(),
            parent_id: "root-1".into(),
            path: "src/x.rs".into(),
            side: "new".into(),
            line: 56,
            diff_text: "+ hi".into(),
            conversation: crate::conversation::render(&[crate::conversation::ConversationThread {
                anchor: "src/other.rs:3 (github)".into(),
                messages: vec![ThreadMessage {
                    author: "matt (github)".into(),
                    body: "already fixed in 3823133".into(),
                }],
            }]),
            pr_body: "removes the edge cluster".into(),
        };
        let thread = vec![ThreadMessage {
            author: "you".into(),
            body: "remove this line please".into(),
        }];
        let prompt = build_reply_prompt(&ctx, &fixture_pr(), &thread, "be terse");
        assert!(prompt.contains("\"suggestion\":null"));
        assert!(prompt.contains("end_line"));
        assert!(prompt.contains("DELETE lines"));
        assert!(prompt.contains("parent_id"));
        // The agent can see the PR and its other threads, not just this
        // one — it was answering "I can't see those" without them.
        assert!(prompt.contains("removes the edge cluster"));
        assert!(prompt.contains("already fixed in 3823133"));
        assert!(prompt.contains("src/other.rs:3 (github)"));
    }
}
