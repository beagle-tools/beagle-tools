//! Build the run-context block prepended to every agent prompt.
//!
//! The agent must know it is inside Tandem's review flow (like hunk's CLI
//! contract): it reviews a diff and emits local comments — it must NOT
//! try to post anything to GitHub. Values are interpolated literally
//! (agentd learned that scoped tool allowlists reject `$VAR`).

use std::fmt::Write as _;
use tandem_core::github::PullRequest;

use crate::summary::summary_contract;

/// Everything the agent needs to know about where it is running.
#[derive(Debug, Clone)]
pub struct ReviewContext {
    /// Run id the agent quotes back; also names the run directory.
    pub run_id: String,
    /// Absolute path of the JSONL drop-box file the agent may append
    /// `tandem_comment` lines to (an alternative to printing them on stdout).
    pub comments_file: String,
    /// Unified diff of the whole PR, embedded verbatim in the prompt.
    pub diff_text: String,
    /// Whether this agent writes the PR-level summary (spec opt-out).
    pub wants_summary: bool,
    /// The summary already on this PR, if any — a re-review revises it
    /// instead of starting from scratch.
    pub existing_summary: Option<String>,
    /// The repo's own conventions, rendered by [`crate::conventions`];
    /// empty when the checkout states none.
    pub conventions: String,
}

/// Assemble the full prompt fed to the runner on stdin:
/// context block + user's review instructions + the diff itself.
pub fn build_prompt(ctx: &ReviewContext, pr: &PullRequest, instructions: &str) -> String {
    format!(
        "# Tandem review context\n\
         You are running inside Tandem, a local code-review app. You are \
         reviewing a GitHub pull request, but your comments stay LOCAL — \
         never attempt to post to GitHub or call `gh`.\n\n\
         - run id: {run_id}\n\
         - repository: {repo}\n\
         - pull request: #{number} — {title}\n\
         - branch: {head} -> {base}\n\
         - head sha: {sha}\n\n\
         ## How to emit review comments\n\
         Write one JSON object per comment, each on its own line, in your \
         FINAL response text (no tools needed — this is the preferred \
         path). Alternatively, append the same lines to the file \
         `{comments_file}`.\n\n\
         {{\"type\":\"tandem_comment\",\"path\":\"<file>\",\"side\":\"new|old\",\
         \"line\":<n>,\"end_line\":<n|null>,\
         \"severity\":\"info|suggestion|issue|blocker\",\
         \"tag\":\"<kind>\",\
         \"body\":\"<markdown>\",\"suggestion\":<null or \"replacement code\">}}\n\n\
         `tag` says what kind of remark it is, in one or two words: \
         \"coding-standards\" when it holds the code to a rule this \
         repo states, else something plain like \"correctness\", \
         \"tests\", \"performance\", \"security\", \"naming\" or \
         \"docs\". Omit it when nothing fits.\n\n\
         Rules: `line` is the line number in the new file (or old file \
         when side=old) and must be a line that appears in the diff below. \
         When you can propose an exact fix, set `suggestion` to the \
         COMPLETE replacement for lines line..=end_line on the new side \
         (end_line defaults to line; preserve indentation; no code \
         fences) — the reviewer gets a one-click 'commit suggestion' \
         button for it. To delete lines outright, set `suggestion` to \
         the empty string \"\". Use null when no concrete fix applies. \
         Do NOT wrap the JSON in code fences. The `body` MUST be \
         well-formatted GitHub-flavored markdown: backticked code spans \
         for identifiers, fenced code blocks for multi-line code, tables \
         or lists where they aid clarity (escape newlines as \\n inside \
         the JSON string). Keep bodies concise and actionable. The \
         comments ARE the review — do not write a summary paragraph \
         instead of comments.\n\n\
         {summary_contract}\
         {conventions}\
         ## Review instructions\n\
         {instructions}\n\n\
         ## Pull request description\n\
         {body}\n\n\
         ## Diff\n\
         ```diff\n{diff}\n```\n",
        run_id = ctx.run_id,
        repo = pr.repo.slug(),
        number = pr.number,
        title = pr.title,
        head = pr.head_ref,
        base = pr.base_ref,
        sha = pr.head_sha,
        comments_file = ctx.comments_file,
        summary_contract = summary_contract(ctx),
        conventions = ctx.conventions,
        instructions = instructions,
        body = pr.body,
        diff = ctx.diff_text,
    )
}

/// One message of a review-comment thread, oldest first.
#[derive(Debug, Clone)]
pub struct ThreadMessage {
    /// "you" for the human, otherwise the agent name.
    pub author: String,
    /// Message text (markdown).
    pub body: String,
}

/// Everything a resolution run needs: the merge is already started in
/// the agent's working directory, so this is mostly instructions.
#[derive(Debug, Clone)]
pub struct ResolveContext {
    /// Run id the agent quotes back.
    pub run_id: String,
    /// JSONL drop-box file for the agent's single `tandem_comment` report.
    pub comments_file: String,
    /// Base branch the merge is pulling in.
    pub base_ref: String,
    /// Paths git left conflict markers in.
    pub conflicted: Vec<String>,
    /// The briefing a previous analysis run wrote, if there was one.
    pub briefing: Option<String>,
}

/// Prompt for actually resolving the conflicts, as opposed to writing
/// about them. The decisive instruction is the last one: resolve what
/// you can judge, and only ask about what you genuinely can't.
pub fn build_resolve_prompt(ctx: &ResolveContext, pr: &PullRequest, instructions: &str) -> String {
    let files = ctx
        .conflicted
        .iter()
        .map(|p| format!("- {p}"))
        .collect::<Vec<_>>()
        .join("\n");
    let briefing = match &ctx.briefing {
        Some(text) => format!("## The analysis already written for this conflict\n{text}\n\n"),
        None => String::new(),
    };
    format!(
        "# Tandem conflict resolution\n\
         PR #{number} ({title}) is being merged with `{base}`. The merge is \
         ALREADY STARTED in your working directory: the base commits are \
         fetched and git has left conflict markers in the files below. Your \
         job is to edit those files until they are right.\n\n\
         - run id: {run_id}\n\
         - conflicted files:\n{files}\n\n\
         {briefing}\
         ## How to work\n\
         Edit the files in place. Remove every conflict marker you resolve \
         (`<<<<<<<`, `=======`, `>>>>>>>`). Do NOT run `git commit`, `git \
         push`, `git merge --abort`, or `gh` — Tandem commits and pushes \
         after a human has looked at what you did. Do not touch files that \
         are not conflicted.\n\n\
         Resolve what you can judge. Most conflicts are two additions at \
         the same anchor and the answer is to keep both, in whichever \
         order the surrounding code implies — do that without asking. \
         Preserve both sides' comments. If the two sides genuinely \
         disagree about behavior and picking wrong would break something, \
         leave THAT hunk's markers in place and ask about it.\n\n\
         ## What to report\n\
         Emit EXACTLY ONE JSON object on its own line in your final \
         response (no code fences):\n\n\
         {{\"type\":\"tandem_comment\",\"path\":\"\",\"side\":\"new\",\"line\":0,\
         \"severity\":\"info\",\"tag\":\"conflict\",\"body\":\"<markdown>\"}}\n\n\
         Keep `path` empty and `line` 0. The body is SHORT: a line per file \
         saying what you did and why, then — only if you left something \
         unresolved — a numbered question per undecided hunk, each with the \
         options and your recommendation. No preamble, no restating the \
         diff. If you resolved everything, say so in one line and stop.\n\n\
         ## The reviewer's standing instructions\n\
         {instructions}\n",
        number = pr.number,
        title = pr.title,
        base = ctx.base_ref,
        run_id = ctx.run_id,
        files = files,
        briefing = briefing,
        instructions = instructions,
    )
}

/// One likely-conflicting file: the PR touches it AND the base branch
/// changed it since the merge-base.
#[derive(Debug, Clone)]
pub struct ConflictFile {
    /// Repo-relative path of the file.
    pub path: String,
    /// Patch of what the BASE branch did to this file since branching.
    pub base_patch: Option<String>,
}

/// Everything a conflict-analysis run needs: which files are likely to
/// conflict and what the base branch did to each.
#[derive(Debug, Clone)]
pub struct ConflictContext {
    /// Run id the agent quotes back.
    pub run_id: String,
    /// JSONL drop-box file for the agent's `tandem_comment` output.
    pub comments_file: String,
    /// Likely-conflicting files, in PR order.
    pub files: Vec<ConflictFile>,
    /// The PR's own diff.
    pub diff_text: String,
}

/// Prompt for analyzing merge conflicts and opening a local discussion.
pub fn build_conflict_prompt(ctx: &ConflictContext, pr: &PullRequest) -> String {
    let mut files_block = String::new();
    for file in &ctx.files {
        let _ = writeln!(files_block, "### {}", file.path);
        match &file.base_patch {
            Some(patch) => {
                files_block.push_str("Base-branch changes since branching:\n```diff\n");
                files_block.push_str(patch);
                files_block.push_str("\n```\n\n");
            }
            None => files_block.push_str("(base patch unavailable — likely binary or huge)\n\n"),
        }
    }
    format!(
        "# Tandem merge-conflict analysis\n\
         You are running inside Tandem, a local code-review app. PR #{number} \
         ({title}) has MERGE CONFLICTS with `{base}`. Below are the files both \
         sides touched: the base branch's changes since branching, and the \
         PR's own diff. Everything stays LOCAL — never call `gh` or git push.\n\n\
         ## Your task\n\
         Emit EXACTLY ONE JSON object on its own line in your final response \
         (no code fences):\n\n\
         {{\"type\":\"tandem_comment\",\"path\":\"\",\"side\":\"new\",\"line\":0,\"severity\":\"info\",\"body\":\"<markdown>\"}}\n\n\
         Keep \"path\" as an EMPTY string and \"line\" 0 exactly as shown — \
         that makes it a PR-level comment; do NOT point it at a file.\n\n\
         The body is a conflict-resolution briefing in GitHub-flavored \
         markdown: for EACH conflicting file, summarize what the PR changes \
         vs what base changed, propose a concrete resolution (show merged \
         code in fenced blocks where helpful), and end with any questions \
         you need answered. The reviewer will reply in this thread — treat \
         it as the start of a conversation.\n\n\
         - run id: {run_id}\n\
         - branch: {head} -> {base}\n\n\
         ## Likely-conflicting files\n\
         {files}\n\
         ## The PR's diff\n\
         ```diff\n{diff}\n```\n",
        number = pr.number,
        title = pr.title,
        base = pr.base_ref,
        head = pr.head_ref,
        run_id = ctx.run_id,
        files = files_block,
        diff = ctx.diff_text,
    )
}

/// A PR fixture shared by this module's tests and reply.rs's.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_pr {
    use chrono::Utc;
    use tandem_core::github::{PrState, PullRequest, RepoRef, User};

    pub(crate) fn fixture_pr() -> PullRequest {
        PullRequest {
            repo: RepoRef::parse("o/r").unwrap(),
            number: 42,
            title: "Add thing".into(),
            body: "does thing".into(),
            state: PrState::Open,
            draft: false,
            author: User {
                login: "matt".into(),
                avatar_url: None,
            },
            head_ref: "feat".into(),
            head_sha: "abc".into(),
            base_ref: "main".into(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            labels: vec![],
            mergeable_state: None,
            requested_reviewers: vec![],
            node_id: None,
            review_decision: None,
            checks_state: None,
            unresolved_threads: 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::test_pr::fixture_pr;
    use super::*;

    #[test]
    fn prompt_contains_contract_and_diff() {
        let pr = fixture_pr();
        let ctx = ReviewContext {
            run_id: "run-1".into(),
            comments_file: "/tmp/run-1/comments.jsonl".into(),
            diff_text: "+ hello".into(),
            wants_summary: true,
            existing_summary: None,
            conventions: String::new(),
        };
        let prompt = build_prompt(&ctx, &pr, "focus on correctness");
        assert!(prompt.contains("tandem_comment"));
        assert!(prompt.contains("o/r"));
        assert!(prompt.contains("#42"));
        assert!(prompt.contains("focus on correctness"));
        assert!(prompt.contains("+ hello"));
        assert!(prompt.contains("never attempt to post to GitHub"));
        // The tag field is part of the contract the agent is shown.
        assert!(prompt.contains("coding-standards"));
        // No conventions in this fixture, so no empty heading for them.
        assert!(!prompt.contains("This repo's conventions"));
    }

    #[test]
    fn conventions_land_above_the_review_instructions() {
        let pr = fixture_pr();
        let ctx = ReviewContext {
            run_id: "run-1".into(),
            comments_file: "/tmp/c.jsonl".into(),
            diff_text: String::new(),
            wants_summary: false,
            existing_summary: None,
            conventions: crate::conventions::block(&[crate::conventions::ConventionDoc {
                path: "AGENTS.md".into(),
                text: "no unwrap outside tests".into(),
                truncated: false,
            }]),
        };
        let prompt = build_prompt(&ctx, &pr, "be thorough");
        let rules = prompt.find("no unwrap outside tests").unwrap();
        let instructions = prompt.find("be thorough").unwrap();
        assert!(
            rules < instructions,
            "conventions must precede instructions"
        );
    }

    #[test]
    fn summary_contract_is_opt_out_and_carries_the_existing_text() {
        let pr = fixture_pr();
        let mut ctx = ReviewContext {
            run_id: "run-1".into(),
            comments_file: "/tmp/run-1/comments.jsonl".into(),
            diff_text: "+ hello".into(),
            wants_summary: false,
            existing_summary: None,
            conventions: String::new(),
        };
        let off = build_prompt(&ctx, &pr, "go");
        assert!(!off.contains("tandem_summary"));

        ctx.wants_summary = true;
        let on = build_prompt(&ctx, &pr, "go");
        assert!(on.contains("tandem_summary"));
        assert!(!on.contains("already on this PR"));

        ctx.existing_summary = Some("the story so far".into());
        let revising = build_prompt(&ctx, &pr, "go");
        assert!(revising.contains("already on this PR"));
        assert!(revising.contains("the story so far"));
    }
}
