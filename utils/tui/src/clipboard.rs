//! Copying text to the system clipboard by shelling out to the platform tool.

use std::io::ErrorKind;
use std::process::Command;
use std::time::Duration;

use beagle_utils::process::output_with_input_and_timeout;

use crate::error::ToolError;

/// Candidate clipboard writers, tried in order.
const TOOLS: [(&str, &[&str]); 3] = [
    ("pbcopy", &[]),
    ("wl-copy", &[]),
    ("xclip", &["-selection", "clipboard"]),
];

/// A clipboard write is local and instant; a tool that hangs (a stuck X
/// server) must not freeze the TUI.
const COPY_TIMEOUT: Duration = Duration::from_secs(5);

/// Copies `text` to the clipboard.
///
/// # Errors
/// [`ToolError`] when no clipboard tool is available, it fails, or it does
/// not finish within 5 s.
pub fn copy(text: &str) -> Result<(), ToolError> {
    for (tool, args) in TOOLS {
        let mut cmd = Command::new(tool);
        cmd.args(args);
        let output = match output_with_input_and_timeout(&mut cmd, text.as_bytes(), COPY_TIMEOUT) {
            Ok(output) => output,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(ToolError {
                    tool,
                    message: e.to_string(),
                })
            }
        };
        return if output.status.success() {
            Ok(())
        } else {
            Err(ToolError {
                tool,
                message: format!("exited with {}", output.status),
            })
        };
    }
    Err(ToolError {
        tool: "clipboard",
        message: "no pbcopy, wl-copy, or xclip on PATH".to_owned(),
    })
}
