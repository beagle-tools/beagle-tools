//! agenttop — a live TUI over the agentd store: jobs → runs → logs.
//!
//! Thin entry point: terminal setup/teardown plus the event loop. Data lives
//! in [`app`], rendering in [`ui`], input mapping in [`keys`], tmux glue in
//! [`tmux`], pure helpers in [`mod@format`].

mod app;
mod clip;
mod confirm;
mod dag;
mod detail;
mod format;
mod highlight;
mod jobruns;
mod jobs;
mod keys;
mod logs;
mod logview;
mod tmux;
mod ui;
mod widgets;

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/select.rs"]
mod select_tests;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Mirrors the key agentd writes when retention hits a problem. Duplicated
/// rather than shared because agenttop depends on agent-core, not agentd.
pub const RETENTION_PROBLEM_KEY: &str = "retention_problem";
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::keys::Action;

const POLL_TIMEOUT: Duration = Duration::from_millis(250);
const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// btop-style live dashboard over the agentd store. Takes no arguments
/// beyond `--help` / `--version`; everything else is interactive.
#[derive(Debug, Parser)]
#[command(name = "agenttop", version, about = "Live dashboard for agentd runs")]
struct Cli {}

fn main() -> Result<()> {
    Cli::parse();
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
                // The R confirmation pane owns every key while it is open.
                if key.kind == KeyEventKind::Press && app.confirm.is_some() {
                    app.confirm_key(key.code);
                } else if key.kind == KeyEventKind::Press && app.search_typing() {
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
        Action::Copy => clip::copy_selected_link(app),
        Action::ToggleHelp => app.show_help = true,
        _ => {}
    }
    true
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
    // Overlays own the keys, topmost first. This sat below the jobs-screen
    // branch, so on the DAG page `q` closed the whole page instead of the
    // overlay drawn above it.
    if app.show_run_detail {
        if matches!(action, Action::Open | Action::Attach) {
            clip::open_selected_link(terminal, app)?;
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
            Action::Copy => clip::copy_log_view(app),
            Action::Search => app.search_begin(),
            Action::SearchNext => {
                let s = terminal.size()?;
                app.search_step(s.width, s.height, true);
            }
            Action::SearchPrev => {
                let s = terminal.size()?;
                app.search_step(s.width, s.height, false);
            }
            Action::ToggleHelp => app.show_help = true,
            _ => {}
        }
        return Ok(false);
    }
    handle_dag(terminal, app, action)
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

/// Enter: attach when the run is live; otherwise fall through to its logs
/// (a finished run has no tmux session, but its event feed is the story).
fn attach_selected(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    if let Some(session) = app.attach_target() {
        if let Some(message) = tmux::attach(terminal, &session)? {
            app.set_status(&message);
        }
        app.refresh();
    } else {
        app.set_status("only a running step has a tmux session — enter opens its log");
    }
    Ok(())
}

/// Keys on the jobs screens. Enter goes down one level — job to its runs,
/// step to its log — and esc comes back up one, so a key never skips
/// past where you were. q quits only from the top.
fn handle_dag(terminal: &mut DefaultTerminal, app: &mut App, action: Action) -> Result<bool> {
    if app.runs_open() {
        match action {
            Action::Back | Action::Quit => app.close_job(),
            Action::Follow => app.toggle_follow(),
            // r / a on the all-runs page: only live chains, or every chain.
            Action::ShowRunning if app.all_runs => {
                app.all_running_only = true;
                app.job_step_selected = 0;
            }
            Action::ShowAll if app.all_runs => {
                app.all_running_only = false;
                app.job_step_selected = 0;
            }
            Action::Up => app.dag_move(false),
            Action::Down => app.dag_move(true),
            // Enter (`Action::Attach` in keys.rs) or `l`: the step's log.
            Action::Attach | Action::Logs => match app.dag_selected_run().cloned() {
                Some(run) => app.open_logs_for(run),
                None => app.set_status("this job has no runs yet"),
            },
            // x: kill the selected step if it is running; t: attach to its
            // tmux session (detach with ctrl-b d).
            Action::Kill => app.kill_selected(),
            Action::Run => app.ask_run(),
            Action::Tmux => attach_selected(terminal, app)?,
            // `w`: the step's write-up, rendered — an investigation's
            // OUTPUT.md, or the chain's handover. Before this it was three
            // keys into the details overlay that nothing advertised.
            Action::WriteUp => match (app.step_writeup(), app.dag_selected_run()) {
                (Some(path), _) => {
                    if let Some(message) = tmux::render_markdown(terminal, &path)? {
                        app.set_status(&message);
                    }
                }
                (None, Some(run)) => {
                    let msg = format!(
                        "{} has no write-up — a step shows its summary.md here, an \
                         investigation its OUTPUT.md, a code chain its handover",
                        run.agent
                    );
                    app.set_status(&msg);
                }
                (None, None) => app.set_status("no step selected"),
            },
            // `o`: the draft PR THIS step opened, in the browser. Only the
            // step that ran `gh pr create` has one — on a docs or report
            // step it says so instead of opening the chain's PR.
            Action::Open => match (app.selected_step_pr(), app.dag_selected_run()) {
                (Some(pr), _) => {
                    let pr = pr.to_owned();
                    let ok = std::process::Command::new("open")
                        .arg(&pr)
                        .status()
                        .is_ok_and(|s| s.success());
                    app.set_status(&if ok {
                        format!("opened {pr}")
                    } else {
                        format!("could not open {pr}")
                    });
                }
                (None, Some(run)) => {
                    let msg = format!("{} opened no PR", run.agent);
                    app.set_status(&msg);
                }
                (None, None) => app.set_status("no step selected"),
            },
            Action::Detail => {
                app.show_run_detail = true;
                app.detail_selected = 0;
            }
            Action::Edit => edit_selected_manifest(terminal, app)?,
            Action::Copy => clip::copy_selected(app),
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
        Action::Up => app.dag_move(false),
        Action::Down => app.dag_move(true),
        Action::Attach | Action::Open => app.open_job(),
        Action::AllRuns => app.open_all_runs(),
        Action::Search => app.job_search_begin(),
        Action::Edit => edit_selected_manifest(terminal, app)?,
        // r / a: running jobs only, or all of them. The cursor goes back
        // to the top — the old index points into the other list.
        Action::ShowRunning => {
            app.job_running_only = true;
            app.dag_selected = 0;
        }
        Action::ShowAll => {
            app.job_running_only = false;
            app.dag_selected = 0;
        }
        Action::ToggleHidden => {
            app.show_hidden = !app.show_hidden;
            app.dag_selected = 0;
        }
        Action::Run => app.ask_run(),
        Action::Copy => clip::copy_selected(app),
        Action::ToggleHelp => app.show_help = true,
        _ => {}
    }
    Ok(false)
}
