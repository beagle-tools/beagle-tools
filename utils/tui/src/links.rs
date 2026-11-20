//! Opening URLs (tickets, PRs) in the default browser.

use std::process::Command;
use std::time::Duration;

use beagle_utils::process::spawn_bounded;

use crate::error::ToolError;

/// Opens `url` with `open` (macOS) or `xdg-open`. Fire-and-forget; the child
/// is detached from our stdio so it can never corrupt the terminal.
///
/// # Errors
/// [`ToolError`] when the opener can't be spawned.
pub fn open_url(url: &str) -> Result<(), ToolError> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // Detached but reaped (and killed if it hangs past 10 s).
    spawn_bounded(Command::new(opener).arg(url), Duration::from_secs(10)).map_err(|e| ToolError {
        tool: opener,
        message: e.to_string(),
    })
}
