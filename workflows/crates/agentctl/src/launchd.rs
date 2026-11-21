//! The pure half of `agentctl daemon`: everything a brew install needs that
//! used to come from the repo checkout, embedded at build time, plus the
//! rendering and parsing rules. No I/O here, so all of it is unit-tested;
//! [`crate::daemon`] does the filesystem and `launchctl` work.

use std::path::{Path, PathBuf};

/// launchd label of the agentd unit (also the plist's file stem).
pub const LABEL: &str = "com.agentd.daemon";

/// The launchd unit template. `deploy/` is the single source of truth; the
/// binary carries a copy so it works without the repo.
const PLIST_TEMPLATE: &str = include_str!("../../../deploy/com.agentd.daemon.plist");

/// `scripts/build-notifier.sh`, run with `AGENTD_NOTIFY_ICONS_DIR` pointing
/// at the unpacked [`NOTIFY_ICONS`].
pub const BUILD_NOTIFIER_SH: &str = include_str!("../../../scripts/build-notifier.sh");

/// The stock notification icons (`assets/notify/*.png`), by file name. One
/// branded notifier app is built per entry.
pub const NOTIFY_ICONS: &[(&str, &[u8])] = &[
    (
        "agentd.png",
        include_bytes!("../../../assets/notify/agentd.png"),
    ),
    (
        "agentd-fail.png",
        include_bytes!("../../../assets/notify/agentd-fail.png"),
    ),
];

/// What the launchd unit runs: a binary plus its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The binary, at a path that survives upgrades.
    pub bin: PathBuf,
    /// Its arguments (`agent daemon run` for the unified `beagle`).
    pub args: Vec<String>,
}

/// The arguments that make the unified `beagle` binary run the daemon. The
/// process lookup in `agent_core::daemon` matches on them, so keep the two
/// in step.
pub const HOSTED_DAEMON_ARGS: [&str; 3] = ["agent", "daemon", "run"];

/// Render the launchd unit running `program`, logging into `log_dir`. The
/// binary's directory goes first on the unit's PATH, so runs find the
/// control CLI (and its siblings) from the same install.
pub fn render_plist(program: &Program, log_dir: &Path) -> String {
    let bin_dir = program.bin.parent().unwrap_or_else(|| Path::new("/"));
    let arguments: Vec<String> = std::iter::once(program.bin.to_string_lossy().into_owned())
        .chain(program.args.iter().cloned())
        .map(|arg| format!("    <string>{}</string>", xml_escape(&arg)))
        .collect();
    PLIST_TEMPLATE
        .replace("__PROGRAM_ARGUMENTS__", &arguments.join("\n"))
        .replace("__CARGO_BIN_DIR__", &xml_escape(&bin_dir.to_string_lossy()))
        .replace("__LOG_DIR__", &xml_escape(&log_dir.to_string_lossy()))
}

/// The unit's program when this code runs inside the unified `beagle`
/// binary at `exe`: that same binary (at its upgrade-proof path), told to
/// run the daemon.
pub fn hosted_program(exe: &Path) -> Program {
    Program {
        bin: beagle_utils::paths::stable_exe(exe),
        args: HOSTED_DAEMON_ARGS.iter().map(|a| (*a).to_owned()).collect(),
    }
}

/// Escape the characters that are not allowed verbatim in XML text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Which `agentd` the unit should run: the one installed next to this
/// `agentctl` (`exe`), so both always come from the same install, else the
/// one on PATH (`on_path`). `exists` is the file check, injected for tests.
pub fn resolve_agentd(
    exe: Option<&Path>,
    on_path: Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let sibling = exe
        .and_then(Path::parent)
        .map(|dir| beagle_utils::paths::stable_bin_dir(dir).join("agentd"))
        .filter(|p| exists(p));
    sibling.or(on_path)
}

/// The pid in `launchctl print gui/<uid>/<label>` output (`pid = 123`), or
/// `None` when the unit is loaded but not running.
pub fn parse_pid(print_output: &str) -> Option<u32> {
    print_output.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("pid")?.trim_start();
        rest.strip_prefix('=')?.trim().parse().ok()
    })
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn standalone(bin: &str) -> Program {
        Program {
            bin: PathBuf::from(bin),
            args: Vec::new(),
        }
    }

    #[test]
    fn rendering_fills_every_placeholder() {
        let out = render_plist(
            &standalone("/opt/homebrew/bin/agentd"),
            Path::new("/Users/me/Library/Logs/agentd"),
        );
        assert!(!out.contains("__"), "unrendered placeholder in:\n{out}");
        assert!(out.contains(&format!("<string>{LABEL}</string>")));
        assert!(out.contains("<array>\n    <string>/opt/homebrew/bin/agentd</string>\n  </array>"));
        assert!(out.contains("<string>/opt/homebrew/bin:/opt/homebrew/bin:"));
        assert!(out.contains("/Users/me/Library/Logs/agentd/agentd.err.log"));
        assert!(out.trim_start().starts_with("<?xml"));
        assert!(out.trim_end().ends_with("</plist>"));
    }

    #[test]
    fn hosted_units_run_beagle_agent_daemon_run() {
        let program = hosted_program(Path::new("/opt/homebrew/Cellar/beagle/1.0.0/bin/beagle"));
        assert_eq!(program.bin, PathBuf::from("/opt/homebrew/bin/beagle"));
        let out = render_plist(&program, Path::new("/logs"));
        assert!(out.contains(
            "    <string>/opt/homebrew/bin/beagle</string>\n    <string>agent</string>\n    \
             <string>daemon</string>\n    <string>run</string>\n  </array>"
        ));
        assert!(out.contains("<string>/opt/homebrew/bin:/opt/homebrew/bin:"));
        // agent_core::daemon finds a heartbeat-less daemon by this command line.
        assert_eq!(HOSTED_DAEMON_ARGS.join(" "), "agent daemon run");
    }

    #[test]
    fn rendering_escapes_xml_metacharacters() {
        let out = render_plist(&standalone("/a&b/<x>/agentd"), Path::new("/logs"));
        assert!(out.contains("<string>/a&amp;b/&lt;x&gt;/agentd</string>"));
        assert!(!out.contains("/a&b/"));
    }

    #[test]
    fn agentd_prefers_the_sibling_then_path() {
        let exe = Path::new("/Users/me/.cargo/bin/agentctl");
        let on_path = Some(PathBuf::from("/usr/local/bin/agentd"));
        assert_eq!(
            resolve_agentd(Some(exe), on_path.clone(), |_| true),
            Some(PathBuf::from("/Users/me/.cargo/bin/agentd"))
        );
        assert_eq!(
            resolve_agentd(Some(exe), on_path.clone(), |_| false),
            on_path
        );
        assert_eq!(resolve_agentd(None, None, |_| true), None);
        let keg = Path::new("/opt/homebrew/Cellar/beagle-workflows/1.2.3/bin/agentctl");
        assert_eq!(
            resolve_agentd(Some(keg), None, |p| p
                == Path::new("/opt/homebrew/bin/agentd")),
            Some(PathBuf::from("/opt/homebrew/bin/agentd"))
        );
    }

    #[test]
    fn pid_parsing() {
        let out = "gui/501/com.agentd.daemon = {\n\tstate = running\n\tpid = 4242\n}";
        assert_eq!(parse_pid(out), Some(4242));
        assert_eq!(parse_pid("\tstate = not running\n"), None);
        assert_eq!(parse_pid("\tpid-namespace = 3\n"), None);
    }

    fn is_png(name: &str) -> bool {
        Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    }

    #[test]
    fn embedded_assets_are_present() {
        assert_ne!(NOTIFY_ICONS.len(), 0);
        for (name, bytes) in NOTIFY_ICONS {
            assert!(is_png(name), "{name}");
            assert!(bytes.starts_with(b"\x89PNG"), "{name} is not a png");
        }
        assert!(BUILD_NOTIFIER_SH.contains("AGENTD_NOTIFY_ICONS_DIR"));
    }

    #[test]
    fn every_png_in_assets_is_embedded() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/notify");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| is_png(n))
            .collect();
        on_disk.sort();
        let mut embedded: Vec<String> = NOTIFY_ICONS.iter().map(|(n, _)| (*n).to_owned()).collect();
        embedded.sort();
        assert_eq!(on_disk, embedded, "add the new png to NOTIFY_ICONS");
    }
}
