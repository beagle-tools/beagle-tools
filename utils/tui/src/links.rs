//! Opening URLs (tickets, PRs) in the default browser.

use std::process::{Command, Stdio};

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
    Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| ToolError {
            tool: opener,
            message: e.to_string(),
        })
}
