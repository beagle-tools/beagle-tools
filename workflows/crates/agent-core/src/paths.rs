//! Where agentd keeps its state and finds its manifests.
//!
//! Environment resolution (empty variables count as unset, XDG fallbacks)
//! comes from [`beagle_utils::paths`]; this module only adds the agentd
//! layout on top and turns a missing `$HOME` into a [`CoreError`].

use std::path::{Path, PathBuf};

use beagle_utils::paths::{env_path, xdg_state_home};

use crate::CoreError;

/// Root for mutable daemon state (db, run logs, worktrees).
///
/// Resolution order: `$AGENTD_STATE_DIR`, then `$XDG_STATE_HOME/agentd`,
/// then `~/.local/state/agentd`.
pub fn state_dir() -> Result<PathBuf, CoreError> {
    if let Some(dir) = env_path("AGENTD_STATE_DIR") {
        return Ok(dir);
    }
    xdg_state_home()
        .map(|dir| dir.join("agentd"))
        .ok_or_else(home_missing)
}

/// Where agent manifests live — the desired state.
///
/// Resolution order: `$AGENTD_MANIFESTS_DIR`, then `~/.config/agentd/manifests`
/// (`scripts/agentman` links manifest roots there).
pub fn manifests_dir() -> Result<PathBuf, CoreError> {
    if let Some(dir) = env_path("AGENTD_MANIFESTS_DIR") {
        return Ok(dir);
    }
    Ok(home_dir()?.join(".config/agentd/manifests"))
}

/// The `SQLite` store every agentd binary shares: `state_dir()/state.db`.
pub fn db_path() -> Result<PathBuf, CoreError> {
    Ok(state_dir()?.join("state.db"))
}

/// Per-run log files (`<run id>.log`), pruned by the daemon's retention.
pub fn logs_dir() -> Result<PathBuf, CoreError> {
    Ok(state_dir()?.join("logs"))
}

/// Git worktrees created for agents with `isolation: worktree`.
pub fn worktrees_dir() -> Result<PathBuf, CoreError> {
    Ok(state_dir()?.join("worktrees"))
}

/// Per-run scratch directories for passing data between DAG jobs: a run
/// writes to `data_dir()/<its id>` (exposed as `$AGENTD_DATA_DIR`), children
/// read it via `$AGENTD_PARENT_DATA_DIR`.
pub fn data_dir() -> Result<PathBuf, CoreError> {
    Ok(state_dir()?.join("data"))
}

/// Create the state directories if they don't exist yet.
pub fn ensure_state_dirs() -> Result<(), CoreError> {
    for dir in [state_dir()?, logs_dir()?, worktrees_dir()?, data_dir()?] {
        std::fs::create_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;
    }
    Ok(())
}

/// Expand a leading `~` or `~/` to the user's home directory.
///
/// `~user/...` and paths with `~` anywhere else come back unchanged. Errors
/// only when the path actually needs `$HOME` and it is unset or empty.
pub fn expand_tilde(path: &Path) -> Result<PathBuf, CoreError> {
    if path.starts_with("~") {
        home_dir()?;
    }
    Ok(beagle_utils::paths::expand_tilde(path))
}

fn home_dir() -> Result<PathBuf, CoreError> {
    beagle_utils::paths::home_dir().ok_or_else(home_missing)
}

fn home_missing() -> CoreError {
    CoreError::Other("HOME is not set".to_owned())
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion() {
        let home = home_dir().unwrap();
        assert_eq!(expand_tilde(Path::new("~/x")).unwrap(), home.join("x"));
        assert_eq!(expand_tilde(Path::new("~")).unwrap(), home);
        assert_eq!(
            expand_tilde(Path::new("/abs/x")).unwrap(),
            PathBuf::from("/abs/x")
        );
    }
}
