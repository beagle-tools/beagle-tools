//! Fetching a PR's comment threads from GitHub (`gh api graphql`).

use std::time::Duration;

use serde::Deserialize;

use super::{Comment, CommentsConfig, Thread, ThreadKind};
use crate::error::{Error, Result};

const QUERY: &str =
    "query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){\
reviewThreads(first:100){nodes{isResolved isOutdated path line originalLine \
comments(first:100){nodes{databaseId author{login __typename} body createdAt url}}}} \
comments(first:100){nodes{databaseId author{login __typename} body createdAt url}} \
reviews(first:100){nodes{databaseId author{login __typename} body state submittedAt url}}}}}";

#[derive(Debug, Deserialize)]
struct GqlAuthor {
    login: String,
    #[serde(rename = "__typename")]
    typename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlComment {
    database_id: Option<u64>,
    author: Option<GqlAuthor>,
    #[serde(default)]
    body: String,
    #[serde(default, alias = "submittedAt")]
    created_at: Option<String>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlThread {
    is_resolved: bool,
    is_outdated: bool,
    path: String,
    line: Option<u64>,
    original_line: Option<u64>,
    comments: Nodes<GqlComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlPr {
    review_threads: Nodes<GqlThread>,
    comments: Nodes<GqlComment>,
    reviews: Nodes<GqlComment>,
}

fn comment(c: &GqlComment, config: &CommentsConfig) -> Comment {
    let (author, bot) = c.author.as_ref().map_or(("ghost".to_owned(), false), |a| {
        (a.login.clone(), a.typename == "Bot")
    });
    // GraphQL drops the `[bot]` suffix REST uses; put it back so the
    // ignore list can use the names people see on GitHub.
    let author = if bot && !author.ends_with("[bot]") {
        format!("{author}[bot]")
    } else {
        author
    };
    Comment {
        id: c.database_id.unwrap_or_default().to_string(),
        context: config.ignores(&author, bot),
        author,
        bot,
        created: c.created_at.clone().unwrap_or_default(),
        body: c.body.trim().to_owned(),
        url: c.url.clone(),
    }
}

/// Builds the threads from the GraphQL response. Threads made only of
/// ignored comments are dropped. Pure, so it is tested without GitHub.
///
/// # Errors
/// [`Error::Json`] if `json` is not the query's shape.
pub fn parse_threads(json: &str, config: &CommentsConfig) -> Result<Vec<Thread>> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let pr: GqlPr = serde_json::from_value(value["data"]["repository"]["pullRequest"].clone())?;
    let mut out = Vec::new();
    for t in &pr.review_threads.nodes {
        let comments: Vec<Comment> = t
            .comments
            .nodes
            .iter()
            .map(|c| comment(c, config))
            .collect();
        let Some(first) = comments.first() else {
            continue;
        };
        out.push(Thread {
            id: format!("t-{}", first.id),
            kind: ThreadKind::Inline {
                path: t.path.clone(),
                line: t.line.or(t.original_line),
                resolved: t.is_resolved,
                outdated: t.is_outdated,
            },
            comments,
        });
    }
    for c in &pr.comments.nodes {
        let c = comment(c, config);
        out.push(Thread {
            id: format!("c-{}", c.id),
            kind: ThreadKind::Conversation,
            comments: vec![c],
        });
    }
    for r in &pr.reviews.nodes {
        if r.body.trim().is_empty() {
            continue; // an approval with no words is not a comment
        }
        let state = r.state.clone().unwrap_or_default();
        let c = comment(r, config);
        out.push(Thread {
            id: format!("r-{}", c.id),
            kind: ThreadKind::Review { state },
            comments: vec![c],
        });
    }
    out.retain(|t| t.comments.iter().any(|c| !c.context));
    out.sort_by(|a, b| {
        let first = |t: &Thread| {
            t.comments
                .first()
                .map(|c| c.created.clone())
                .unwrap_or_default()
        };
        first(a).cmp(&first(b))
    });
    Ok(out)
}

/// Whether `gh`'s error text is a rate limit.
#[must_use]
pub fn is_rate_limit(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("rate limit") || m.contains("abuse detection") || m.contains("http 429")
}

/// How long to wait before the one retry after a rate limit.
pub const RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);

fn gh(args: &[&str]) -> std::result::Result<String, String> {
    beagle_utils::gh::run(args, crate::gh::GH_TIMEOUT).map_err(|e| e.to_string())
}

/// Fetches one PR's threads, waiting [`RATE_LIMIT_WAIT`] and retrying once
/// on a rate limit.
///
/// # Errors
/// [`Error::Tool`] when `gh` fails (twice, for a rate limit).
pub fn fetch(repo: &str, number: u64, config: &CommentsConfig) -> Result<Vec<Thread>> {
    let (owner, name) = repo.split_once('/').ok_or_else(|| Error::Tool {
        tool: "gh",
        message: format!("{repo} is not owner/repo"),
    })?;
    let number_arg = format!("n={number}");
    let owner_arg = format!("o={owner}");
    let repo_arg = format!("r={name}");
    let query_arg = format!("query={QUERY}");
    let args = [
        "api",
        "graphql",
        "-F",
        &owner_arg,
        "-F",
        &repo_arg,
        "-F",
        &number_arg,
        "-f",
        &query_arg,
    ];
    let json = match gh(&args) {
        Ok(json) => json,
        Err(e) if is_rate_limit(&e) => {
            std::thread::sleep(RATE_LIMIT_WAIT);
            gh(&args).map_err(|e| Error::Tool {
                tool: "gh",
                message: format!("rate limited twice (30s apart): {e}"),
            })?
        }
        Err(e) => {
            return Err(Error::Tool {
                tool: "gh",
                message: e,
            })
        }
    };
    parse_threads(&json, config)
}
