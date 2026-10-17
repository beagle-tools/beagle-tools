//! The run's event sink: every event is appended to the run's JSONL log
//! and forwarded to the live channel the UI tails, in one place so the
//! two can never disagree.

use chrono::Utc;
use std::io::Write as _;
use std::path::PathBuf;
use tandem_core::agent::{RunEvent, RunEventKind, RunStatus};
use tandem_core::Result;
use tokio::sync::mpsc;

/// Serializes events to the log file and the live channel.
pub(crate) struct Emitter {
    run_id: String,
    seq: u64,
    log: std::fs::File,
    tx: mpsc::UnboundedSender<RunEvent>,
}

impl Emitter {
    pub(crate) fn new(
        run_id: String,
        log_path: &PathBuf,
        tx: mpsc::UnboundedSender<RunEvent>,
    ) -> Result<Self> {
        let log = std::fs::File::create(log_path)?;
        Ok(Self {
            run_id,
            seq: 0,
            log,
            tx,
        })
    }

    pub(crate) fn emit(&mut self, kind: RunEventKind, payload: impl Into<String>) {
        let event = RunEvent {
            run_id: self.run_id.clone(),
            seq: self.seq,
            at: Utc::now(),
            kind,
            payload: payload.into(),
        };
        self.seq += 1;
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = writeln!(self.log, "{json}");
        }
        let _ = self.tx.send(event);
    }

    pub(crate) fn lifecycle(&mut self, status: RunStatus, detail: &str) {
        let payload = serde_json::json!({ "status": status, "detail": detail }).to_string();
        self.emit(RunEventKind::Lifecycle, payload);
    }
}
