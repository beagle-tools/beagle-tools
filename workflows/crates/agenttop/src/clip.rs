//! Clipboard and link actions: `y` copies, `o`/enter on a link opens.
//!
//! Split from `main.rs`, which keeps the event loop and key dispatch.

use anyhow::Result;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::tmux;

/// `y` — put the highlighted link on the clipboard. For a markdown path that
/// is the point: paste it at another agent and it can read the write-up.
pub(crate) fn copy_selected_link(app: &mut App) {
    let Some(target) = selected_link_target(app) else {
        return;
    };
    match beagle_tui::clipboard::copy(&target) {
        Ok(_) => app.set_status(&format!("copied {target}")),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}

pub(crate) fn selected_link_target(app: &App) -> Option<String> {
    let run = crate::detail::focused_run(app)?;
    crate::detail::run_links_in_context(app, run)
        .get(app.detail_selected)
        .map(|l| l.target.clone())
}

/// Hand a link to the OS: `open` takes a URL or a path and does the right
/// thing with either. Best-effort — a failure must not take the TUI down.
/// Open the highlighted link. A markdown file is rendered in the terminal
/// (suspending the TUI, like `e`); anything else goes to the OS, which knows
/// what a URL is.
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

pub(crate) fn copy_log_view(app: &mut App) {
    let Some(view) = &app.log_view else {
        return;
    };
    let text = agent_core::transcript::plain_text(&view.events);
    match beagle_tui::clipboard::copy(&text) {
        Ok(_) => app.set_status("log copied to clipboard"),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}

/// `y` on the jobs screens: on a runs screen the selected step's run id
/// (what `agentctl logs` / `resume` take), on the jobs list the job's name
/// (what `ag -j` / `agentctl run` take).
pub(crate) fn copy_selected(app: &mut App) {
    let text = if app.runs_open() {
        app.dag_selected_run().map(|r| r.id.clone())
    } else {
        app.selected_job().map(|j| j.spec.qualified_name())
    };
    let Some(text) = text else {
        app.set_status("nothing selected");
        return;
    };
    match beagle_tui::clipboard::copy(&text) {
        Ok(_) => app.set_status(&format!("copied {text}")),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}
