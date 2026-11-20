//! Finding a tool's store root and reading its config file: the one beagle
//! folder (`docs/CODING_STANDARDS.md` rule 15).
//!
//! [`AppDirs`] names a tool, its older paths and variables, its marker file
//! and its data directory. Then:
//!
//! - **Config:** `$XDG_CONFIG_HOME/beagle/<tool>.toml` (so
//!   `~/.config/beagle/<tool>.toml`), else the first older file that exists
//!   ([`AppDirs::legacy_configs`]). A file that does not parse warns and the
//!   tool runs on defaults ([`AppDirs::load_config`]); it never refuses to
//!   start.
//! - **Store root**, first match wins ([`AppDirs::find_root`]):
//!   1. the `--root` flag,
//!   2. `$BEAGLE_<TOOL>_ROOT`, then the older variables
//!      ([`AppDirs::legacy_env`]),
//!   3. the nearest ancestor of the working directory holding the tool's
//!      marker file (like `.beagle`),
//!   4. `root` in the config,
//!   5. the nearest ancestor holding the tool's data directory,
//!   6. the tool's default (the working directory, unless the tool has its
//!      own).
//!
//! A leading `~` is expanded in the flag, the environment and the config.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::error::ConfigError;

/// The names that locate one tool's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppDirs {
    /// The tool's name under `beagle` (`recap`): the config is
    /// `beagle/<app>.toml` and the root variable `BEAGLE_<APP>_ROOT`.
    pub app: &'static str,
    /// Older root variables (`RECAP_ROOT`), read after the new one.
    pub legacy_env: &'static [&'static str],
    /// Older config files relative to the config home
    /// (`recap/config.toml`), read when the new one does not exist.
    pub legacy_configs: &'static [&'static str],
    /// A file marking a directory as a store root (`.scout-store`), found
    /// walking up from the working directory.
    pub marker: Option<&'static str>,
    /// The data directory under the store root (`recaps`), found walking
    /// up from the working directory.
    pub data_dir: Option<&'static str>,
}

/// Which step of the order picked a store root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootSource {
    /// The `--root` flag.
    Flag,
    /// `$BEAGLE_<TOOL>_ROOT` or an older variable.
    Env,
    /// The nearest marker file.
    Marker,
    /// `root` in the config file.
    Config,
    /// The nearest ancestor holding the data directory.
    DataDir,
}

impl AppDirs {
    /// `BEAGLE_<APP>_ROOT` (`-` becomes `_`).
    #[must_use]
    pub fn env_var(&self) -> String {
        let app: String = self
            .app
            .chars()
            .map(|c| {
                if c == '-' {
                    '_'
                } else {
                    c.to_ascii_uppercase()
                }
            })
            .collect();
        format!("BEAGLE_{app}_ROOT")
    }

    /// `$XDG_CONFIG_HOME/beagle/<app>.toml`: where the config lives now,
    /// and where a new one is written.
    #[must_use]
    pub fn config_path(&self) -> Option<PathBuf> {
        beagle_utils::paths::xdg_config_home().map(|dir| self.config_path_in(&dir))
    }

    /// [`config_path`](Self::config_path) under an explicit config home.
    #[must_use]
    pub fn config_path_in(&self, config_home: &Path) -> PathBuf {
        config_home
            .join("beagle")
            .join(format!("{}.toml", self.app))
    }

    /// The config file to read under `config_home`: the new path when it
    /// exists, else the first older one that does, else the new path.
    #[must_use]
    pub fn config_file_in(&self, config_home: &Path) -> PathBuf {
        let new = self.config_path_in(config_home);
        if new.is_file() {
            return new;
        }
        self.legacy_configs
            .iter()
            .map(|rel| config_home.join(rel))
            .find(|p| p.is_file())
            .unwrap_or(new)
    }

    /// [`config_file_in`](Self::config_file_in) under the real config home.
    #[must_use]
    pub fn config_file(&self) -> Option<PathBuf> {
        beagle_utils::paths::xdg_config_home().map(|dir| self.config_file_in(&dir))
    }

    /// The root from the environment: `$BEAGLE_<APP>_ROOT`, then each older
    /// variable; unset and empty values are skipped.
    #[must_use]
    pub fn env_root(&self) -> Option<String> {
        std::iter::once(self.env_var())
            .chain(self.legacy_env.iter().map(|v| (*v).to_owned()))
            .find_map(|var| std::env::var(var).ok().filter(|s| !s.is_empty()))
    }

    /// The nearest directory at or above `start` holding the marker FILE
    /// (a directory of that name does not count).
    #[must_use]
    pub fn marked_ancestor(&self, start: &Path) -> Option<PathBuf> {
        let marker = self.marker?;
        start
            .ancestors()
            .find(|dir| dir.join(marker).is_file())
            .map(Path::to_path_buf)
    }

    /// The store root and the step that chose it, or `None` when nothing
    /// matched and the tool's default applies (see the module docs).
    #[must_use]
    pub fn find_root(
        &self,
        flag: Option<&str>,
        env: Option<&str>,
        config_root: Option<&str>,
        cwd: &Path,
    ) -> Option<(PathBuf, RootSource)> {
        if let Some(flag) = flag {
            return Some((expand_tilde(flag), RootSource::Flag));
        }
        if let Some(env) = env {
            return Some((expand_tilde(env), RootSource::Env));
        }
        if let Some(dir) = self.marked_ancestor(cwd) {
            return Some((dir, RootSource::Marker));
        }
        if let Some(root) = config_root {
            return Some((expand_tilde(root), RootSource::Config));
        }
        let data = self.data_dir?;
        cwd.ancestors()
            .find(|dir| dir.join(data).is_dir())
            .map(|dir| (dir.to_path_buf(), RootSource::DataDir))
    }

    /// [`find_root`](Self::find_root), defaulting to `cwd`.
    #[must_use]
    pub fn resolve_root(
        &self,
        flag: Option<&str>,
        env: Option<&str>,
        config_root: Option<&str>,
        cwd: &Path,
    ) -> PathBuf {
        self.find_root(flag, env, config_root, cwd)
            .map_or_else(|| cwd.to_path_buf(), |(root, _)| root)
    }

    /// The nearest ancestor of `start` (itself included) that contains the
    /// data directory, or `start` when none does.
    #[must_use]
    pub fn discover_root(&self, start: &Path) -> PathBuf {
        self.data_dir
            .and_then(|data| start.ancestors().find(|dir| dir.join(data).is_dir()))
            .unwrap_or(start)
            .to_path_buf()
    }

    /// The config ([`config_file`](Self::config_file)), or the defaults when
    /// there is none. A file that cannot be read or parsed gives the
    /// defaults and a warning naming the file and the error.
    #[must_use]
    pub fn load_config<T: DeserializeOwned + Default>(&self) -> (T, Option<String>) {
        match self.config_file() {
            Some(path) => load_toml_or_default(&path),
            None => (T::default(), None),
        }
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

/// [`load_toml`] that never fails: a broken file gives `T::default()` and a
/// warning (`<file>: <error> (using defaults)`) for the caller to show.
#[must_use]
pub fn load_toml_or_default<T: DeserializeOwned + Default>(path: &Path) -> (T, Option<String>) {
    match load_toml(path) {
        Ok(config) => (config, None),
        Err(ConfigError::Parse { path, source }) => (
            T::default(),
            Some(format!(
                "{}: {} (using defaults)",
                path.display(),
                source.message()
            )),
        ),
        Err(e) => (T::default(), Some(format!("{e} (using defaults)"))),
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
