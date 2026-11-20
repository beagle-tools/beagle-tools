//! `/v1/rca/…`: RCA workspaces, read through `beagle_rca::store` (the rca
//! crate is named `beagle`; the workspace renames it `beagle-rca`).
//!
//! A root without `rcas/` is an empty listing: the store is never opened
//! there, because opening creates the directory and serving never writes.

use std::path::Path;

use beagle_rca::model::{RcaId, RcaSummary, SectionKind};
use beagle_rca::store::{self, Listing, Store};
use serde_json::{json, Value};

use super::{diagram, ApiError, ApiResult};
use crate::http::Request;

/// Routes `rest` (the path after `/v1/rca`).
///
/// # Errors
/// [`ApiError`] for unknown paths, bad ids and unreadable files.
pub fn route(root: &Path, rest: &[&str], req: &Request) -> ApiResult {
    match rest {
        [] => list(root),
        [id] => one(&open(root)?, id),
        [id, "sections", name] => section(&open(root)?, id, name),
        [id, "diagrams", which] => diagram_of(&open(root)?, id, which),
        _ => Err(ApiError::not_found(format!("no endpoint {}", req.path))),
    }
}

fn internal(e: &beagle_rca::Error) -> ApiError {
    ApiError::internal(e.to_string())
}

/// The store, or `404` for every RCA when there is no `rcas/` yet.
fn open(root: &Path) -> Result<Store, ApiError> {
    if !root.join(store::RCAS_DIR).is_dir() {
        return Err(ApiError::not_found(format!(
            "no RCAs: {} has no {}/",
            root.display(),
            store::RCAS_DIR
        )));
    }
    Store::open(root).map_err(|e| internal(&e))
}

/// `beagle rca list --json`'s entries for `summaries`, as JSON values.
fn entries(summaries: &[&RcaSummary]) -> Result<Vec<Value>, ApiError> {
    let text = beagle_rca::list_json(summaries).map_err(|e| internal(&e))?;
    serde_json::from_str(&text).map_err(|e| ApiError::internal(e.to_string()))
}

fn list(root: &Path) -> ApiResult {
    let listing = if root.join(store::RCAS_DIR).is_dir() {
        open(root)?.list_all().map_err(|e| internal(&e))?
    } else {
        Listing::default()
    };
    let summaries: Vec<&RcaSummary> = listing.summaries.iter().collect();
    Ok(json!({
        "rcas": entries(&summaries)?,
        "broken": listing
            .broken
            .iter()
            .map(|b| json!({ "dir": b.dir_name, "reason": b.reason }))
            .collect::<Vec<_>>(),
        "warnings": listing.warnings.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(),
    }))
}

/// The id, validated, of a workspace that exists (active or archived).
fn existing(store: &Store, raw: &str) -> Result<RcaId, ApiError> {
    let id = RcaId::new(raw).map_err(|e| ApiError::bad_request(e.to_string()))?;
    if store
        .workspace_dir(&id)
        .join(store::MANIFEST_FILE)
        .is_file()
    {
        Ok(id)
    } else {
        Err(ApiError::not_found(format!("no RCA `{raw}`")))
    }
}

/// A section's name in the API: its file stem (`root-cause`).
fn section_name(kind: SectionKind) -> &'static str {
    kind.file_name().trim_end_matches(".md")
}

fn one(store: &Store, raw: &str) -> ApiResult {
    let id = existing(store, raw)?;
    let meta = store.read_meta(&id).map_err(|e| internal(&e))?;
    let archived = !store
        .watch_root()
        .join(id.as_str())
        .join(store::MANIFEST_FILE)
        .is_file();
    let summary = RcaSummary {
        id: id.clone(),
        meta,
        archived,
    };
    let entry = entries(&[&summary])?
        .into_iter()
        .next()
        .unwrap_or(Value::Null);
    let present = store.section_mtimes(&id);
    let sections: Vec<Value> = SectionKind::ALL
        .into_iter()
        .map(|kind| {
            json!({
                "name": section_name(kind),
                "title": kind.title(),
                "present": present.contains_key(&kind),
            })
        })
        .collect();
    let diagrams = store.list_diagrams(&id).map_err(|e| internal(&e))?;
    Ok(json!({
        "rca": entry,
        "tickets": summary.meta.tickets,
        "sections": sections,
        "diagrams": diagrams.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
    }))
}

fn section(store: &Store, raw: &str, name: &str) -> ApiResult {
    let id = existing(store, raw)?;
    let kind = SectionKind::ALL
        .into_iter()
        .find(|k| section_name(*k) == name)
        .ok_or_else(|| ApiError::not_found(format!("no section `{name}`")))?;
    let markdown = store
        .read_section(&id, kind)
        .map_err(|e| internal(&e))?
        .ok_or_else(|| ApiError::not_found(format!("`{raw}` has no {name} yet")))?;
    Ok(json!({
        "id": id.as_str(),
        "name": name,
        "title": kind.title(),
        "markdown": markdown,
    }))
}

fn diagram_of(store: &Store, raw: &str, which: &str) -> ApiResult {
    let id = existing(store, raw)?;
    let diagrams = store.list_diagrams(&id).map_err(|e| internal(&e))?;
    let entry = match which.parse::<usize>() {
        Ok(n) => n.checked_sub(1).and_then(|i| diagrams.get(i)),
        Err(_) => diagrams.iter().find(|d| d.name == which),
    }
    .ok_or_else(|| ApiError::not_found(format!("`{raw}` has no diagram {which}")))?;
    let body = store
        .read_diagram(entry)
        .map_err(|e| internal(&e))?
        .ok_or_else(|| ApiError::not_found(format!("diagram {which} vanished")))?;
    Ok(diagram(&entry.name, &body))
}

#[cfg(test)]
#[path = "../tests/rca.rs"]
mod tests;
