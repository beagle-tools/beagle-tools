//! The review context `claim` writes for the agent: who it is, what it
//! must emit and how, then the PR — description, everything people and
//! bots already said on it, and the diff. The *how to review* lives in
//! the agentd prompt; this file is the tool's half of the contract and
//! changes only with `ingest`.

use std::fmt::Write as _;

use tandem_core::github::PrDetail;

use crate::verdict::Verdict;

/// What goes into the context besides the PR itself.
#[derive(Debug, Clone)]
pub struct ContextInput<'a> {
    /// The claimed run the review loads into.
    pub run_id: &'a str,
    /// How the agent calls this tool back (`beagle tandem`, `tandem-cli`).
    pub program: &'a str,
    pub detail: &'a PrDetail,
    pub raw_diff: &'a str,
    /// The summary an earlier review wrote; when set, no new one is
    /// wanted (only a fresh verdict).
    pub existing_summary: Option<&'a str>,
}

/// The whole context file, markdown.
pub fn build(input: &ContextInput<'_>) -> String {
    let pr = &input.detail.pull_request;
    let mut out = format!(
        "# Tandem review context\n\n\
         You are reviewing a GitHub pull request for the user. Everything \
         you write stays LOCAL in their review TUI until they choose to post it: \
         never post, comment, approve or push on GitHub yourself.\n\n\
         - run id: {run}\n\
         - pull request: https://github.com/{repo}/pull/{n} — {title}\n\
         - author: @{author}\n\
         - branch: {head} -> {base}\n\
         - head sha: {sha}\n\
         - size: {files} files, +{add} −{del}\n\n",
        run = input.run_id,
        repo = pr.repo.slug(),
        n = pr.number,
        title = pr.title,
        author = pr.author.login,
        head = pr.head_ref,
        base = pr.base_ref,
        sha = pr.head_sha,
        files = pr.changed_files,
        add = pr.additions,
        del = pr.deletions,
    );
    out.push_str(&contract(
        input.program,
        input.run_id,
        input.existing_summary.is_some(),
    ));
    if let Some(summary) = input.existing_summary {
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "## The summary already written for this PR\n\n\
             An earlier review wrote this. Do not write another \
             `tandem_summary`; your verdict replaces the one at its top.\n\n\
             {summary}\n\n"
        );
    }
    let body = if pr.body.trim().is_empty() {
        "(no description)"
    } else {
        pr.body.trim()
    };
    // Writing to a String cannot fail.
    let _ = write!(out, "## Pull request description\n\n{body}\n\n");
    out.push_str(&discussion(input.detail));
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "## Diff\n\n```diff\n{}\n```\n",
        input.raw_diff.trim_end()
    );
    out
}

fn contract(program: &str, run_id: &str, summary_exists: bool) -> String {
    let verdicts = Verdict::ALL
        .iter()
        .map(|v| format!("`{}` ({})", v.key(), v.label()))
        .collect::<Vec<_>>()
        .join(", ");
    let summary = if summary_exists {
        "A summary already exists (below), so emit NO `tandem_summary` line.".to_owned()
    } else {
        "Exactly ONE summary line:\n\n\
         {\"type\":\"tandem_summary\",\"body\":\"<markdown>\"}\n"
            .to_owned()
    };
    format!(
        "## What to emit\n\n\
         Write JSON lines, one object per line, no code fences, to a file, \
         then load them with:\n\n\
         `{program} ingest --run {run_id} --file <that file>`\n\n\
         `ingest` checks every line and loads nothing if any is wrong; it \
         prints what to fix — fix the file and run it again.\n\n\
         Exactly ONE verdict line:\n\n\
         {{\"type\":\"tandem_verdict\",\"verdict\":\"<key>\",\"reason\":\"<one line>\"}}\n\n\
         where `<key>` is one of {verdicts}.\n\n\
         {summary}\n\
         Then one line per review comment:\n\n\
         {{\"type\":\"tandem_comment\",\"path\":\"<file>\",\"side\":\"new|old\",\
         \"line\":<n>,\"end_line\":<n|null>,\
         \"severity\":\"info|suggestion|issue|blocker\",\"tag\":\"<kind>\",\
         \"body\":\"<markdown>\",\"suggestion\":<null|\"replacement code\">,\
         \"context\":\"<markdown>\",\"duplicate_of\":<null|\"<ref>\">}}\n\n\
         - `body` is the comment as it would be posted on GitHub: the \
         problem and the fix, short. `suggestion`, when set, is the \
         COMPLETE replacement for lines `line..=end_line` on the new side \
         (keep indentation, no fences; \"\" deletes them).\n\
         - `context` is required: the short history behind the comment, \
         shown to the user as a local reply under it and never posted.\n\
         - `line` (and `end_line`) must be lines that appear on that side \
         of the diff below. For a PR-level remark use `\"path\":\"\"` and \
         `\"line\":0`.\n\
         - `tag` is one or two words (`correctness`, `security`, \
         `tests`, `naming`, `patterns`…). Use `duplicate` when someone \
         already raised it or you agree with an existing comment, and set \
         `duplicate_of` to who and where — e.g. `@cursor[bot] \
         https://github.com/o/r/pull/1#discussion_r123` — using the \
         references in \"Discussion so far\".\n\
         - Strings are JSON: escape newlines as `\\n`.\n\n"
    )
}

/// Every comment and review body already on the PR, oldest first, each
/// with the link a duplicate can point at.
fn discussion(detail: &PrDetail) -> String {
    let pr = &detail.pull_request;
    let base = format!("https://github.com/{}/pull/{}", pr.repo.slug(), pr.number);
    let mut out = String::from("## Discussion so far\n\n");
    if detail.comments.is_empty() && detail.review_bodies.is_empty() {
        out.push_str("Nobody has commented yet.\n\n");
        return out;
    }
    for review in &detail.review_bodies {
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "### @{} — review ({:?}) — {base}#pullrequestreview-{}\n\n{}\n\n",
            review.author.login,
            review.verdict,
            review.id,
            review.body.trim()
        );
    }
    for c in &detail.comments {
        let (place, link) = match &c.path {
            Some(path) => {
                let line = c.line.or(c.original_line).unwrap_or(0);
                (
                    format!("{path}:{line}"),
                    format!("{base}#discussion_r{}", c.id),
                )
            }
            None => ("the PR".to_owned(), format!("{base}#issuecomment-{}", c.id)),
        };
        let reply = if c.in_reply_to_id.is_some() {
            " (reply)"
        } else {
            ""
        };
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "### @{} on {place}{reply} — {link}\n\n{}\n\n",
            c.author.login,
            c.body.trim()
        );
    }
    out
}
