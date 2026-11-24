//! What an [`Action`] does on the screen on top. Terminal-free: anything
//! that needs the terminal, the clipboard, the browser or a fresh store
//! read comes back as an [`Effect`] for `lib.rs` to carry out, so tests
//! can drive every key (`tests/keymap.rs`).
//!
//! Layers, topmost first: help, the run-detail overlay, the log view, then
//! the runs screen or the jobs list. Enter goes down one level, esc / b
//! come back up one, and q quits from anywhere.

use std::path::PathBuf;

use crate::app::App;
use crate::keys::Action;

/// Work the event loop does after an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    /// r: re-read the store now.
    Reload,
    /// t: attach to the selected step's tmux session.
    Tmux,
    /// w: render this write-up in the terminal.
    WriteUp(PathBuf),
    /// E: open the selected agent's manifest in $EDITOR.
    EditManifest,
    /// enter / o in the details overlay: open the highlighted link.
    OpenLink,
    /// o: open this URL in the browser.
    OpenUrl(String),
    /// y / Y / i: copy `text`; the status line says "copied `what`".
    Copy {
        text: String,
        what: String,
    },
}

/// Rows ctrl-d / ctrl-u move: half the frame.
fn half_page(size: (u16, u16)) -> usize {
    usize::from(size.1 / 2).max(1)
}

/// Apply `action` to `app`. `size` is the frame's (width, height).
pub fn apply(app: &mut App, action: Action, size: (u16, u16)) -> Effect {
    match action {
        Action::ForceQuit => return Effect::Quit,
        // Any key closes help, and does nothing else.
        _ if app.show_help => {
            app.show_help = false;
            return Effect::None;
        }
        Action::Quit => return Effect::Quit,
        Action::ToggleHelp => {
            app.show_help = true;
            return Effect::None;
        }
        Action::Reload => return Effect::Reload,
        _ => {}
    }
    if app.show_run_detail {
        return detail(app, action);
    }
    if app.log_view.is_some() {
        return log_view(app, action, size);
    }
    if app.runs_open() {
        return runs(app, action, size);
    }
    jobs(app, action, size)
}

/// The run-detail overlay. It sits over either screen, so it goes first.
fn detail(app: &mut App, action: Action) -> Effect {
    let links = crate::detail::focused_run(app).map_or(0, |r| crate::detail::run_links(r).len());
    let last = links.saturating_sub(1);
    match action {
        Action::Back | Action::Detail => {
            app.show_run_detail = false;
            app.detail_selected = 0;
        }
        Action::Down | Action::PageDown => {
            app.detail_selected = (app.detail_selected + 1).min(last);
        }
        Action::Up | Action::PageUp => app.detail_selected = app.detail_selected.saturating_sub(1),
        Action::Top => app.detail_selected = 0,
        Action::Bottom => app.detail_selected = last,
        Action::Open | Action::OpenInBrowser => return Effect::OpenLink,
        Action::Copy | Action::CopyUrl => match crate::clip::selected_link_target(app) {
            Some(target) => return copy(target.clone(), target),
            None => app.set_status("no link to copy"),
        },
        _ => {}
    }
    Effect::None
}

/// The log view: always the screen on top once open.
fn log_view(app: &mut App, action: Action, size: (u16, u16)) -> Effect {
    let (max, visible) = log_bounds(app, size);
    let scroll = |app: &mut App, up: bool, rows: usize| {
        for _ in 0..rows.min(max) {
            app.scroll_logs(up, max);
        }
    };
    match action {
        Action::Back => app.close_logs(),
        Action::Up => scroll(app, true, 1),
        Action::Down => scroll(app, false, 1),
        Action::PageUp => scroll(app, true, (visible / 2).max(1)),
        Action::PageDown => scroll(app, false, (visible / 2).max(1)),
        Action::Top => scroll(app, true, max),
        Action::Bottom => scroll(app, false, max),
        // `l` doubles as pan-right here — it can't mean "open logs".
        Action::Right | Action::Logs => app.pan_logs(true),
        Action::Left => app.pan_logs(false),
        Action::Copy => {
            if let Some(view) = &app.log_view {
                let text = agent_core::transcript::plain_text(&view.events);
                return copy(text, "the log".to_owned());
            }
        }
        Action::CopyId => {
            if let Some(view) = &app.log_view {
                let id = view.run.id.clone();
                return copy(id.clone(), id);
            }
        }
        Action::Search => app.search_begin(),
        Action::SearchNext => app.search_step(size.0, size.1, true),
        Action::SearchPrev => app.search_step(size.0, size.1, false),
        _ => {}
    }
    Effect::None
}

/// The log view's scroll bound and visible height for this frame: wrapping
/// depends on the width, so only the frame size knows how far it scrolls.
fn log_bounds(app: &App, size: (u16, u16)) -> (usize, usize) {
    let (width, visible) = crate::logs::body_metrics(size.0, size.1);
    let max = app
        .log_view
        .as_ref()
        .map_or(0, |view| {
            crate::logs::line_count(&view.events, width, view.scroll_x == 0)
        })
        .saturating_sub(visible);
    (max, visible)
}

/// A job's runs (or every job's, on the all-runs page).
fn runs(app: &mut App, action: Action, size: (u16, u16)) -> Effect {
    match action {
        Action::Back => app.close_job(),
        Action::Follow => app.toggle_follow(),
        // f / a on the all-runs page: only live chains, or every chain.
        Action::ShowRunning | Action::ShowAll if app.all_runs => {
            app.all_running_only = action == Action::ShowRunning;
            app.job_step_selected = 0;
        }
        Action::Up => app.dag_move(false),
        Action::Down => app.dag_move(true),
        Action::PageUp => app.dag_move_by(false, half_page(size)),
        Action::PageDown => app.dag_move_by(true, half_page(size)),
        Action::Top => app.dag_move_by(false, usize::MAX),
        Action::Bottom => app.dag_move_by(true, usize::MAX),
        Action::Open | Action::Logs => match app.dag_selected_run().cloned() {
            Some(run) => app.open_logs_for(run),
            None => app.set_status("this job has no runs yet"),
        },
        Action::Kill => app.kill_selected(),
        Action::Run => app.ask_run(),
        Action::Tmux => return Effect::Tmux,
        // The step's write-up, rendered — an investigation's OUTPUT.md, or
        // the chain's handover.
        Action::WriteUp => match (app.step_writeup(), app.dag_selected_run()) {
            (Some(path), _) => return Effect::WriteUp(path),
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
        // o / Y: the draft PR THIS step opened. Only the step that ran
        // `gh pr create` has one — on a docs or report step it says so
        // instead of using the chain's PR.
        Action::OpenInBrowser | Action::CopyUrl => {
            match (app.selected_step_pr(), app.dag_selected_run()) {
                (Some(pr), _) => {
                    let pr = pr.to_owned();
                    return if action == Action::CopyUrl {
                        copy(pr.clone(), pr)
                    } else {
                        Effect::OpenUrl(pr)
                    };
                }
                (None, Some(run)) => {
                    let msg = format!("{} opened no PR", run.agent);
                    app.set_status(&msg);
                }
                (None, None) => app.set_status("no step selected"),
            }
        }
        Action::Detail => {
            app.show_run_detail = true;
            app.detail_selected = 0;
        }
        Action::Edit => return Effect::EditManifest,
        // y / i: the step's run id (what `logs` and `resume` take).
        Action::Copy | Action::CopyId => match app.dag_selected_run() {
            Some(run) => return copy(run.id.clone(), run.id.clone()),
            None => app.set_status("nothing selected"),
        },
        _ => {}
    }
    Effect::None
}

/// The jobs list, the landing page.
fn jobs(app: &mut App, action: Action, size: (u16, u16)) -> Effect {
    match action {
        // esc / b on the landing page only clear a search; there is no
        // level above it to go back to.
        Action::Back => {
            app.job_search.clear();
            app.dag_selected = 0;
        }
        Action::Up => app.dag_move(false),
        Action::Down => app.dag_move(true),
        Action::PageUp => app.dag_move_by(false, half_page(size)),
        Action::PageDown => app.dag_move_by(true, half_page(size)),
        Action::Top => app.dag_move_by(false, usize::MAX),
        Action::Bottom => app.dag_move_by(true, usize::MAX),
        Action::Open => app.open_job(),
        Action::OpenInBrowser | Action::CopyUrl => {
            app.set_status("a job has no URL — a step's draft PR is on its runs screen (enter)");
        }
        Action::AllRuns => app.open_all_runs(),
        Action::Search => app.job_search_begin(),
        Action::Edit => return Effect::EditManifest,
        // f / a: running jobs only, or all of them. The cursor goes back
        // to the top — the old index points into the other list.
        Action::ShowRunning | Action::ShowAll => {
            app.job_running_only = action == Action::ShowRunning;
            app.dag_selected = 0;
        }
        Action::ToggleHidden => {
            app.show_hidden = !app.show_hidden;
            app.dag_selected = 0;
        }
        Action::Run => app.ask_run(),
        // y / i: the job's name (what `-j` and `run` take).
        Action::Copy | Action::CopyId => match app.selected_job() {
            Some(job) => {
                let name = job.spec.qualified_name();
                return copy(name.clone(), name);
            }
            None => app.set_status("nothing selected"),
        },
        _ => {}
    }
    Effect::None
}

fn copy(text: String, what: String) -> Effect {
    Effect::Copy { text, what }
}
