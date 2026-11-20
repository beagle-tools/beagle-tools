//! Pulling a PR's metadata and diff from GitHub through the `gh` CLI.
//!
//! Shelling out keeps the dependency budget flat and reuses the user's
//! existing `gh auth`. Parsing is a pure function so it's tested without
//! the network.

use std::time::Duration;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{ticket_from_title, PrRef, PrState, RecapManifest};

const FIELDS: &str = "number,title,url,state,isDraft,author,headRefName,baseRefName,additions,deletions,changedFiles,createdAt,updatedAt,mergedAt";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPr {
    number: u64,
    title: String,
    url: String,
    state: String,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    author: Option<GhAuthor>,
    #[serde(default)]
    head_ref_name: Option<String>,
    #[serde(default)]
    base_ref_name: Option<String>,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default)]
    changed_files: u64,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAuthor {
    login: String,
}

/// Builds a manifest from `gh pr view --json …` output. The ticket id is
/// taken from the title prefix (`ABC-123: …`); its URL is left to the caller.
///
/// # Errors
/// [`Error::Json`] if the JSON doesn't have the expected shape.
pub fn manifest_from_json(pr: &PrRef, json: &str) -> Result<RecapManifest> {
    let gh: GhPr = serde_json::from_str(json)?;
    let state = PrState::from_github(&gh.state, gh.is_draft).ok_or_else(|| Error::Tool {
        tool: "gh",
        message: format!("unknown PR state `{}`", gh.state),
    })?;
    Ok(RecapManifest {
        repo: pr.full_repo(),
        number: gh.number,
        ticket: ticket_from_title(&gh.title),
        title: gh.title,
        url: gh.url,
        state,
        author: gh.author.map(|a| a.login),
        branch: gh.head_ref_name,
        base: gh.base_ref_name,
        ticket_url: None,
        additions: gh.additions,
        deletions: gh.deletions,
        files: gh.changed_files,
        created: gh.created_at,
        updated: gh.updated_at,
        merged_at: gh.merged_at,
    })
}

/// Bound on one `gh` API call (`pr view`, `pr diff`, `api`, `pr merge`).
pub(crate) const GH_TIMEOUT: Duration = Duration::from_secs(30);
/// Bound on a `gh api --paginate` call, which makes several requests.
pub(crate) const GH_PAGINATED_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs `gh args` through [`beagle_utils::gh`] (augmented `PATH`, bounded),
/// with the longer bound for `--paginate`.
fn gh(args: &[&str]) -> Result<String> {
    let timeout = if args.contains(&"--paginate") {
        GH_PAGINATED_TIMEOUT
    } else {
        GH_TIMEOUT
    };
    beagle_utils::gh::run(args, timeout).map_err(|e| Error::Tool {
        tool: "gh",
        message: e.to_string(),
    })
}

/// Fetches the PR's metadata and its unified diff.
///
/// # Errors
/// [`Error::Tool`] when `gh` is missing, unauthenticated, or the PR doesn't
/// exist; [`Error::Json`] on unexpected output.
pub fn fetch(pr: &PrRef) -> Result<(RecapManifest, String)> {
    let number = pr.number.to_string();
    let repo = pr.full_repo();
    let json = gh(&["pr", "view", &number, "-R", &repo, "--json", FIELDS])?;
    let manifest = manifest_from_json(pr, &json)?;
    let diff = gh(&["pr", "diff", &number, "-R", &repo])?;
    Ok((manifest, diff))
}

#[cfg(test)]
#[path = "tests/gh.rs"]
mod tests;

#[derive(Debug, Deserialize)]
struct GhDiscussion {
    #[serde(default)]
    reviews: Vec<GhReview>,
    #[serde(default)]
    comments: Vec<GhComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReview {
    #[serde(default)]
    author: Option<GhAuthor>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
struct GhComment {
    #[serde(default)]
    author: Option<GhAuthor>,
    #[serde(default)]
    body: String,
}

/// An inline (line-anchored) review comment, from the REST API.
#[derive(Debug, Deserialize)]
struct GhInline {
    #[serde(default)]
    user: Option<GhAuthor>,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u64>,
    #[serde(default)]
    original_line: Option<u64>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    in_reply_to_id: Option<u64>,
}

fn who(a: Option<&GhAuthor>) -> &str {
    a.map_or("?", |a| a.login.as_str())
}

/// Formats a PR's reviews, inline comments and conversation as plain text.
/// `discussion` is `gh pr view --json reviews,comments`; `inline` is the
/// REST `pulls/<n>/comments` array (inline comments aren't in the former).
///
/// # Errors
/// [`Error::Json`] if either input isn't the expected shape.
pub fn format_comments(discussion: &str, inline: &str) -> Result<String> {
    use std::fmt::Write as _;
    let d: GhDiscussion = serde_json::from_str(discussion)?;
    let inline: Vec<GhInline> = serde_json::from_str(inline)?;
    let mut out = String::new();
    // `write!` into a `String` is infallible; its results are dropped.
    let _ = writeln!(out, "## Reviews ({})", d.reviews.len());
    for r in &d.reviews {
        let body = r.body.trim();
        let _ = writeln!(
            out,
            "- {} {}{}",
            who(r.author.as_ref()),
            r.state,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {}", body.replace('\n', "\n  "))
            }
        );
    }
    let _ = writeln!(out, "\n## Inline comments ({})", inline.len());
    for c in &inline {
        let line = c
            .line
            .or(c.original_line)
            .map(|l| format!(":{l}"))
            .unwrap_or_default();
        let reply = if c.in_reply_to_id.is_some() {
            "  ↳ reply "
        } else {
            "- "
        };
        let _ = writeln!(
            out,
            "{reply}{} on {}{line}: {}",
            who(c.user.as_ref()),
            c.path,
            c.body.trim().replace('\n', "\n  ")
        );
    }
    let _ = writeln!(out, "\n## Conversation ({})", d.comments.len());
    for c in &d.comments {
        let _ = writeln!(
            out,
            "- {}: {}",
            who(c.author.as_ref()),
            c.body.trim().replace('\n', "\n  ")
        );
    }
    Ok(out)
}

/// Reads a PR's reviews, inline review comments and conversation —
/// read-only (`gh pr view` plus a GET of the inline-comments endpoint).
///
/// # Errors
/// [`Error::Tool`] when `gh` fails; [`Error::Json`] on unexpected output.
pub fn comments(pr: &PrRef) -> Result<String> {
    let number = pr.number.to_string();
    let repo = pr.full_repo();
    let discussion = gh(&[
        "pr",
        "view",
        &number,
        "-R",
        &repo,
        "--json",
        "reviews,comments",
    ])?;
    let endpoint = format!("repos/{repo}/pulls/{number}/comments");
    let inline = gh(&["api", "--method", "GET", "--paginate", "--slurp", &endpoint])?;
    // --slurp wraps pages in an outer array; flatten it.
    let pages: Vec<serde_json::Value> = serde_json::from_str(&inline)?;
    let flat: Vec<serde_json::Value> = pages
        .into_iter()
        .flat_map(|p| match p {
            serde_json::Value::Array(items) => items,
            other => vec![other],
        })
        .collect();
    format_comments(&discussion, &serde_json::to_string(&flat)?)
}
