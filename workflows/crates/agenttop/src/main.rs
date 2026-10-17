//! agenttop — btop-style live dashboard over the agentd store.
//!
//! Thin entry point: terminal setup/teardown plus the event loop. Data lives
//! in [`app`], rendering in [`ui`], input mapping in [`keys`], tmux glue in
//! [`tmux`], pure helpers in [`mod@format`].

mod app;
mod dag;
mod detail;
mod format;
mod jobruns;
mod jobs;
mod keys;
mod logs;
mod logview;
mod select;
mod tmux;
mod ui;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Mirrors the key agentd writes when retention hits a problem. Duplicated
/// rather than shared because agenttop depends on agent-core, not agentd.
pub const RETENTION_PROBLEM_KEY: &str = "retention_problem";
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::keys::Action;

const POLL_TIMEOUT: Duration = Duration::from_millis(250);
const REFRESH_EVERY: Duration = Duration::from_secs(1);

fn main() -> Result<()> {
    // try_init installs a panic hook that restores the terminal before the
    // panic message prints; the explicit restore below covers the Err path.
    let mut terminal = ratatui::try_init().context("initialize terminal")?;
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    app.refresh();
    let mut last_refresh = Instant::now();
    loop {
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .context("draw frame")?;
        if event::poll(POLL_TIMEOUT).context("poll input")? {
            if let Event::Key(key) = event::read().context("read input")? {
                // While a search term is being typed, keys are text — `j`
                // must type a j, not scroll. So this runs before map_key.
                if key.kind == KeyEventKind::Press && app.search_typing() {
                    app.search_key(key.code);
                } else if key.kind == KeyEventKind::Press && app.job_search_typing() {
                    app.job_search_key(key.code);
                } else if key.kind == KeyEventKind::Press
                    && handle(terminal, &mut app, keys::map_key(key))?
                {
                    return Ok(());
                }
            }
        }
        if last_refresh.elapsed() >= REFRESH_EVERY {
            app.refresh();
            last_refresh = Instant::now();
        }
    }
}

/// Apply one action. Returns `true` when the app should exit. Overlays eat
/// keys first: help closes on q/esc/?, the log view scrolls and closes on
/// q/esc — only the bare dashboard interprets q as quit.
/// Keys while the run-detail overlay is up. It sits over either screen, so
/// this runs before both of their key maps.
fn handle_detail(app: &mut App, action: Action) -> bool {
    let links = crate::detail::focused_run(app).map_or(0, |r| crate::detail::run_links(r).len());
    match action {
        Action::Quit | Action::Back | Action::Detail => {
            app.show_run_detail = false;
            app.detail_selected = 0;
        }
        Action::Down if links > 0 => {
            app.detail_selected = (app.detail_selected + 1).min(links - 1);
        }
        Action::Up => app.detail_selected = app.detail_selected.saturating_sub(1),
        Action::Copy => copy_selected_link(app),
        Action::ToggleHelp => app.show_help = true,
        _ => {}
    }
    true
}

/// `y` — put the highlighted link on the clipboard. For a markdown path that
/// is the point: paste it at another agent and it can read the write-up.
fn copy_selected_link(app: &mut App) {
    let Some(target) = selected_link_target(app) else {
        return;
    };
    match copy_to_clipboard(&target) {
        Ok(()) => app.set_status(&format!("copied {target}")),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}

fn selected_link_target(app: &App) -> Option<String> {
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
fn open_selected_link(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
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
    let ok = std::process::Command::new("open")
        .arg(&target)
        .status()
        .is_ok_and(|s| s.success());
    let msg = if ok {
        format!("opened {target}")
    } else {
        format!("could not open {target}")
    };
    app.set_status(&msg);
    Ok(())
}

fn handle(terminal: &mut DefaultTerminal, app: &mut App, action: Action) -> Result<bool> {
    if action == Action::ForceQuit {
        return Ok(true);
    }
    if app.show_help {
        if matches!(action, Action::Quit | Action::Back | Action::ToggleHelp) {
            app.show_help = false;
        }
        return Ok(false);
    }
    // Overlays own the keys, topmost first. This sat below the show_dag
    // branch, so on the DAG page `q` closed the whole page instead of the
    // overlay drawn above it.
    if app.show_run_detail {
        if matches!(action, Action::Open | Action::Attach) {
            open_selected_link(terminal, app)?;
        } else {
            handle_detail(app, action);
        }
        return Ok(false);
    }

    if app.log_view.is_some() {
        match action {
            Action::Quit | Action::Back => app.close_logs(),
            Action::Up => scroll_log_view(terminal, app, true)?,
            Action::Down => scroll_log_view(terminal, app, false)?,
            // `l` doubles as pan-right here — it can't mean "open logs".
            Action::Right | Action::Logs => app.pan_logs(true),
            Action::Left => app.pan_logs(false),
            Action::Copy => copy_log_view(app),
            Action::Search => app.search_begin(),
            Action::SearchNext => {
                let s = terminal.size()?;
                app.search_step(s.width, s.height, true);
            }
            Action::SearchPrev => {
                let s = terminal.size()?;
                app.search_step(s.width, s.height, false);
            }
            Action::Refresh => app.refresh(),
            Action::ToggleHelp => app.show_help = true,
            _ => {}
        }
        return Ok(false);
    }
    if app.show_dag {
        return handle_dag(terminal, app, action);
    }
    match action {
        Action::Quit => return Ok(true),
        Action::Up => app.move_selection(false),
        Action::Down => app.move_selection(true),
        Action::Refresh => app.refresh(),
        Action::ToggleHelp => app.show_help = true,
        Action::Kill => app.kill_selected(),
        Action::Logs => app.open_logs(),
        Action::Copy => copy_selected_run_id(app),
        Action::Dag => app.show_dag = true,
        Action::DagFocus => app.focus_dag_on_selection(),
        Action::Detail | Action::Open => {
            app.show_run_detail = true;
            app.detail_selected = 0;
        }
        Action::Filter => app.cycle_recent_filter(),
        Action::Attach => attach_selected(terminal, app)?,
        // Search belongs to the log view; on the dashboard these do nothing
        // rather than something surprising.
        Action::Edit
        | Action::Left
        | Action::Right
        | Action::Back
        | Action::Search
        | Action::SearchNext
        | Action::SearchPrev
        | Action::ForceQuit
        | Action::None => {}
    }
    Ok(false)
}

/// `e` on the DAG page: open the selected agent's manifest in $EDITOR. The
/// daemon reconciles the change within a tick of the editor closing.
fn edit_selected_manifest(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let Some(path) = app.dag_selected_agent().map(|a| a.source.clone()) else {
        app.set_status("no agent selected");
        return Ok(());
    };
    if !path.is_file() {
        app.set_status("manifest path unknown for this agent");
        return Ok(());
    }
    if let Some(message) = tmux::edit_file(terminal, &path)? {
        app.set_status(&message);
    } else {
        app.set_status("saved — the daemon reloads it within ~5s");
    }
    app.refresh();
    Ok(())
}

/// Scroll the log view, clamped to the real number of wrapped display lines
/// (which depends on the current terminal width).
fn scroll_log_view(terminal: &DefaultTerminal, app: &mut App, up: bool) -> Result<()> {
    let size = terminal.size().context("terminal size")?;
    let width = usize::from(size.width.saturating_sub(2));
    let visible = usize::from(size.height.saturating_sub(3));
    let max = app
        .log_view
        .as_ref()
        .map_or(0, |view| {
            logs::line_count(&view.events, width, view.scroll_x == 0)
        })
        .saturating_sub(visible);
    app.scroll_logs(up, max);
    Ok(())
}

fn copy_log_view(app: &mut App) {
    let Some(view) = &app.log_view else {
        return;
    };
    let text = agent_core::transcript::plain_text(&view.events);
    match copy_to_clipboard(&text) {
        Ok(()) => app.set_status("log copied to clipboard"),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}

fn copy_selected_run_id(app: &mut App) {
    let Some(id) = app.selected_run().map(|run| run.id.clone()) else {
        app.set_status("nothing selected");
        return;
    };
    match copy_to_clipboard(&id) {
        Ok(()) => app.set_status(&format!("copied {id}")),
        Err(e) => app.set_status(&format!("copy failed: {e}")),
    }
}

/// Pipe text into the system clipboard (`pbcopy` on macOS).
fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write as _;
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("spawn pbcopy")?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .context("write to pbcopy")?;
    }
    let status = child.wait().context("wait for pbcopy")?;
    anyhow::ensure!(status.success(), "pbcopy exited with {status}");
    Ok(())
}

/// Enter: attach when the run is live; otherwise fall through to its logs
/// (a finished run has no tmux session, but its event feed is the story).
fn attach_selected(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    if let Some(session) = app.attach_target() {
        if let Some(message) = tmux::attach(terminal, &session)? {
            app.set_status(&message);
        }
        app.refresh();
    } else {
        app.open_logs();
    }
    Ok(())
}

/// Keys on the jobs screens. Enter goes down one level — job to its runs,
/// step to its log — and esc comes back up one, so a key never skips
/// past where you were. q quits only from the top.
fn handle_dag(terminal: &mut DefaultTerminal, app: &mut App, action: Action) -> Result<bool> {
    if app.job_open.is_some() {
        match action {
            Action::Back | Action::Quit => app.close_job(),
            Action::Up => app.dag_move(false),
            Action::Down => app.dag_move(true),
            // Enter is `Action::Attach` in keys.rs — `Open` is `o`. Every
            // earlier "enter drills in" matched only Open, so enter did
            // nothing on this page and only `o` worked. Both, here.
            Action::Attach | Action::Open | Action::Logs => match app.dag_selected_run().cloned() {
                Some(run) => app.open_logs_for(run),
                None => app.set_status("this job has no runs yet"),
            },
            Action::Detail => {
                app.show_run_detail = true;
                app.detail_selected = 0;
            }
            Action::Edit => edit_selected_manifest(terminal, app)?,
            Action::Refresh => app.refresh(),
            Action::ToggleHelp => app.show_help = true,
            _ => {}
        }
        return Ok(false);
    }
    match action {
        Action::Quit => return Ok(true),
        // esc on the landing page only clears a search; there is no
        // level above it to go back to.
        Action::Back => {
            app.job_search.clear();
            app.dag_selected = 0;
        }
        Action::Dag => app.show_dag = false,
        Action::Up => app.dag_move(false),
        Action::Down => app.dag_move(true),
        Action::Attach | Action::Open => app.open_job(),
        Action::Search => app.job_search_begin(),
        Action::Edit => edit_selected_manifest(terminal, app)?,
        Action::Refresh => app.refresh(),
        Action::ToggleHelp => app.show_help = true,
        _ => {}
    }
    Ok(false)
}
