//! Home directory, XDG base directories, and `~` expansion.
//!
//! Every function that reads the process environment has a pure `_from`
//! twin that takes the variable values as arguments, so callers and tests
//! can exercise the resolution rules without mutating process-global state.
//! Empty variables are treated exactly like unset ones throughout.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// `$HOME` as a path, or `None` when it is unset or empty.
pub fn home_dir() -> Option<PathBuf> {
    env_path("HOME")
}

/// The environment variable `var` as a path, or `None` when it is unset or
/// set to the empty string.
pub fn env_path(var: &str) -> Option<PathBuf> {
    non_empty_path(std::env::var_os(var).as_deref())
}

/// `value` as a path when it is present and non-empty — the pure core of
/// [`env_path`].
pub fn non_empty_path(value: Option<&OsStr>) -> Option<PathBuf> {
    value.filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `$XDG_CONFIG_HOME`, falling back to `~/.config`. `None` only when both
/// that variable and `$HOME` are unset or empty.
pub fn xdg_config_home() -> Option<PathBuf> {
    xdg_config_home_from(env("XDG_CONFIG_HOME").as_deref(), env("HOME").as_deref())
}

/// Pure form of [`xdg_config_home`] taking the raw `$XDG_CONFIG_HOME` and
/// `$HOME` values.
pub fn xdg_config_home_from(xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    xdg_or_home(xdg, home, ".config")
}

/// `$XDG_STATE_HOME`, falling back to `~/.local/state`. `None` only when
/// both that variable and `$HOME` are unset or empty.
pub fn xdg_state_home() -> Option<PathBuf> {
    xdg_state_home_from(env("XDG_STATE_HOME").as_deref(), env("HOME").as_deref())
}

/// Pure form of [`xdg_state_home`] taking the raw `$XDG_STATE_HOME` and
/// `$HOME` values.
pub fn xdg_state_home_from(xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    xdg_or_home(xdg, home, ".local/state")
}

/// `$XDG_CACHE_HOME`, falling back to `~/.cache`. `None` only when both that
/// variable and `$HOME` are unset or empty.
pub fn xdg_cache_home() -> Option<PathBuf> {
    xdg_cache_home_from(env("XDG_CACHE_HOME").as_deref(), env("HOME").as_deref())
}

/// Pure form of [`xdg_cache_home`] taking the raw `$XDG_CACHE_HOME` and
/// `$HOME` values.
pub fn xdg_cache_home_from(xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    xdg_or_home(xdg, home, ".cache")
}

/// `$XDG_DATA_HOME`, falling back to `~/.local/share`. `None` only when both
/// that variable and `$HOME` are unset or empty.
pub fn xdg_data_home() -> Option<PathBuf> {
    xdg_data_home_from(env("XDG_DATA_HOME").as_deref(), env("HOME").as_deref())
}

/// Pure form of [`xdg_data_home`] taking the raw `$XDG_DATA_HOME` and
/// `$HOME` values.
pub fn xdg_data_home_from(xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    xdg_or_home(xdg, home, ".local/share")
}

/// `$XDG_RUNTIME_DIR` when set and non-empty. The spec defines no fallback,
/// so there is none: callers pick their own (e.g. a temp dir) on `None`.
pub fn xdg_runtime_dir() -> Option<PathBuf> {
    env_path("XDG_RUNTIME_DIR")
}

/// Expands a leading `~` component to `$HOME`.
///
/// `~` and `~/rest` are expanded; `~user/...`, paths with `~` elsewhere, and
/// every path when `$HOME` is unset or empty are returned unchanged.
pub fn expand_tilde(path: &Path) -> PathBuf {
    expand_tilde_with(path, env("HOME").as_deref())
}

/// Pure form of [`expand_tilde`]: expands a leading `~` component to `home`.
///
/// An empty `home` is treated like `None` (path returned unchanged). `~`
/// alone becomes exactly `home`, without a trailing separator.
pub fn expand_tilde_with(path: &Path, home: Option<&OsStr>) -> PathBuf {
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return path.to_path_buf();
    };
    // `strip_prefix` is component-wise, so `~user` does not match `~`.
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    if rest.as_os_str().is_empty() {
        PathBuf::from(home)
    } else {
        Path::new(home).join(rest)
    }
}

fn env(var: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(var)
}

fn xdg_or_home(xdg: Option<&OsStr>, home: Option<&OsStr>, fallback: &str) -> Option<PathBuf> {
    non_empty_path(xdg).or_else(|| non_empty_path(home).map(|h| h.join(fallback)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> &OsStr {
        OsStr::new(s)
    }

    #[test]
    fn non_empty_path_filters_empty_and_unset() {
        assert_eq!(non_empty_path(None), None);
        assert_eq!(non_empty_path(Some(os(""))), None);
        assert_eq!(non_empty_path(Some(os("/x"))), Some(PathBuf::from("/x")));
    }

    #[test]
    fn env_path_is_none_for_unset_var() {
        assert_eq!(env_path("BEAGLE_UTILS_TEST_VAR_THAT_IS_NEVER_SET"), None);
    }

    #[test]
    fn xdg_var_wins_when_non_empty() {
        let got = xdg_config_home_from(Some(os("/xdg/cfg")), Some(os("/home/u")));
        assert_eq!(got, Some(PathBuf::from("/xdg/cfg")));
    }

    #[test]
    fn xdg_falls_back_to_home_when_empty_or_unset() {
        let home = Some(os("/home/u"));
        assert_eq!(
            xdg_config_home_from(Some(os("")), home),
            Some(PathBuf::from("/home/u/.config"))
        );
        assert_eq!(
            xdg_state_home_from(None, home),
            Some(PathBuf::from("/home/u/.local/state"))
        );
        assert_eq!(
            xdg_cache_home_from(None, home),
            Some(PathBuf::from("/home/u/.cache"))
        );
        assert_eq!(
            xdg_data_home_from(None, home),
            Some(PathBuf::from("/home/u/.local/share"))
        );
    }

    #[test]
    fn xdg_is_none_without_var_or_home() {
        assert_eq!(xdg_config_home_from(None, None), None);
        assert_eq!(xdg_data_home_from(Some(os("")), Some(os(""))), None);
    }

    #[test]
    fn tilde_alone_is_home() {
        let got = expand_tilde_with(Path::new("~"), Some(os("/home/u")));
        assert_eq!(got, PathBuf::from("/home/u"));
    }

    #[test]
    fn tilde_slash_joins_rest() {
        let got = expand_tilde_with(Path::new("~/a/b"), Some(os("/home/u")));
        assert_eq!(got, PathBuf::from("/home/u/a/b"));
    }

    #[test]
    fn tilde_user_and_non_leading_tilde_unchanged() {
        let home = Some(os("/home/u"));
        assert_eq!(
            expand_tilde_with(Path::new("~bob/x"), home),
            PathBuf::from("~bob/x")
        );
        assert_eq!(
            expand_tilde_with(Path::new("/a/~/b"), home),
            PathBuf::from("/a/~/b")
        );
        assert_eq!(
            expand_tilde_with(Path::new("rel/x"), home),
            PathBuf::from("rel/x")
        );
    }

    #[test]
    fn tilde_unchanged_without_home() {
        assert_eq!(
            expand_tilde_with(Path::new("~/x"), None),
            PathBuf::from("~/x")
        );
        assert_eq!(
            expand_tilde_with(Path::new("~/x"), Some(os(""))),
            PathBuf::from("~/x")
        );
    }
}
