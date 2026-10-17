//! Core domain types for Tandem: pull requests, diffs, local review
//! comments, and agent run specifications.
//!
//! This crate is dependency-light on purpose: every other crate in the
//! workspace (GitHub client, cache, agent runner, Tauri app) speaks these
//! types, so nothing heavier than serde/chrono belongs here.

pub mod agent;
pub mod diff;
pub mod diff_parse;
pub mod error;
pub mod github;
pub mod review;

pub use error::TandemError;

/// Convenience result alias used across the workspace.
pub type Result<T> = std::result::Result<T, TandemError>;
