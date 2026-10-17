//! Shared core for the agentd stack.
//!
//! The abstraction hierarchy, borrowed from Kubernetes:
//!
//! | Kubernetes | Here           | Meaning                                        |
//! |------------|----------------|------------------------------------------------|
//! | Namespace  | [`types::ProjectSpec`] | A grouping of related agents (a directory) |
//! | Workload   | [`types::AgentSpec`]   | A declared agent: prompt + runner + schedule |
//! | Pod        | [`types::Run`]         | One execution instance of an agent         |
//! | Container  | tmux session           | The process actually executing the run     |
//!
//! Manifests on disk are the desired state; the `SQLite` [`state::Store`] is the
//! observed state; `agentd` reconciles one into the other. `agentctl` and
//! `agenttop` only ever read/write the store — they never talk to the daemon.

mod codex;
/// The crate-wide [`CoreError`].
pub mod error;
mod events;
pub mod flow;
/// Run id generation (sortable, human-readable).
pub mod ids;
pub mod lineage;
pub mod links;
pub mod logtext;
/// Loading and validating manifests from disk into [`types::AgentSpec`]s.
pub mod manifest;
mod manifest_util;
pub mod params;
pub mod paths;
pub mod pipelines;
mod prune;
mod schema;
/// The `SQLite` [`state::Store`]: runs, events, agents, and daemon metadata.
pub mod state;
mod store_rows;
pub mod tmux;
pub mod transcript;
/// Domain types shared by every binary: agents, runs, events, and their enums.
pub mod types;

pub use error::CoreError;
