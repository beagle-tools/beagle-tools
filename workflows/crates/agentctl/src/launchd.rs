//! The pure half of `agentctl daemon`: everything a brew install needs that
//! used to come from the repo checkout, embedded at build time, plus the
//! rendering and parsing rules. No I/O here, so all of it is unit-tested;
//! [`crate::daemon`] does the filesystem and `launchctl` work.

use std::path::{Component, Path, PathBuf};

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

/// Render the launchd unit for an `agentd` at `agentd_bin`, logging into
/// `log_dir`. The binary's directory goes first on the unit's PATH, so
/// runs find `agentctl` (and its siblings) from the same install.
pub fn render_plist(agentd_bin: &Path, log_dir: &Path) -> String {
    let bin_dir = agentd_bin.parent().unwrap_or_else(|| Path::new("/"));
    PLIST_TEMPLATE
        .replace("__AGENTD_BIN__", &xml_escape(&agentd_bin.to_string_lossy()))
        .replace("__CARGO_BIN_DIR__", &xml_escape(&bin_dir.to_string_lossy()))
        .replace("__LOG_DIR__", &xml_escape(&log_dir.to_string_lossy()))
}

/// Escape the characters that are not allowed verbatim in XML text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A bin dir that survives upgrades. Homebrew runs binaries from a
/// versioned keg (`<prefix>/Cellar/<formula>/<version>/bin`) that the next
/// `brew upgrade` deletes, so a unit pointing there would break; the
/// `<prefix>/bin` symlinks are stable. Anything else is used as-is.
pub fn stable_bin_dir(dir: &Path) -> PathBuf {
    let parts: Vec<Component<'_>> = dir.components().collect();
    match parts
        .iter()
        .position(|c| c.as_os_str() == "Cellar")
        .filter(|&i| i > 0)
    {
        Some(i) => parts[..i].iter().collect::<PathBuf>().join("bin"),
        None => dir.to_path_buf(),
    }
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
        .map(|dir| stable_bin_dir(dir).join("agentd"))
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn rendering_fills_every_placeholder() {
        let out = render_plist(
            Path::new("/opt/homebrew/bin/agentd"),
            Path::new("/Users/me/Library/Logs/agentd"),
        );
        assert!(!out.contains("__"), "unrendered placeholder in:\n{out}");
        assert!(out.contains(&format!("<string>{LABEL}</string>")));
        assert!(out.contains("<string>/opt/homebrew/bin/agentd</string>"));
        assert!(out.contains("<string>/opt/homebrew/bin:/opt/homebrew/bin:"));
        assert!(out.contains("/Users/me/Library/Logs/agentd/agentd.err.log"));
        assert!(out.trim_start().starts_with("<?xml"));
        assert!(out.trim_end().ends_with("</plist>"));
    }

    #[test]
    fn rendering_escapes_xml_metacharacters() {
        let out = render_plist(Path::new("/a&b/<x>/agentd"), Path::new("/logs"));
        assert!(out.contains("<string>/a&amp;b/&lt;x&gt;/agentd</string>"));
        assert!(!out.contains("/a&b/"));
    }

    #[test]
    fn cellar_kegs_map_to_the_stable_prefix_bin() {
        assert_eq!(
            stable_bin_dir(Path::new("/opt/homebrew/Cellar/beagle-workflows/1.2.3/bin")),
            PathBuf::from("/opt/homebrew/bin")
        );
        assert_eq!(
            stable_bin_dir(Path::new("/Users/me/.cargo/bin")),
            PathBuf::from("/Users/me/.cargo/bin")
        );
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
