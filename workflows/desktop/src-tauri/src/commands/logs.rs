//! A run's log, as a transcript.
//!
//! This used to hand the webview the raw log file, so a model run showed
//! up as walls of stream-json. It now reads the same stored events
//! agenttop reads and runs them through the same
//! `agent_core::transcript::prettify`, so a run reads identically in both
//! apps: a model run as a conversation, a script run as levelled lines.
//!
//! Events are pruned with the logs, four hours after a run finishes. A run
//! with none left is not an error — it is the normal end state of an old
//! run, and the UI says so rather than showing an empty pane.

use agent_core::transcript::{prettify, PrettyEvent};
use tauri::State;

use crate::error::{lock_failed, AppError};
use crate::state::AppState;

/// Plenty for a long model run (a 60-tool-call stage is ~300 events),
/// while keeping one pathological run from stalling the webview.
const EVENT_LIMIT: u32 = 3000;

/// Event kinds only a model runner emits. Seeing any of them is what
/// makes a run a conversation rather than a script's log.
const MODEL_KINDS: [&str; 5] = ["system", "assistant", "user", "result", "item.completed"];

#[derive(Debug, serde::Serialize)]
pub struct Transcript {
    pub entries: Vec<PrettyEvent>,
    /// True for a model run, false for a script: the page labels and
    /// lays the two out differently.
    pub agent: bool,
    /// Set when there is nothing to show, with the reason.
    pub missing_reason: Option<String>,
}

#[tauri::command]
pub fn read_transcript(state: State<'_, AppState>, id: String) -> Result<Transcript, AppError> {
    let store = state.store.lock().map_err(|_| lock_failed("store"))?;
    let events = store
        .events_after(&id, 0, EVENT_LIMIT)
        .map_err(|e| AppError::new(format!("could not read events for {id}: {e}")))?;
    drop(store);

    if events.is_empty() {
        return Ok(Transcript {
            entries: Vec::new(),
            agent: false,
            missing_reason: Some(
                "no events — agentd prunes a run's log four hours after it finishes, \
                 and a run that never started has none"
                    .to_owned(),
            ),
        });
    }
    let agent = events
        .iter()
        .any(|e| MODEL_KINDS.contains(&e.kind.as_str()));
    Ok(Transcript {
        entries: events.iter().flat_map(prettify).collect(),
        agent,
        missing_reason: None,
    })
}
