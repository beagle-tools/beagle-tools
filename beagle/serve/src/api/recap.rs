//! `/v1/recap/…`: PR recaps, read through `recap::store` (crate `beagle-recap`) and
//! its local comment state (`recap::comments`). Never calls GitHub:
//! comments are what the TUI last synced.

use std::path::{Path, PathBuf};

use ::recap::comments::{self, Thread, ThreadKind};
use ::recap::model::RecapId;
use ::recap::store::{Recap, Shelf, Store};
use ::recap::Error;
use serde_json::{json, Value};

use super::{diagram, ApiError, ApiResult};
use crate::http::Request;

/// Diffs past this are cut (on a character boundary) and flagged.
pub const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;

/// Routes `rest` (the path after `/v1/recap`).
///
/// # Errors
/// [`ApiError`] for unknown paths, bad ids and unreadable files.
pub fn route(root: &Path, rest: &[&str], req: &Request) -> ApiResult {
    let store = Store::new(root);
    match rest {
        [] => list(&store, req.param("archived")),
        [id] => one(&store, id),
        [id, "diff"] => diff(&store, id),
        [id, "comments"] => threads(&store, id),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

fn api_error(e: &Error) -> ApiError {
    match e {
        Error::InvalidId(_) => ApiError::bad_request(e.to_string()),
        Error::NoSuchRecap(_) => ApiError::not_found(e.to_string()),
        _ => ApiError::internal(e.to_string()),
    }
}

fn list(store: &Store, archived: Option<&str>) -> ApiResult {
    let shelf = match archived {
        None | Some("false") => Shelf::Active,
        Some("true") => Shelf::Archived,
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "archived must be true or false, not {other:?}"
            )))
        }
    };
    let (rows, warnings) = store.list_shelf(shelf).map_err(|e| api_error(&e))?;
    Ok(json!({ "recaps": rows, "warnings": warnings }))
}

/// The recap and the directory it was loaded from (active or archive).
fn load(store: &Store, raw: &str) -> Result<(Recap, PathBuf, bool), ApiError> {
    let id = RecapId::new(raw).map_err(|e| api_error(&e))?;
    let recap = store.load_any(&id).map_err(|e| api_error(&e))?;
    let active = store.shelf_dir(Shelf::Active).join(id.as_str());
    let archived = !active.is_dir();
    let dir = if archived {
        store.shelf_dir(Shelf::Archived).join(id.as_str())
    } else {
        active
    };
    Ok((recap, dir, archived))
}

fn one(store: &Store, raw: &str) -> ApiResult {
    let (recap, _, archived) = load(store, raw)?;
    let m = &recap.manifest;
    Ok(json!({
        "id": recap.id.as_str(),
        "archived": archived,
        "manifest": {
            "repo": m.repo,
            "number": m.number,
            "title": m.title,
            "url": m.url,
            "state": m.state,
            "author": m.author,
            "branch": m.branch,
            "base": m.base,
            "ticket": m.ticket,
            "ticket_url": m.ticket_url,
            "additions": m.additions,
            "deletions": m.deletions,
            "files": m.files,
            "created": m.created,
            "updated": m.updated,
            "merged_at": m.merged_at,
        },
        "summary": recap.summary,
        "diagrams": recap
            .diagrams
            .iter()
            .map(|d| diagram(&d.name, &d.body))
            .collect::<Vec<_>>(),
        "warnings": recap.warnings,
    }))
}

fn diff(store: &Store, raw: &str) -> ApiResult {
    let (recap, _, _) = load(store, raw)?;
    let cut = beagle_utils::text::truncate_bytes(&recap.diff, MAX_DIFF_BYTES);
    Ok(json!({
        "id": recap.id.as_str(),
        "diff": cut,
        "truncated": cut.len() < recap.diff.len(),
    }))
}

fn threads(store: &Store, raw: &str) -> ApiResult {
    let (recap, dir, _) = load(store, raw)?;
    let author = recap.manifest.author.clone().unwrap_or_default();
    let synced = comments::load(&dir);
    let read = comments::load_read(&dir);
    let threads: Vec<Value> = synced
        .threads
        .iter()
        .map(|t| thread(&dir, t, comments::is_read(t, &read), &author))
        .collect();
    Ok(json!({
        "id": recap.id.as_str(),
        "fetched": (!synced.fetched.is_empty()).then_some(synced.fetched),
        "threads": threads,
    }))
}

fn thread(dir: &Path, t: &Thread, read: bool, author: &str) -> Value {
    let (kind, path, line, resolved, outdated, state) = match &t.kind {
        ThreadKind::Inline {
            path,
            line,
            resolved,
            outdated,
        } => (
            "inline",
            Some(path.as_str()),
            *line,
            *resolved,
            *outdated,
            None,
        ),
        ThreadKind::Conversation => ("conversation", None, None, false, false, None),
        ThreadKind::Review { state } => ("review", None, None, false, false, Some(state.as_str())),
    };
    let review = comments::load_review(dir, &t.id).map(|r| {
        let (in_short, rest) = r.split();
        json!({
            "in_short": in_short,
            "markdown": rest,
            "current": comments::is_reviewed(dir, t, author),
        })
    });
    json!({
        "id": t.id,
        "kind": kind,
        "path": path,
        "line": line,
        "resolved": resolved,
        "outdated": outdated,
        "state": state,
        "place": t.place(),
        "read": read,
        "comments": t.comments,
        "review": review,
    })
}

#[cfg(test)]
#[path = "../tests/recap.rs"]
mod tests;
