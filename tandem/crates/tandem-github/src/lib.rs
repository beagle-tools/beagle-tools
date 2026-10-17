//! GitHub REST client for Tandem.
//!
//! Reads power the whole app. Writes exist ONLY for explicit user
//! actions (post a chosen comment / approve); agents never get a path
//! to them and review comments stay local by default.

pub mod auth;
pub mod client;
mod discovery;
mod rate_limit;
mod search;
mod wire;
mod write;

pub use wire::{MergeOptions, RepoSummary};

pub use auth::{GithubAuth, GithubConfig};
pub use client::GithubClient;
pub use write::NewInlineComment;

pub use discovery::PrCommit;
