//! Small helpers shared by the beagle-tools apps (`tandem`, `rca`,
//! `workflows`).
//!
//! Everything here is std-only unless a feature says otherwise: the `log`
//! module needs the `tracing` feature, `gh` (the GitHub CLI) the `gh`
//! feature, and `update` (self-update against GitHub releases) the `update`
//! feature. Only code used by two or more apps
//! belongs in this crate, and app-specific error types stay in the apps.

pub mod agent_env;
pub mod duration;
pub mod exposure;
pub mod fs;
#[cfg(feature = "gh")]
pub mod gh;
#[cfg(feature = "tracing")]
pub mod log;
pub mod notify;
pub mod paths;
pub mod process;
pub mod shell;
pub mod text;
#[cfg(feature = "update")]
pub mod update;
