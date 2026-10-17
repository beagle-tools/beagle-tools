//! Tauri commands, one module per domain.
//!
//! Commands validate their input, take the store lock briefly, delegate to
//! `agent-core`, and release. Nothing long-running happens while the lock
//! is held — the poller needs it every two seconds.

pub mod dag;
pub mod jobs;
pub mod links;
pub mod logs;
pub mod pipelines;
pub mod runs;
pub mod updates;
