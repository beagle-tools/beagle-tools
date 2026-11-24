//! `/v1/tandem/…`: the review TUI's queue, read through `tandem_tui::read`
//! (the rows `beagle tandem list` prints). The store is opened read-only
//! per request and never created; posting and approving stay on the Mac.

use std::path::Path;

use serde_json::json;
use tandem_tui::read;

use super::{ApiError, ApiResult};
use crate::http::Request;

/// Routes `rest` (the path after `/v1/tandem`); `db` is the review store.
///
/// # Errors
/// [`ApiError`] for unknown paths, bad repos or numbers, and store errors.
pub fn route(db: &Path, rest: &[&str], req: &Request) -> ApiResult {
    match rest {
        ["queue"] => queue(db),
        ["prs", owner, repo, number] => pr(db, owner, repo, number),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

fn internal(e: &impl std::fmt::Display) -> ApiError {
    ApiError::internal(format!("tandem review store: {e}"))
}

fn queue(db: &Path) -> ApiResult {
    match read::open(db).map_err(|e| internal(&e))? {
        None => Ok(json!({ "prs": [] })),
        Some(cache) => read::queue(&cache).map_err(|e| internal(&e)),
    }
}

/// GitHub's own rules for owner and repo names, loosely: what can appear
/// in a URL path segment without escaping.
fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn pr(db: &Path, owner: &str, repo: &str, number: &str) -> ApiResult {
    if !is_name(owner) || !is_name(repo) {
        return Err(ApiError::bad_request(format!("not a repo: {owner}/{repo}")));
    }
    let number: u64 = number
        .parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| ApiError::bad_request(format!("not a PR number: {number}")))?;
    let cache = read::open(db).map_err(|e| internal(&e))?.ok_or_else(|| {
        ApiError::not_found("no review store yet: queue a PR with `beagle tandem add`")
    })?;
    read::pr(&cache, &format!("{owner}/{repo}"), number).map_err(|e| internal(&e))
}

#[cfg(test)]
#[path = "../tests/tandem.rs"]
mod tests;
