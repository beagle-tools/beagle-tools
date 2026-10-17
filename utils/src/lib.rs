//! Small helpers shared by the beagle-tools apps (`tandem`, `rca`,
//! `workflows`).
//!
//! Everything here is std-only unless a feature says otherwise; the `log`
//! module needs the `tracing` feature. Only code used by two or more apps
//! belongs in this crate, and app-specific error types stay in the apps.

pub mod duration;
pub mod fs;
#[cfg(feature = "tracing")]
pub mod log;
pub mod notify;
pub mod paths;
pub mod process;
pub mod shell;
pub mod text;
