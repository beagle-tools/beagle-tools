//! rca in the one beagle folder (`docs/CODING_STANDARDS.md` rule 15):
//! the config is `~/.config/beagle/rca.toml`, the older
//! `~/.config/beagle/config.toml` still works when it is the only one, and
//! `$BEAGLE_RCA_ROOT` names the store. The names live in [`DIRS`]; the
//! rules in [`beagle_tui::config`].

use std::path::PathBuf;

use beagle_tui::config::AppDirs;

use super::CONFIG_ENV;

/// rca's names for the shared resolver: `beagle/rca.toml` (older
/// `beagle/config.toml`), `$BEAGLE_RCA_ROOT`, the `.beagle` marker and the
/// `rcas/` data directory.
pub const DIRS: AppDirs = AppDirs {
    app: "rca",
    legacy_env: &[],
    legacy_configs: &["beagle/config.toml"],
    marker: Some(super::PROJECT_FILE),
    data_dir: Some(crate::store::RCAS_DIR),
};

/// Where the config file lives: `$BEAGLE_CONFIG` if set, else
/// `$XDG_CONFIG_HOME/beagle/rca.toml` (so `~/.config/beagle/rca.toml`),
/// else the older `~/.config/beagle/config.toml` when only that exists,
/// else `.beagle.toml` in the current directory as a last resort.
#[must_use]
pub fn path() -> PathBuf {
    if let Some(explicit) = std::env::var_os(CONFIG_ENV) {
        return PathBuf::from(explicit);
    }
    DIRS.config_file()
        .unwrap_or_else(|| PathBuf::from(".beagle.toml"))
}

/// `$BEAGLE_RCA_ROOT`, when set and non-empty.
#[must_use]
pub fn env_root() -> Option<PathBuf> {
    DIRS.env_root()
        .map(|r| beagle_tui::config::expand_tilde(&r))
}
