//! Staying current.
//!
//! The CLI normally ships *inside* the Tandem app, where it isn't its
//! own installable thing — updating the app updates it. So this doesn't
//! try to replace a binary it doesn't own: it works out how this copy
//! got here and does the right thing for that, which is sometimes
//! "tell you to press the button in the app".

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

/// The rolling mirror's updater manifest; the in-app updater reads the
/// same file, so its `version` is the newest Tandem release.
const LATEST_JSON: &str =
    "https://github.com/beagle-tools/beagle-tools/releases/download/beagle-tandem-latest/latest.json";

/// The Homebrew cask that installs the app (and links this CLI).
const CASK: &str = "beagle-tandem";

const TIMEOUT: Duration = Duration::from_secs(30);

/// How this copy of the CLI was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Install {
    /// Inside the app, which Homebrew manages.
    BrewApp,
    /// Inside the app, installed some other way.
    App,
    /// Built from source, or linked into the `beagle` command.
    Standalone,
}

/// The app bundles its CLI beside its own executable: inside
/// `Tandem.app/Contents/MacOS/` on macOS, next to the app binary in the
/// Linux packages and the Windows install dir.
fn inside_app(exe: &Path) -> bool {
    if exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        return true;
    }
    let Some(dir) = exe.parent() else {
        return false;
    };
    // Only the CLI's own name counts: `beagle` linking this library is a
    // different program that happens to sit in some other bin dir.
    let named_cli = exe
        .file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("tandem-cli"));
    named_cli
        && ["tandem", "Tandem.exe", "tandem.exe"]
            .iter()
            .any(|app| dir.join(app).is_file())
}

fn brew_manages_tandem() -> bool {
    std::process::Command::new("brew")
        .args(["list", "--cask", CASK])
        .output()
        .is_ok_and(|o| o.status.success())
}

pub(crate) fn detect(exe: &Path) -> Install {
    if !inside_app(exe) {
        return Install::Standalone;
    }
    if cfg!(target_os = "macos") && brew_manages_tandem() {
        Install::BrewApp
    } else {
        Install::App
    }
}

fn latest_version() -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("tandem-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let body: Value = client
        .get(LATEST_JSON)
        .send()
        .map_err(|e| format!("couldn't reach GitHub: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| format!("latest.json wasn't JSON: {e}"))?;
    body.get("version")
        .and_then(Value::as_str)
        .map(|v| v.trim_start_matches('v').to_owned())
        .ok_or_else(|| "latest.json has no version".to_owned())
}

/// True when `latest` is newer. Falls back to "different means newer"
/// if either side isn't three numbers, which beats claiming you're
/// current when we can't tell.
pub(crate) fn behind(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let mut it = v.split('.').map(str::parse::<u64>);
        match (it.next(), it.next(), it.next(), it.next()) {
            (Some(Ok(a)), Some(Ok(b)), Some(Ok(c)), None) => Some((a, b, c)),
            _ => None,
        }
    };
    match (parse(current), parse(latest)) {
        (Some(a), Some(b)) => b > a,
        _ => current != latest,
    }
}

fn label(how: Install) -> &'static str {
    match how {
        Install::BrewApp => "inside the Tandem app (homebrew)",
        Install::App => "inside the Tandem app",
        Install::Standalone => "standalone build",
    }
}

struct Status {
    current: String,
    latest: String,
    behind: bool,
    how: Install,
}

fn status(exe: &Path) -> Result<Status, String> {
    let current = env!("CARGO_PKG_VERSION").to_owned();
    let latest = latest_version()?;
    let behind = behind(&current, &latest);
    Ok(Status {
        current,
        latest,
        behind,
        how: detect(exe),
    })
}

/// `update --check`: versions and how this copy is installed.
pub(crate) fn check() -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let s = status(&exe)?;
    Ok(serde_json::json!({
        "current": s.current,
        "latest": s.latest,
        "behind": s.behind,
        "installed_as": label(s.how),
        "path": exe,
    }))
}

/// `update`: do whatever updating means for this install.
pub(crate) fn perform() -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let s = status(&exe)?;
    let mut value = if s.behind {
        update(s.how)?
    } else {
        serde_json::json!({ "updated": false, "note": "already current" })
    };
    value["current"] = Value::String(s.current);
    value["latest"] = Value::String(s.latest);
    Ok(value)
}

fn update(how: Install) -> Result<Value, String> {
    Ok(match how {
        // The CLI is part of the app here, so updating it means
        // updating the app — and brew is the thing that owns that.
        Install::BrewApp => {
            let out = std::process::Command::new("brew")
                .args(["upgrade", "--cask", CASK])
                .output()
                .map_err(|e| format!("couldn't run brew: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "brew upgrade failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            serde_json::json!({ "updated": true, "via": format!("brew upgrade --cask {CASK}") })
        }
        // Replacing an app out from under a running app is the app's own
        // job, and it has an updater for it.
        Install::App => serde_json::json!({
            "updated": false,
            "via": "in-app updater",
            "note": "this CLI ships inside the Tandem app — update the app and the CLI comes with it",
        }),
        // No standalone binary is published; this copy was built from
        // source (or is part of `beagle`), so it updates the same way.
        Install::Standalone => serde_json::json!({
            "updated": false,
            "via": "rebuild",
            "note": format!(
                "this is a standalone build — rebuild it from source, or install the app \
                 (brew install --cask beagle-tools/tap/{CASK}), which ships tandem-cli"
            ),
        }),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_count_as_behind() {
        assert!(behind("0.2.28", "0.3.0"));
        assert!(behind("0.2.9", "0.2.10"));
        assert!(!behind("0.3.0", "0.3.0"));
        assert!(!behind("0.3.1", "0.3.0"));
    }

    #[test]
    fn unparseable_versions_err_towards_telling_you() {
        // We can't compare, so say "behind" rather than "current".
        assert!(behind("dev", "0.3.0"));
        assert!(behind("0.3.0.1", "0.3.0"));
        assert!(!behind("same", "same"));
    }

    #[test]
    fn a_loose_binary_is_standalone() {
        assert_eq!(
            detect(Path::new("/nowhere/at/all/tandem-cli")),
            Install::Standalone
        );
        assert!(inside_app(Path::new(
            "/Applications/Tandem.app/Contents/MacOS/tandem-cli"
        )));
    }

    #[test]
    fn standalone_update_explains_instead_of_replacing() {
        let value = update(Install::Standalone).unwrap();
        assert_eq!(value["updated"], Value::Bool(false));
        let value = update(Install::App).unwrap();
        assert_eq!(value["via"], Value::from("in-app updater"));
    }
}
