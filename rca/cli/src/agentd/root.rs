//! Which RCA store the daemon works on — the same answer `beagle rca` gives.
//!
//! launchd and systemd start `beagle-agentd` with no shell around it, so the
//! daemon cannot lean on "whatever directory you ran it from". It resolves
//! the store the way the CLI does, so both always look at the same RCAs:
//!
//! 1. `$BEAGLE_AGENT_RCA_ROOT`, when set
//! 2. `root` from the effective config: the nearest `.beagle` at or above
//!    the working directory, else `~/.config/beagle/config.toml`
//! 3. the nearest directory at or above the working directory with `rcas/`
//!    (else the working directory itself)
//!
//! `beagle rca agent install` sets the service's working directory to the
//! store the CLI resolves at install time, so step 3 lands on it too.

use std::path::{Path, PathBuf};

/// The rule above, with every input passed in (no environment or disk
/// reads beyond `discover_root`'s look for `rcas/`).
#[must_use]
pub fn store_root_from(
    env_root: Option<PathBuf>,
    config_root: Option<PathBuf>,
    cwd: &Path,
) -> PathBuf {
    env_root
        .or(config_root)
        .unwrap_or_else(|| crate::store::discover_root(cwd))
}

/// The daemon's store, read from this process's environment, config and
/// working directory.
///
/// # Errors
/// When the working directory cannot be read or the config file is broken.
pub fn store_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| format!("resolving current dir: {err}"))?;
    let config_root = crate::config::load_effective(&cwd)
        .map_err(|err| format!("reading beagle config: {err}"))?
        .and_then(|config| config.root);
    Ok(store_root_from(
        beagle_utils::paths::env_path("BEAGLE_AGENT_RCA_ROOT"),
        config_root,
        &cwd,
    ))
}
