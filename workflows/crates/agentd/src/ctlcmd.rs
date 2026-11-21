//! `$AGENTD_CTL`: the control command a run should call to start another
//! run (`$AGENTD_CTL run demo/report --parent "$AGENTD_RUN_ID"`).
//!
//! The daemon runs as the standalone `agentd`, as `agentctl daemon run`, or
//! as `beagle agent daemon run` inside the unified `beagle` command — and
//! only in the first case is there an `agentctl` binary to rely on. So the
//! daemon works out, from how it was started, the command from the same
//! install, and exports it to every run.

use std::path::Path;
use std::sync::OnceLock;

static CTL: OnceLock<String> = OnceLock::new();

/// Record the control command for this daemon process (first call wins).
pub(crate) fn set(command: String) {
    let _ = CTL.set(command);
}

/// The recorded control command, or plain `agentctl` when the daemon never
/// recorded one (unit tests).
pub(crate) fn get() -> &'static str {
    CTL.get().map_or("agentctl", String::as_str)
}

/// The control command for a daemon started as `program` from `exe`.
///
/// - `agentd` (the standalone daemon) → the `agentctl` next to it when
///   `sibling_exists`, else `agentctl` from PATH;
/// - `<x> … daemon run` → `exe` plus the words between the program name
///   and `daemon run`: `beagle agent daemon run` → `<exe> agent`,
///   `agentctl daemon run` → `<exe>`.
pub(crate) fn command(program: &str, exe: Option<&Path>, sibling_exists: bool) -> String {
    let words: Vec<&str> = program.split_whitespace().collect();
    let hosted = words.len() >= 3 && words[words.len() - 2..] == ["daemon", "run"];
    match exe {
        Some(exe) if hosted => {
            let exe = beagle_utils::paths::stable_exe(exe);
            std::iter::once(exe.to_string_lossy().into_owned())
                .chain(words[1..words.len() - 2].iter().map(|w| (*w).to_owned()))
                .collect::<Vec<_>>()
                .join(" ")
        }
        Some(exe) if sibling_exists => match exe.parent() {
            Some(dir) => beagle_utils::paths::stable_bin_dir(dir)
                .join("agentctl")
                .to_string_lossy()
                .into_owned(),
            None => "agentctl".to_owned(),
        },
        _ => "agentctl".to_owned(),
    }
}

/// The path an `agentctl` next to `exe` would have, for the sibling check.
pub(crate) fn sibling(exe: &Path) -> Option<std::path::PathBuf> {
    exe.parent()
        .map(|dir| beagle_utils::paths::stable_bin_dir(dir).join("agentctl"))
}

#[cfg(test)]
mod tests {
    use super::command;
    use std::path::Path;

    #[test]
    fn hosted_daemons_call_back_into_their_own_binary() {
        let beagle = Path::new("/opt/homebrew/Cellar/beagle/1.0.0/bin/beagle");
        assert_eq!(
            command("beagle agent daemon run", Some(beagle), false),
            "/opt/homebrew/bin/beagle agent"
        );
        let agentctl = Path::new("/Users/me/.cargo/bin/agentctl");
        assert_eq!(
            command("agentctl daemon run", Some(agentctl), false),
            "/Users/me/.cargo/bin/agentctl"
        );
    }

    #[test]
    fn the_standalone_daemon_prefers_its_sibling_agentctl() {
        let agentd = Path::new("/Users/me/.cargo/bin/agentd");
        assert_eq!(
            command("agentd", Some(agentd), true),
            "/Users/me/.cargo/bin/agentctl"
        );
        assert_eq!(command("agentd", Some(agentd), false), "agentctl");
        assert_eq!(command("agentd", None, true), "agentctl");
        assert_eq!(command("beagle agent daemon run", None, false), "agentctl");
    }
}
