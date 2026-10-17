//! Agent execution for Tandem.
//!
//! v1 runs agents as **local headless processes** (`claude -p`,
//! `codex exec`, or any custom command) behind the [`runner`] interface.
//! v2 will add a Docker + squid sandbox backend enforcing each spec's
//! `network_allowlist` — the interface is designed so that lands as a new
//! implementation, not a rewrite. See docs/SANDBOXING.md.
//!
//! Agents emit review comments through a simple contract (see
//! [`context`]): JSON lines of `{"type": "tandem_comment", ...}` written to
//! stdout or to the `$TANDEM_COMMENTS_FILE` the runner provides. Comments
//! are stored locally and are never posted to GitHub.

pub mod command;
pub mod context;
pub mod conventions;
pub mod conversation;
mod emitter;
pub mod events;
pub mod reply;
pub mod runner;
mod summary;

pub use runner::{LocalProcessRunner, RunHandle, RunRequest};
