//! The details overlay's links: which one is highlighted, and opening it.
//! Copying goes through `input::Effect::Copy`.

use anyhow::Result;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::tmux;

/// The highlighted link's target in the details overlay. For a markdown
/// path, copying it is the point: paste it at another agent and it can
/// read the write-up.
pub(crate) fn selected_link_target(app: &App) -> Option<String> {
    let run = crate::detail::focused_run(app)?;
    crate::detail::run_links_in_context(app, run)
        .get(app.detail_selected)
        .map(|l| l.target.clone())
}

/// Open the highlighted link. A markdown file is rendered in the terminal
/// (suspending the TUI, like `E`); anything else goes to the OS, which
/// knows what a URL is. Best-effort — a failure must not take the TUI down.
pub(crate) fn open_selected_link(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let Some(target) = selected_link_target(app) else {
        return Ok(());
    };
    let path = std::path::Path::new(&target);
    let is_markdown = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if is_markdown && path.is_file() {
        if let Some(message) = tmux::render_markdown(terminal, path)? {
            app.set_status(&message);
        }
        return Ok(());
    }
    let msg = match beagle_tui::links::open_url(&target) {
        Ok(()) => format!("opened {target}"),
        Err(e) => format!("could not open {target}: {e}"),
    };
    app.set_status(&msg);
    Ok(())
}
