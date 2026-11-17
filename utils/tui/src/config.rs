//! Finding an app's store root and reading its config file.
//!
//! Each app stores its data in one directory (`recaps/`, `standups/`, …)
//! under a *store root*. [`AppDirs`] names the app, its environment variable
//! and that data directory; the root is then resolved in this order:
//!
//! 1. the `--root` flag,
//! 2. `$<APP>_ROOT`,
//! 3. `root` in `~/.config/<app>/config.toml` (`$XDG_CONFIG_HOME` honoured),
//! 4. the nearest ancestor of the working directory containing the data dir,
//! 5. the working directory.
//!
//! A leading `~` is expanded in the first three.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::error::ConfigError;

/// The names that locate one app's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppDirs {
    /// The app's name: the config directory under `~/.config/`.
    pub app: &'static str,
    /// The environment variable that overrides the store root.
    pub env_var: &'static str,
    /// The data directory under the store root (`recaps`, `standups`).
    pub data_dir: &'static str,
}

impl AppDirs {
    /// `~/.config/<app>/config.toml` (or under `$XDG_CONFIG_HOME`).
    #[must_use]
    pub fn config_path(&self) -> Option<PathBuf> {
        beagle_utils::paths::xdg_config_home().map(|dir| dir.join(self.app).join("config.toml"))
    }

    /// The env var's value, when set and non-empty.
    #[must_use]
    pub fn env_root(&self) -> Option<String> {
        std::env::var(self.env_var).ok().filter(|s| !s.is_empty())
    }

    /// Picks the store root from the flag, environment and config value,
    /// falling back to discovery from `cwd` (see the module docs).
    #[must_use]
    pub fn resolve_root(
        &self,
        flag: Option<&str>,
        env: Option<&str>,
        config_root: Option<&str>,
        cwd: &Path,
    ) -> PathBuf {
        match flag.or(env).or(config_root) {
            Some(chosen) => expand_tilde(chosen),
            None => self.discover_root(cwd),
        }
    }

    /// The nearest ancestor of `start` (itself included) that contains the
    /// data directory, or `start` when none does.
    #[must_use]
    pub fn discover_root(&self, start: &Path) -> PathBuf {
        start
            .ancestors()
            .find(|dir| dir.join(self.data_dir).is_dir())
            .unwrap_or(start)
            .to_path_buf()
    }
}

/// Loads a TOML config file; a missing file is `T::default()`.
///
/// # Errors
/// [`ConfigError::Io`] when the file can't be read, [`ConfigError::Parse`]
/// when it doesn't match `T` (use `deny_unknown_fields` so typos fail).
pub fn load_toml<T: DeserializeOwned + Default>(path: &Path) -> Result<T, ConfigError> {
    match beagle_utils::fs::read_optional(path) {
        Ok(Some(text)) => toml::from_str(&text).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            source: Box::new(e),
        }),
        Ok(None) => Ok(T::default()),
        Err(e) => Err(ConfigError::Io {
            path: path.to_owned(),
            source: e,
        }),
    }
}

/// Expands a leading `~/` (or a bare `~`) to `$HOME`.
#[must_use]
pub fn expand_tilde(path: &str) -> PathBuf {
    beagle_utils::paths::expand_tilde(Path::new(path))
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
