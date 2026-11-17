//! Rendering: the jobs screens (`dag.rs` — jobs, then a job's runs), the
//! log view on top when one is open, and the help overlay.
//!
//! There used to be a second top-level screen, an ACTIVE / RECENT / EVENTS
//! dashboard behind `d`. The jobs screens replaced everything it showed, so
//! it is gone.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    // The log view first: a log is always the screen on top, whichever
    // screen opened it.
    if let Some(view) = &app.log_view {
        crate::logs::draw(frame, view, app.status_message());
    } else {
        crate::dag::draw(frame, app);
        if app.show_run_detail {
            crate::detail::draw_overlay(frame, app);
        }
        if let Some(confirm) = &app.confirm {
            crate::confirm::draw(frame, confirm);
        }
    }
    draw_daemon_bar(frame, app);
    if app.show_help {
        draw_help(frame);
    }
}

/// A stopped (or hung) agentd looks exactly like an idle one in the store:
/// the same rows, just nothing new. So when it is not running, the bottom
/// row of EVERY screen says so, in red, over whatever is there.
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

fn draw_help(frame: &mut Frame<'_>) {
    let lines = [
        "jobs         j/k select a job · enter its runs · / search",
        "             R run it (asks first) · r only running jobs · a all jobs",
        "             e edit manifest · y copy its name (work/pr-watch)",
        "             A every job's runs on one live page",
        "             H show the projects ui.yaml hides (demo) · H again hides",
        "runs         j/k select a step · enter its log · w its write-up",
        "             o its draft PR · m details · x kill it if running",
        "             R resume the chain at this failed step (within a week),",
        "               or run the job again from the start (asks first)",
        "             t attach to its tmux session (detach: ctrl-b d)",
        "             y copy its run id (for agentctl logs / resume)",
        "all runs     the runs screen for every job, newest first, live",
        "             r only chains still running · a every chain again",
        "             W watch: the cursor rides the newest step (j/k stops it)",
        "logs         j/k scroll · h/l pan · / search · n/N next/prev · y copy",
        "",
        "esc          back one level · q quits from the jobs list",
        "ctrl-c       quit from anywhere (the screens refresh themselves)",
        "? / esc      close this help",
    ];
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let area = centered_rect(72, height, frame.area());
    let text: Vec<Line<'_>> = lines.iter().map(|line| Line::from(*line)).collect();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .block(crate::dag::pane_block("HELP"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}
