//! The `beagle` base CLI: installs, updates, pins and removes Beagle
//! extensions, dispatches `beagle <ext> …` to them, and updates itself.
//!
//! Synchronous throughout: one process, blocking child processes with
//! timeouts. `main.rs` builds a [`Context`] from the environment and hands
//! argv to [`run`]; everything else takes its paths from that context, so
//! tests run the whole CLI against tempdirs and `file://` release fixtures.

pub mod cli;
mod commands;
pub mod dispatch;
pub mod doctor;
pub mod error;
pub mod fuzzy;
pub mod install;
pub mod manifest;
pub mod paths;
pub mod picker;
pub mod registry;
pub mod releases;
pub mod state;
pub mod tool;
pub mod version;

#[cfg(test)]
#[path = "tests/fixture.rs"]
mod test_fixture;

use std::ffi::OsString;
use std::io::{IsTerminal as _, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

pub use commands::run;
pub use error::{Error, Result};

use paths::Layout;
use registry::ExtensionSpec;
use releases::ReleaseSource;

/// Everything a command reads from its environment.
#[derive(Debug, Clone)]
pub struct Context<'a> {
    /// The extensions this base knows.
    pub registry: &'a [ExtensionSpec],
    /// Where extensions are installed.
    pub layout: Layout,
    /// Where releases come from.
    pub source: ReleaseSource,
    /// This platform's release target; `None` where nothing is built.
    pub target: Option<String>,
    /// `$PATH`.
    pub path_var: OsString,
    /// This binary's path.
    pub exe: PathBuf,
    /// `$HOME`.
    pub home: Option<PathBuf>,
    /// stdin and stderr are terminals: the version picker may run.
    pub interactive: bool,
    /// "Update available" notices may be printed before dispatch.
    pub notices: bool,
    /// How long to wait for another install of the same extension.
    pub lock_wait: Duration,
}

impl Context<'static> {
    /// The real environment: the compiled-in registry, `$XDG_DATA_HOME`,
    /// GitHub releases, this process's TTYs and env.
    pub fn from_env() -> Result<Self> {
        let env = |key: &str| std::env::var(key).ok();
        let stderr_tty = std::io::stderr().is_terminal();
        Ok(Self {
            registry: registry::REGISTRY,
            layout: Layout::from_env()?,
            source: ReleaseSource::from_env(),
            target: registry::release_target().map(str::to_owned),
            path_var: std::env::var_os("PATH").unwrap_or_default(),
            exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("beagle")),
            home: beagle_utils::paths::home_dir(),
            interactive: stderr_tty && std::io::stdin().is_terminal(),
            notices: stderr_tty
                && state::notices_enabled(
                    env("BEAGLE_NO_UPDATE_CHECK").as_deref(),
                    env("CI").as_deref(),
                ),
            lock_wait: install::lock::WAIT,
        })
    }
}

/// What `main` must do after [`run`].
#[derive(Debug)]
pub enum Outcome {
    /// Exit with this code.
    Exit(u8),
    /// Replace the process with an extension.
    Exec(Box<Dispatch>),
}

/// A resolved extension, ready to exec.
#[derive(Debug)]
pub struct Dispatch {
    /// The prepared command, env included.
    pub command: Command,
    /// What it resolved to, for exec-failure messages.
    pub resolved: dispatch::Resolved,
}

/// Output sink for commands (stdout in `main`, a buffer in tests).
pub type Out<'w> = &'w mut dyn Write;
