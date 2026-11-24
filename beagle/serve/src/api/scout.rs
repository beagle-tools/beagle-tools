//! `/v1/scout/…`: projects and tasks, read through `beagle_scout::store`.
//!
//! Read-only: unlike `beagle scout list`, this never runs the auto-pause
//! sweep, so looking from the phone never changes a task.

use std::path::Path;

use beagle_scout::store::{self, Project, Store, Task};
use beagle_scout::{frontmatter, Error};
use serde_json::{json, Value};

use super::{file_time, lines_param, tail, timestamp, ApiError, ApiResult};
use crate::http::Request;

/// Routes `rest` (the path after `/v1/scout/`).
///
/// # Errors
/// [`ApiError`] for unknown paths, bad ids and unreadable files.
pub fn route(root: &Path, rest: &[&str], req: &Request) -> ApiResult {
    let store = Store::new(root);
    match rest {
        ["projects"] => projects(&store),
        ["projects", id] => project(&store, id),
        ["projects", id, "tasks", reference] => task(&store, id, reference),
        ["projects", id, "log"] => log(&store, id, lines_param(req.param("lines"))?),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

/// Maps scout's errors to the documented statuses.
fn api_error(e: &Error) -> ApiError {
    match e {
        Error::InvalidSlug(_) => ApiError::bad_request(e.to_string()),
        Error::ProjectNotFound(_) | Error::TaskNotFound { .. } => {
            ApiError::not_found(e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    }
}

/// `beagle scout list --json`'s row for `p`, minus the local `dir`, plus
/// what the phone shows on a project card.
fn summary(p: &Project) -> Value {
    let mut row = p.list_json();
    if let Value::Object(map) = &mut row {
        map.remove("dir");
        map.insert("goal".into(), json!(p.goal()));
        map.insert("created".into(), timestamp(Some(p.meta.created)));
        map.insert("updated".into(), timestamp(p.meta.updated));
        map.insert("last_activity".into(), file_time(p.last_activity));
    }
    row
}

fn task_summary(t: &Task) -> Value {
    json!({
        "id": t.id,
        "number": t.number,
        "slug": t.slug,
        "title": t.meta.title,
        "status": t.meta.status.as_str(),
        "checklist": { "done": t.checklist.0, "total": t.checklist.1 },
        "links": t.meta.links,
        "created": timestamp(Some(t.meta.created)),
        "updated": timestamp(t.meta.updated),
    })
}

/// A file's Markdown body, frontmatter removed.
fn body(path: &Path) -> Result<String, ApiError> {
    let source = store::read_capped(path).map_err(|e| api_error(&e))?;
    Ok(frontmatter::body(&source).trim_start().to_owned())
}

fn projects(store: &Store) -> ApiResult {
    let listing = store.list().map_err(|e| api_error(&e))?;
    Ok(json!({
        "projects": listing.projects.iter().map(summary).collect::<Vec<_>>(),
        "warnings": listing.warnings,
    }))
}

fn project(store: &Store, id: &str) -> ApiResult {
    let (p, warnings) = store.load_project(id).map_err(|e| api_error(&e))?;
    let mut out = summary(&p);
    if let Value::Object(map) = &mut out {
        map.remove("tasks_closed");
        map.remove("tasks_total");
        map.remove("working");
        map.insert("markdown".into(), json!(body(&p.file())?));
        map.insert(
            "tasks".into(),
            json!(p.tasks.iter().map(task_summary).collect::<Vec<_>>()),
        );
        let handoff = p
            .handoff()
            .map(|(t, steps)| json!({ "task": t.id, "markdown": steps }));
        map.insert("handoff".into(), handoff.unwrap_or(Value::Null));
        map.insert("warnings".into(), json!(warnings));
    }
    Ok(out)
}

fn task(store: &Store, id: &str, reference: &str) -> ApiResult {
    let p = store.project(id).map_err(|e| api_error(&e))?;
    let t = p.resolve_task(reference).ok_or_else(|| {
        api_error(&Error::TaskNotFound {
            project: id.to_owned(),
            task: reference.to_owned(),
        })
    })?;
    let mut out = task_summary(t);
    if let Value::Object(map) = &mut out {
        map.insert("project".into(), json!(p.id.as_str()));
        map.insert("markdown".into(), json!(body(&t.path)?));
        map.insert("next_steps".into(), json!(t.next_steps()));
    }
    Ok(out)
}

fn log(store: &Store, id: &str, lines: usize) -> ApiResult {
    let p = store.project(id).map_err(|e| api_error(&e))?;
    let path = p.log_file();
    let text = if path.is_file() {
        store::read_capped(&path).map_err(|e| api_error(&e))?
    } else {
        String::new()
    };
    let (lines, truncated) = tail(&text, lines);
    Ok(json!({ "project": p.id.as_str(), "lines": lines, "truncated": truncated }))
}

#[cfg(test)]
#[path = "../tests/scout.rs"]
mod tests;
