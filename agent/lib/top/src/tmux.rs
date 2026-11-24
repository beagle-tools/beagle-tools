//! tmux integration: attach to a run's session (suspending the TUI) and
//! best-effort session kill.

use std::io::stdout;
use std::process::Command;

use anyhow::{Context, Result};
use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::DefaultTerminal;

/// Attach to `session`, handing the real terminal to tmux for the duration.
///
/// Leaves the alternate screen and raw mode, runs `tmux attach-session` as a
/// child until it exits (the user detaches with `ctrl-b d` or the session
/// dies), then restores the TUI and forces a full redraw.
///
/// Returns a status message to surface when the attach did not go cleanly.
pub fn attach(terminal: &mut DefaultTerminal, session: &str) -> Result<Option<String>> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(stdout(), LeaveAlternateScreen, Show).context("leave alternate screen")?;

    let status = agent_core::tmux::command()
        .args([
            "attach-session",
            "-t",
            &agent_core::tmux::exact_target(session),
        ])
        .status();

    enable_raw_mode().context("re-enable raw mode")?;
    execute!(stdout(), EnterAlternateScreen).context("re-enter alternate screen")?;
    terminal.clear().context("redraw after attach")?;

    match status {
        Ok(exit) if exit.success() => Ok(None),
        Ok(exit) => Ok(Some(format!("tmux attach exited with {exit}"))),
        Err(e) => Ok(Some(format!("failed to spawn tmux: {e}"))),
    }
}

/// Open `path` in `$EDITOR` (default vim), suspending the TUI like attach.
/// The editor line runs through `sh -c` so an EDITOR with flags works.
pub fn edit_file(terminal: &mut DefaultTerminal, path: &std::path::Path) -> Result<Option<String>> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(stdout(), LeaveAlternateScreen, Show).context("leave alternate screen")?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_owned());
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("agenttop-edit")
        .arg(path)
        .status();

    enable_raw_mode().context("re-enable raw mode")?;
    execute!(stdout(), EnterAlternateScreen).context("re-enter alternate screen")?;
    terminal.clear().context("redraw after edit")?;

    match status {
        Ok(exit) if exit.success() => Ok(None),
        Ok(exit) => Ok(Some(format!("editor exited with {exit}"))),
        Err(e) => Ok(Some(format!("failed to spawn editor: {e}"))),
    }
}

/// Render a markdown file in the terminal, suspending the TUI like `edit_file`.
///
/// `glow` renders; `bat` only highlights the source, which is the wrong tool
/// for a write-up meant to be read. Falls back to `$PAGER`, then `less`, so a
/// machine without glow still shows something.
pub fn render_markdown(
    terminal: &mut DefaultTerminal,
    path: &std::path::Path,
) -> Result<Option<String>> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(stdout(), LeaveAlternateScreen, Show).context("leave alternate screen")?;

    let viewer = if beagle_utils::process::which("glow").is_some() {
        // -p pages it; width comes from the terminal glow is handed back.
        "glow -s dark -p \"$1\"".to_owned()
    } else {
        let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
        format!("{pager} \"$1\"")
    };
    // Same augmented PATH `which` searched, so a glow it found is runnable.
    let status = beagle_utils::process::command("sh")
        .arg("-c")
        .arg(viewer)
        .arg("agenttop-view")
        .arg(path)
        .status();

    enable_raw_mode().context("re-enable raw mode")?;
    execute!(stdout(), EnterAlternateScreen).context("re-enter alternate screen")?;
    terminal.clear().context("redraw after viewing")?;

    match status {
        Ok(exit) if exit.success() => Ok(None),
        Ok(exit) => Ok(Some(format!("viewer exited with {exit}"))),
        Err(e) => Ok(Some(format!("failed to spawn viewer: {e}"))),
    }
}

/// Kill a tmux session (exact name), ignoring errors — it may already be gone.
pub fn kill_session(session: &str) {
    let _ = agent_core::tmux::kill_session(session);
}
