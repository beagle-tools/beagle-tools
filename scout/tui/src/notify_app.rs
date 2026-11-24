//! `Scout.app`: the notification app, so banners show scout's dog instead of
//! the Terminal icon.
//!
//! macOS always shows the icon of the *app* that posts a notification, and
//! no API lets a sender override it. So `beagle scout notify setup` copies Homebrew's
//! `terminal-notifier.app`, renames it `Scout` with its own bundle id, swaps
//! in the dog icon (`assets/scout-dog.png`, built into the binary), re-signs
//! it locally, and registers it with Launch Services. [`crate::notify`] then
//! posts through it whenever it exists.
//!
//! Lives at `~/Library/Application Support/scout/Scout.app`. Re-running setup
//! rebuilds it from scratch.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// The dog icon, built into the binary.
const DOG_PNG: &[u8] = include_bytes!("../assets/scout-dog.png");

/// The notification app's bundle id. A new id is what makes macOS treat it
/// as its own app, with its own icon and notification settings.
pub const BUNDLE_ID: &str = "dev.scout.notifier";

/// Where Homebrew keeps `terminal-notifier.app` (Apple silicon, then Intel).
const CELLARS: [&str; 2] = [
    "/opt/homebrew/Cellar/terminal-notifier",
    "/usr/local/Cellar/terminal-notifier",
];

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

/// `~/Library/Application Support/scout/Scout.app`.
#[must_use]
pub fn path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/scout/Scout.app")
}

/// The executable inside `Scout.app`, if setup has been run.
#[must_use]
pub fn executable(home: &Path) -> Option<PathBuf> {
    let exe = path(home).join("Contents/MacOS/terminal-notifier");
    exe.is_file().then_some(exe)
}

/// Builds (or rebuilds) `Scout.app` and returns its path.
///
/// # Errors
/// [`Error::Io`] when `terminal-notifier` is not installed, or a step
/// (copy, `plutil`, `sips`, `iconutil`, `codesign`) fails.
pub fn setup(home: &Path) -> Result<PathBuf> {
    let source = find_terminal_notifier().ok_or_else(|| Error::Io {
        path: PathBuf::from("terminal-notifier.app"),
        source: std::io::Error::other(
            "terminal-notifier is not installed — run `brew install terminal-notifier`",
        ),
    })?;
    let app = path(home);
    if app.exists() {
        fs::remove_dir_all(&app).map_err(Error::io(&app))?;
    }
    if let Some(parent) = app.parent() {
        fs::create_dir_all(parent).map_err(Error::io(parent))?;
    }
    run("ditto", &[source.as_os_str(), app.as_os_str()])?;

    let plist = app.join("Contents/Info.plist");
    for (key, value) in [
        ("CFBundleIdentifier", BUNDLE_ID),
        ("CFBundleName", "Scout"),
        ("CFBundleIconFile", "Scout"),
    ] {
        run(
            "plutil",
            &[
                "-replace".as_ref(),
                key.as_ref(),
                "-string".as_ref(),
                value.as_ref(),
                plist.as_os_str(),
            ],
        )?;
    }
    run(
        "plutil",
        &[
            "-replace".as_ref(),
            "CFBundleDisplayName".as_ref(),
            "-string".as_ref(),
            "Scout".as_ref(),
            plist.as_os_str(),
        ],
    )?;

    let resources = app.join("Contents/Resources");
    write_icon(&resources)?;
    let old_icon = resources.join("Terminal.icns");
    if old_icon.exists() {
        fs::remove_file(&old_icon).map_err(Error::io(&old_icon))?;
    }

    // Editing the bundle broke its signature; sign it again, ad hoc (this
    // machine only), as Homebrew's original is.
    run(
        "codesign",
        &[
            "--force".as_ref(),
            "--deep".as_ref(),
            "--sign".as_ref(),
            "-".as_ref(),
            app.as_os_str(),
        ],
    )?;
    // Register so Notification Center learns the new name and icon now.
    let _ = run(LSREGISTER, &["-f".as_ref(), app.as_os_str()]);
    Ok(app)
}

/// Turns the embedded PNG into `Scout.icns` (every size macOS asks for).
fn write_icon(resources: &Path) -> Result<()> {
    let work = resources.join("Scout.iconset");
    fs::create_dir_all(&work).map_err(Error::io(&work))?;
    let source = work.join("source.png");
    fs::write(&source, DOG_PNG).map_err(Error::io(&source))?;
    for size in [16u32, 32, 128, 256, 512] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let pixels = (size * scale).to_string();
            let out = work.join(format!("icon_{size}x{size}{suffix}.png"));
            run(
                "sips",
                &[
                    "-z".as_ref(),
                    pixels.as_ref(),
                    pixels.as_ref(),
                    source.as_os_str(),
                    "--out".as_ref(),
                    out.as_os_str(),
                ],
            )?;
        }
    }
    fs::remove_file(&source).map_err(Error::io(&source))?;
    let icns = resources.join("Scout.icns");
    run(
        "iconutil",
        &[
            "-c".as_ref(),
            "icns".as_ref(),
            work.as_os_str(),
            "-o".as_ref(),
            icns.as_os_str(),
        ],
    )?;
    fs::remove_dir_all(&work).map_err(Error::io(&work))
}

/// The newest `terminal-notifier.app` Homebrew has installed.
fn find_terminal_notifier() -> Option<PathBuf> {
    CELLARS.iter().find_map(|cellar| {
        let mut versions: Vec<PathBuf> = fs::read_dir(cellar)
            .ok()?
            .flatten()
            .map(|e| e.path().join("terminal-notifier.app"))
            .filter(|app| app.is_dir())
            .collect();
        versions.sort();
        versions.pop()
    })
}

/// Runs a tool quietly; a non-zero exit becomes an error naming it.
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<()> {
    // Local tools (ditto, sips, codesign, lsregister): 60 s is generous.
    let output = beagle_utils::process::output_with_timeout(
        Command::new(program).args(args),
        std::time::Duration::from_secs(60),
    )
    .map_err(Error::io(program))?;
    if output.status.success() {
        return Ok(());
    }
    Err(Error::Io {
        path: PathBuf::from(program),
        source: std::io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
    })
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_dog_is_a_png() {
        assert!(DOG_PNG.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn paths() {
        let home = Path::new("/Users/me");
        assert_eq!(
            path(home),
            Path::new("/Users/me/Library/Application Support/scout/Scout.app")
        );
        assert!(executable(Path::new("/nonexistent")).is_none());
    }
}
