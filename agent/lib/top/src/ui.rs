//! Rendering: the jobs screens (`dag.rs` — jobs, then a job's runs), the
//! log view on top when one is open, the footer, and the help overlay.
//! The `?` sheet and the footer are `beagle_tui::chrome`'s, the same in
//! every beagle TUI.
//!
//! There used to be a second top-level screen, an ACTIVE / RECENT / EVENTS
//! dashboard behind `d`. The jobs screens replaced everything it showed, so
//! it is gone.

use beagle_tui::chrome::{self, HelpFilter, Tone};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// The log view's footer hints.
const LOG_HINTS: &str =
    "esc:back  j/k:scroll  g/G:top/bottom  h/l:pan  /:search  y:copy  q:quit  ?:help";

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).areas(frame.area());
    // The log view first: a log is always the screen on top, whichever
    // screen opened it.
    if let Some(view) = &app.log_view {
        crate::logs::draw(frame, view, body);
    } else {
        crate::dag::draw(frame, app, body);
        if app.show_run_detail {
            crate::detail::draw_overlay(frame, app);
        }
        if let Some(confirm) = &app.confirm {
            crate::confirm::draw(frame, confirm);
        }
    }
    let status = app.status_message().map(|s| (s, Tone::Info));
    chrome::draw_footer(frame, footer, footer_hints(app), status, false);
    draw_daemon_bar(frame, app);
    if app.show_help {
        chrome::draw_help(frame, crate::keys::HELP, HelpFilter::Off);
    }
}

/// The footer's key hints for the screen on top.
fn footer_hints(app: &App) -> &'static str {
    if app.log_view.is_some() {
        LOG_HINTS
    } else if app.all_runs {
        crate::jobruns::ALL_RUNS_HINTS
    } else if app.runs_open() {
        crate::jobruns::RUNS_HINTS
    } else {
        crate::dag::JOBS_HINTS
    }
}

/// A stopped (or hung) agentd looks exactly like an idle one in the store:
/// the same rows, just nothing new. So when it is not running, the bottom
/// row of EVERY screen (the footer) says so, in red, over whatever is there.
fn draw_daemon_bar(frame: &mut Frame<'_>, app: &App) {
    let Some(text) = app
        .daemon
        .as_ref()
        .and_then(|d| d.banner(chrono::Utc::now()))
    else {
        return;
    };
    let area = frame.area();
    if area.height < 2 {
        return;
    }
    let bar = Rect {
        y: area.y + area.height - 1,
        height: 1,
        ..area
    };
    let hint = "  ·  start: launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.agentd.daemon.plist";
    frame.render_widget(Clear, bar);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" ■ {text} "),
                Style::new()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(hint, Style::new().fg(Color::Red)),
        ])),
        bar,
    );
}
