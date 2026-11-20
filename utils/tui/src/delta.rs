//! Rendering a unified diff with the `delta` pager (side by side, the user's
//! own git look), for TUIs that show a Diff tab. Callers keep a plain
//! fallback: every failure here is `None`/`false`, never an error screen.

use std::time::Duration;

use beagle_utils::process;

/// Bound on one delta run; a hung delta falls back to the plain view.
const DELTA_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether `delta` is installed (on the augmented `PATH`) and runnable.
#[must_use]
pub fn available() -> bool {
    process::which("delta").is_some_and(|delta| {
        process::output_with_timeout(process::command(delta).arg("--version"), DELTA_TIMEOUT)
            .is_ok_and(|out| out.status.success())
    })
}

/// Runs `delta --side-by-side` on the diff `source` at `width` columns (at
/// least 40) and returns its raw ANSI output. `None` if delta is missing,
/// fails or hangs.
#[must_use]
pub fn side_by_side(source: &str, width: u16) -> Option<String> {
    let delta = process::which("delta")?;
    let mut cmd = process::command(delta);
    cmd.args(["--side-by-side", "--paging=never", "--width"])
        .arg(width.max(40).to_string());
    // Stdin is fed from a helper thread, so a large diff can't deadlock
    // against a full stdout pipe.
    let out =
        process::output_with_input_and_timeout(&mut cmd, source.as_bytes(), DELTA_TIMEOUT).ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
