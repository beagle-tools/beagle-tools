//! `/v1/standup/…`: standup reports, read through `standup::store` (crate `beagle-standup`).

use std::path::Path;

use ::standup::model::{ReportDate, StateKind};
use ::standup::store::{Store, Ticket};
use ::standup::Error;
use serde_json::{json, Value};

use super::{diagram, ApiError, ApiResult};
use crate::http::Request;

/// Routes `rest` (the path after `/v1/standup`).
///
/// # Errors
/// [`ApiError`] for unknown paths, bad dates and unreadable files.
pub fn route(root: &Path, rest: &[&str], req: &Request) -> ApiResult {
    let store = Store::new(root);
    match rest {
        [] => list(&store),
        [date] => report(&store, date),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

fn api_error(e: &Error) -> ApiError {
    match e {
        Error::InvalidDate(_) => ApiError::bad_request(e.to_string()),
        Error::NoSuchReport(_) | Error::NoReports => ApiError::not_found(e.to_string()),
        _ => ApiError::internal(e.to_string()),
    }
}

fn list(store: &Store) -> ApiResult {
    let reports = store.list().map_err(|e| api_error(&e))?;
    Ok(json!({ "reports": reports }))
}

/// The API's spelling of a ticket's state bucket.
fn state_kind(kind: StateKind) -> &'static str {
    match kind {
        StateKind::Progress => "progress",
        StateKind::Review => "review",
        StateKind::Blocked => "blocked",
        StateKind::Done => "done",
        StateKind::Todo => "todo",
        StateKind::Other => "other",
    }
}

fn ticket(t: &Ticket) -> Value {
    let m = &t.manifest;
    json!({
        "id": t.id.as_str(),
        "title": m.title,
        "url": m.url,
        "state": m.state,
        "state_kind": state_kind(t.state_kind()),
        "systems": m.systems,
        "prs": m.prs,
        "summary": t.summary,
        "diagrams": t.diagrams.iter().map(|d| diagram(&d.name, &d.body)).collect::<Vec<_>>(),
    })
}

fn report(store: &Store, raw: &str) -> ApiResult {
    let date = if raw == "latest" {
        store
            .latest()
            .map_err(|e| api_error(&e))?
            .ok_or_else(|| api_error(&Error::NoReports))?
    } else {
        ReportDate::parse(raw).map_err(|e| api_error(&e))?
    };
    let report = store.load(date).map_err(|e| api_error(&e))?;
    let m = &report.manifest;
    Ok(json!({
        "date": report.date.to_string(),
        "weekday": report.date.weekday(),
        "title": m.title,
        "since": m.since,
        "created": m.created,
        "overview": report.overview,
        "diagrams": report.diagrams.iter().map(|d| diagram(&d.name, &d.body)).collect::<Vec<_>>(),
        "tickets": report.tickets.iter().map(ticket).collect::<Vec<_>>(),
        "warnings": report.warnings,
    }))
}

#[cfg(test)]
#[path = "../tests/standup.rs"]
mod tests;
