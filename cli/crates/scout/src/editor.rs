//! Opening a file in the user's editor: `$VISUAL`, then `$EDITOR`, then `vi`.

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

/// The editor command line, split on whitespace (so `code -w` works).
#[must_use]
pub fn command() -> Vec<String> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|var| std::env::var(var).ok())
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|argv| !argv.is_empty())
        .unwrap_or_else(|| vec!["vi".to_owned()])
}

/// Runs the editor on `path` and waits for it to exit. A non-zero exit is
/// not an error: the user may simply have quit without saving.
///
/// # Errors
/// [`Error::Editor`] if the editor cannot be started.
pub fn open(path: &Path) -> Result<()> {
    let argv = command();
    let Some((program, rest)) = argv.split_first() else {
        return Ok(()); // unreachable: `command` always yields a program
    };
    Command::new(program)
        .args(rest)
        .arg(path)
        .status()
        .map_err(|source| Error::Editor {
            command: argv.join(" "),
            source,
        })?;
    Ok(())
}
