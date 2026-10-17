//! Full-screen log view for a single run: pretty, wrapped, scrollable, and
//! tailing live while the run is going.

use agent_core::types::Event;
use chrono::Local;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::LogView;
use crate::format::wrap_text;
use agent_core::transcript::prettify;

const LOG_KEY_HINTS: &str = "esc/q:back  j/k:scroll  h/l:pan  /:search  y:copy  ?:help";
/// Columns for the `HH:MM:SS ` prefix and the padded tag column.
const TS_COLS: usize = 9;
const TAG_COLS: usize = 12;

pub fn draw(frame: &mut Frame<'_>, view: &LogView, status: Option<&str>) {
    let [header, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(frame.area());
    draw_header(frame, view, status, header);
    draw_body(frame, view, body);
}

fn draw_header(frame: &mut Frame<'_>, view: &LogView, status: Option<&str>, area: Rect) {
    let run = &view.run;
    let mut spans = vec![
        Span::styled(
            " logs ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  {}/{}  ", run.project, run.agent)),
        Span::styled(
            run.state.to_string(),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  {}  ", run.id)),
    ];
    // What it cost, where you are when you want to know. The RECENT table has a
    // COST column, but by the time a run is interesting you are usually inside
    // its log and the table is off-screen.
    if let Some(c) = run.cost_usd {
        spans.push(Span::styled(
            format!(" {} ", crate::format::cost(Some(c))),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        if let Some(t) = run.num_turns {
            spans.push(Span::styled(
                format!(" {t} tool calls "),
                Style::new().add_modifier(Modifier::DIM),
            ));
        }
    }
    if view.search_input {
        // Live prompt: the term is being typed right now.
        spans.push(Span::styled(
            format!(" /{}_ ", view.search.clone().unwrap_or_default()),
            Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(term) = view.search.as_deref().filter(|t| !t.is_empty()) {
        spans.push(Span::styled(
            format!(" /{term} "),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        if let Some((at, total)) = view.search_at {
            spans.push(Span::styled(
                format!(" {at}/{total} "),
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(
            " n:fwd  N:back  esc:clear ",
            Style::new().add_modifier(Modifier::DIM),
        ));
    } else if let Some(status) = status {
        spans.push(Span::styled(
            format!(" {status} "),
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            LOG_KEY_HINTS,
            Style::new().add_modifier(Modifier::DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Body width and visible height for a given frame size.
///
/// `draw` splits the frame into a 1-row header and the bordered body, so the
/// body wraps at `width - 2` and shows `height - 3` lines. Search has to use
/// the SAME numbers: wrapping decides how many lines each event becomes, so a
/// different width yields different line indices and a hit lands on the wrong
/// row. (It did — search computed at a hardcoded 80.)
pub fn body_metrics(frame_width: u16, frame_height: u16) -> (usize, usize) {
    (
        usize::from(frame_width.saturating_sub(2)),
        usize::from(frame_height.saturating_sub(3)).max(1),
    )
}

fn draw_body(frame: &mut Frame<'_>, view: &LogView, area: Rect) {
    let width = usize::from(area.width.saturating_sub(2));
    // Panned at all → unwrapped lines shifted left; at column 0 → wrapped.
    let wrap = view.scroll_x == 0;
    let mut lines = build_lines(&view.events, width, wrap);
    if let Some(term) = view.search.as_deref().filter(|t| !t.is_empty()) {
        lines = lines
            .iter()
            .enumerate()
            .map(|(i, l)| highlight(l, term, Some(i) == view.search_line))
            .collect();
    }
    let block = Block::bordered().title(Span::styled(
        format!(" EVENTS ({}) ", view.events.len()),
        Style::new().add_modifier(Modifier::BOLD),
    ));
    if lines.is_empty() {
        let hint = "no events for this run (its logs may have been pruned — \
                    see agentd --log-retention-hours)";
        let text = Paragraph::new(hint)
            .style(Style::new().add_modifier(Modifier::DIM))
            .wrap(Wrap { trim: true })
            .block(block);
        frame.render_widget(text, area);
        return;
    }

    let visible = usize::from(area.height.saturating_sub(2));
    let (start, end) = window(lines.len(), visible, view.scroll_from_bottom);
    frame.render_widget(
        Paragraph::new(lines[start..end].to_vec())
            .scroll((0, view.scroll_x))
            .block(block),
        area,
    );
}

/// Render the events as display lines for a body `width` columns wide:
/// prettified, continuation lines indented under the tag. With `wrap`, bodies
/// word-wrap to the pane; without it, lines run long for horizontal panning.
/// Highlight every occurrence of `needle` (already lowercased) in a line.
/// Case-insensitive, and it preserves the original casing in the output.
///
/// `current` marks the line `n`/`N` is sitting on: its matches get a
/// different colour, because when every hit looks identical you cannot tell
/// whether `n` moved.
pub fn highlight(line: &Line<'static>, needle: &str, current: bool) -> Line<'static> {
    if needle.is_empty() {
        return line.clone();
    }
    let mut out: Vec<Span<'static>> = Vec::new();
    for span in &line.spans {
        let text = span.content.to_string();
        let hay = text.to_lowercase();
        if !hay.contains(needle) {
            out.push(span.clone());
            continue;
        }
        let mut at = 0usize;
        while let Some(found) = hay[at..].find(needle) {
            let s = at + found;
            let e = s + needle.len();
            if s > at {
                out.push(Span::styled(text[at..s].to_owned(), span.style));
            }
            out.push(Span::styled(
                text[s..e].to_owned(),
                if current {
                    // The one you are on: reversed magenta, unmistakable
                    // against the yellow of the others.
                    Style::new()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new().fg(Color::Black).bg(Color::Yellow)
                },
            ));
            at = e;
        }
        if at < text.len() {
            out.push(Span::styled(text[at..].to_owned(), span.style));
        }
    }
    Line::from(out)
}

pub fn build_lines(events: &[Event], width: usize, wrap: bool) -> Vec<Line<'static>> {
    let body_width = width.saturating_sub(TS_COLS + TAG_COLS).max(20);
    let mut out = Vec::new();
    for event in events {
        for entry in prettify(event) {
            let ts = entry.ts.with_timezone(&Local).format("%H:%M:%S");
            let style = tag_style(&entry.tag);
            let body_style = body_style(&entry.tag, &entry.body);
            let body_lines: Vec<String> = if wrap {
                wrap_text(&entry.body, body_width)
            } else {
                let mut lines: Vec<String> = entry.body.lines().map(str::to_owned).collect();
                if lines.is_empty() {
                    lines.push(String::new());
                }
                lines
            };
            for (i, text) in body_lines.into_iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![
                        Span::styled(format!("{ts} "), Style::new().add_modifier(Modifier::DIM)),
                        Span::styled(format!("{:<11} ", entry.tag), style),
                    ];
                    // A script line's source (`gather` in `gather: wrote 6
                    // files`) in its own colour. Part of the line's text,
                    // so `/` finds it like any other word.
                    if let Some(source) = &entry.source {
                        spans.push(Span::styled(
                            format!("{source} "),
                            Style::new().fg(Color::Cyan),
                        ));
                    }
                    spans.push(Span::styled(text, body_style));
                    out.push(Line::from(spans));
                } else {
                    out.push(Line::from(vec![
                        Span::raw(" ".repeat(TS_COLS + TAG_COLS)),
                        Span::styled(text, body_style),
                    ]));
                }
            }
        }
    }
    out
}

/// The plain text of every rendered line, for searching. Mirrors
/// [`build_lines`] exactly, so a hit's index is the index of the line the
/// user is looking at — searching the raw events instead would point at the
/// wrong row once one event expands into several lines.
pub fn plain_lines(events: &[Event], width: usize, wrap: bool) -> Vec<String> {
    build_lines(events, width, wrap)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

/// Total display lines at `width` — the vertical scroll bound.
pub fn line_count(events: &[Event], width: usize, wrap: bool) -> usize {
    build_lines(events, width, wrap).len()
}

fn tag_style(tag: &str) -> Style {
    match tag {
        // Both kinds of run.
        "state" => Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        "note" | "hook" => Style::new().fg(Color::Yellow),
        // A model run: the conversation.
        "assistant" => Style::new().add_modifier(Modifier::BOLD),
        "thinking" => Style::new()
            .fg(Color::Magenta)
            .add_modifier(Modifier::DIM | Modifier::ITALIC),
        "tool_use" => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        "tool_result" => Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
        "init" | "system" => Style::new().fg(Color::Yellow).add_modifier(Modifier::DIM),
        "clipped" => Style::new()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        // A script run: the log levels.
        // Something went wrong, from either kind of run.
        "error" | "denied" => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        "warn" => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        // Something finished well, from either kind of run.
        "ok" | "result" => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        "step" => Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
        "info" => Style::new().fg(Color::Blue),
        // out, debug, and anything new: quiet by default.
        _ => Style::new().add_modifier(Modifier::DIM),
    }
}

/// The body follows its tag where the tag is a judgement — an error reads
/// red all the way across, not just in the margin — and stays plain where
/// the body is the content (a model's answer, a tool's command).
fn body_style(tag: &str, body: &str) -> Style {
    match tag {
        "error" | "denied" => Style::new().fg(Color::Red),
        "warn" => Style::new().fg(Color::Yellow),
        "ok" => Style::new().fg(Color::Green),
        "thinking" | "tool_result" | "out" | "debug" | "clipped" | "init" | "system" => {
            Style::new().add_modifier(Modifier::DIM)
        }
        "state" => match body {
            "succeeded" => Style::new().fg(Color::Green),
            "failed" | "killed" | "timedout" => Style::new().fg(Color::Red),
            _ => Style::new(),
        },
        _ => Style::new(),
    }
}

/// The `[start, end)` slice of `len` lines to show in a pane `visible` lines
/// tall, scrolled `from_bottom` lines up from the tail.
fn window(len: usize, visible: usize, from_bottom: usize) -> (usize, usize) {
    let max_scroll = len.saturating_sub(visible);
    let scroll = from_bottom.min(max_scroll);
    let end = len - scroll;
    (end.saturating_sub(visible), end)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
#[path = "tests/logs.rs"]
mod tests;
