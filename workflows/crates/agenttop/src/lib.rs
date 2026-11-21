//! agenttop — a live TUI over the agentd store: jobs → runs → logs.
//!
//! Thin entry point ([`run_as`]): terminal setup/teardown plus the event
//! loop. Data lives in `app`, rendering in `ui`, input mapping in `keys`,
//! what each key does in `input`, tmux glue in `tmux`, pure helpers in
//! `format`.

mod app;
mod clip;
mod confirm;
mod dag;
mod detail;
mod format;
mod highlight;
mod input;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
#[path = "tests/keymap.rs"]
mod keymap_tests;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Mirrors the key agentd writes when retention hits a problem. Duplicated
/// rather than shared because agenttop depends on agent-core, not agentd.
pub const RETENTION_PROBLEM_KEY: &str = "retention_problem";
use std::ffi::OsString;
use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches, Parser};
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::input::Effect;

const POLL_TIMEOUT: Duration = Duration::from_millis(250);
const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// The standalone TUI's program name.
pub const STANDALONE: &str = "agenttop";

/// btop-style live dashboard over the agentd store. Takes no arguments
/// beyond `--help` / `--version`; everything else is interactive.
#[derive(Debug, Parser)]
#[command(name = "agenttop", version, about = "Live dashboard for agentd runs")]
struct Cli {}

/// Open the dashboard as `program` (`agenttop`, or `beagle agent top`
/// under the unified command) with the arguments after it.
pub fn run_as(program: &str, args: Vec<OsString>) -> ExitCode {
    let command_line = std::iter::once(OsString::from(program)).chain(args);
    let parsed = Cli::command()
        .name(program.to_owned())
        .bin_name(program.to_owned())
        .try_get_matches_from(command_line)
        .and_then(|m| Cli::from_arg_matches(&m));
    if let Err(e) = parsed {
        let _ = e.print();
        return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
    }
    match open() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn open() -> Result<()> {
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
                } else if key.kind == KeyEventKind::Press {
                    let size = terminal.size().context("terminal size")?;
                    let action = keys::map_key(key);
                    let effect = input::apply(&mut app, action, (size.width, size.height));
                    if execute(terminal, &mut app, effect)? {
                        return Ok(());
                    }
                }
            }
        }
        if last_refresh.elapsed() >= REFRESH_EVERY {
            app.refresh();
            last_refresh = Instant::now();
        }
    }
}

/// Carry out what `input::apply` asked for. Returns `true` to exit.
fn execute(terminal: &mut DefaultTerminal, app: &mut App, effect: Effect) -> Result<bool> {
    match effect {
        Effect::None => {}
        Effect::Quit => return Ok(true),
        Effect::Reload => {
            app.refresh();
            app.set_status("reloaded");
        }
        Effect::Tmux => attach_selected(terminal, app)?,
        Effect::WriteUp(path) => {
            if let Some(message) = tmux::render_markdown(terminal, &path)? {
                app.set_status(&message);
            }
        }
        Effect::EditManifest => edit_selected_manifest(terminal, app)?,
        Effect::OpenLink => clip::open_selected_link(terminal, app)?,
        Effect::OpenUrl(url) => {
            let message = match beagle_tui::links::open_url(&url) {
                Ok(()) => format!("opened {url}"),
                Err(e) => format!("could not open {url}: {e}"),
            };
            app.set_status(&message);
        }
        Effect::Copy { text, what } => {
            let message = match beagle_tui::clipboard::copy(&text) {
                Ok(_) => format!("copied {what}"),
                Err(e) => format!("copy failed: {e}"),
            };
            app.set_status(&message);
        }
    }
    Ok(false)
}

/// `E`: open the selected agent's manifest in $EDITOR. The daemon
/// reconciles the change within a tick of the editor closing.
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

/// `t`: attach to the selected step's tmux session while it runs (a
/// finished run has none; its log is the story).
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
