//! Copying text to the system clipboard by shelling out to the platform tool.

use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::error::ToolError;

/// Candidate clipboard writers, tried in order.
const TOOLS: [(&str, &[&str]); 3] = [
    ("pbcopy", &[]),
    ("wl-copy", &[]),
    ("xclip", &["-selection", "clipboard"]),
];

/// Copies `text` to the clipboard.
///
/// # Errors
/// [`ToolError`] when no clipboard tool is available or it fails.
pub fn copy(text: &str) -> Result<(), ToolError> {
    for (tool, args) in TOOLS {
        let Ok(mut child) = Command::new(tool)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).map_err(|e| ToolError {
                tool,
                message: e.to_string(),
            })?;
        }
        let status = child.wait().map_err(|e| ToolError {
            tool,
            message: e.to_string(),
        })?;
        return if status.success() {
            Ok(())
        } else {
            Err(ToolError {
                tool,
                message: format!("exited with {status}"),
            })
        };
    }
    Err(ToolError {
        tool: "clipboard",
        message: "no pbcopy, wl-copy, or xclip on PATH".to_owned(),
    })
}
