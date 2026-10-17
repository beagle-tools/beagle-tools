//! Tauri command modules — the IPC surface exposed to the frontend.

mod agent_pump;
pub mod agents;
pub mod conflicts;
pub mod files;
pub mod github_write;
pub mod mcp;
pub mod merge;
pub mod merge_tree;
pub mod prs;
pub mod review;
pub mod settings_cmd;
pub mod text_tools;
pub mod thread_context;
pub mod threads;
pub mod updates;
pub mod workspace;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Payload for `tandem://sync` events so the UI can show what's loading.
#[derive(Debug, Clone, Serialize)]
pub struct SyncEvent {
    pub key: String,
    pub phase: SyncPhase,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Started,
    Finished,
    Error,
}

pub fn emit_sync(app: &AppHandle, key: &str, phase: SyncPhase, error: Option<String>) {
    let event = SyncEvent {
        key: key.to_owned(),
        phase,
        error,
    };
    if let Err(e) = app.emit("tandem://sync", &event) {
        tracing::warn!(error = %e, "failed to emit sync event");
    }
}

/// Parse an `owner/name` slug or produce a user-facing config error.
pub fn parse_repo(slug: &str) -> Result<tandem_core::github::RepoRef, tandem_core::TandemError> {
    tandem_core::github::RepoRef::parse(slug)
        .ok_or_else(|| tandem_core::TandemError::Config(format!("invalid repo slug: {slug}")))
}
