//! Child-process environment helpers.
//!
//! A packaged macOS app launched from Finder inherits the minimal system
//! PATH (`/usr/bin:/bin:...`) — Homebrew and user-local tools like `gh`,
//! `claude`, `codex`, and `grok` are invisible to it, which breaks every
//! spawn that works fine from a terminal. Prepending the standard install
//! locations (and the login shell's PATH) fixes that without affecting runs
//! that already have a full PATH.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// PATH from the user's login shell (`$SHELL -lc`, default `/bin/zsh`), so
/// tools installed via nvm, asdf, etc. are found too.
///
/// Spawns the shell on the first call only; the result (empty if the shell
/// failed) is cached for the life of the process.
pub fn login_shell_path() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(|| {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        Command::new(shell)
            .args(["-lc", "printf %s \"$PATH\""])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    })
}

/// The current `$PATH` with common tool locations prepended and the login
/// shell's PATH appended, deduplicated in first-seen order.
///
/// Order: `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`, `~/bin`,
/// then `$PATH`, then [`login_shell_path`]. Empty entries are dropped.
pub fn augmented_path() -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    augmented_path_from(&current, &home, login_shell_path())
}

/// Pure form of [`augmented_path`] taking the current PATH, `$HOME`, and the
/// login-shell PATH as strings.
pub fn augmented_path_from(current: &str, home: &str, login: &str) -> String {
    let extras = [
        "/opt/homebrew/bin".to_owned(),
        "/usr/local/bin".to_owned(),
        format!("{home}/.local/bin"),
        format!("{home}/bin"),
    ];
    let mut parts: Vec<String> = Vec::new();
    for p in extras
        .into_iter()
        .chain(current.split(':').map(str::to_owned))
        .chain(login.split(':').map(str::to_owned))
    {
        if !p.is_empty() && !parts.contains(&p) {
            parts.push(p);
        }
    }
    parts.join(":")
}

/// A [`Command`] for `program` whose child sees `PATH=`[`augmented_path`].
///
/// Note the *lookup of `program` itself* is done by the OS with the parent's
/// PATH; pass the result of [`which`] when the parent PATH may be minimal.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.env("PATH", augmented_path());
    cmd
}

/// Finds `program` on [`augmented_path`], returning the first executable
/// match. Does not spawn anything beyond the cached login-shell probe.
pub fn which(program: impl AsRef<OsStr>) -> Option<PathBuf> {
    which_in(program.as_ref(), OsStr::new(&augmented_path()))
}

/// Pure form of [`which`]: scans `path_var` (split with
/// [`std::env::split_paths`], i.e. `:` on unix) for an
/// executable regular file named `program`.
///
/// A `program` containing a path separator is checked as-is instead of being
/// searched for. On unix "executable" means any execute bit is set; elsewhere
/// any regular file counts.
pub fn which_in(program: impl AsRef<OsStr>, path_var: &OsStr) -> Option<PathBuf> {
    let program = Path::new(program.as_ref());
    if program.as_os_str().is_empty() {
        return None;
    }
    if program.components().count() > 1 {
        return is_executable(program).then(|| program.to_path_buf());
    }
    std::env::split_paths(path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way to do that.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn prepends_extras_and_dedupes() {
        let path = augmented_path_from("/usr/bin:/opt/homebrew/bin:", "/h", "/bin:/usr/bin");
        assert_eq!(
            path,
            "/opt/homebrew/bin:/usr/local/bin:/h/.local/bin:/h/bin:/usr/bin:/bin"
        );
    }

    #[test]
    fn real_augmented_path_starts_with_homebrew() {
        let path = augmented_path();
        assert!(path.starts_with("/opt/homebrew/bin"));
        assert_eq!(path.matches("/opt/homebrew/bin:").count(), 1);
    }

    #[test]
    fn command_sets_path_env() {
        let cmd = command("true");
        let path = cmd
            .get_envs()
            .find(|(k, _)| *k == OsStr::new("PATH"))
            .and_then(|(_, v)| v);
        assert_eq!(path, Some(OsStr::new(&augmented_path())));
    }

    #[cfg(unix)]
    fn touch(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::write(path, b"").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn which_in_finds_first_executable() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        touch(&a.path().join("tool"), 0o644); // not executable: skipped
        touch(&b.path().join("tool"), 0o755);
        let path_var = std::env::join_paths([a.path(), b.path()]).unwrap();
        assert_eq!(which_in("tool", &path_var), Some(b.path().join("tool")));
    }

    #[cfg(unix)]
    #[test]
    fn which_in_ignores_dirs_and_missing() {
        let a = tempfile::tempdir().unwrap();
        std::fs::create_dir(a.path().join("sub")).unwrap();
        let path_var = a.path().as_os_str();
        assert_eq!(which_in("sub", path_var), None);
        assert_eq!(which_in("missing", path_var), None);
        assert_eq!(which_in("", path_var), None);
    }

    #[cfg(unix)]
    #[test]
    fn which_in_checks_paths_with_separator_directly() {
        let a = tempfile::tempdir().unwrap();
        let tool = a.path().join("tool");
        touch(&tool, 0o700);
        assert_eq!(which_in(&tool, OsStr::new("")), Some(tool.clone()));
        touch(&tool, 0o600);
        assert_eq!(which_in(&tool, OsStr::new("")), None);
    }
}
