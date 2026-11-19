//! The three tab bodies. Each records how far it can scroll in
//! `app.max_scroll`, so keys can clamp without knowing the layout.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use tandem_core::github::ReviewVerdict;
use tandem_core::review::{CommentSeverity, CommentStatus, LocalComment};

use super::view::verdict_style;
use super::{markdown, App, Loaded};
use crate::state::Stage;
use crate::verdict::Verdict;

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// A wrapped paragraph scrolled to `scroll`; returns its max scroll.
fn scrolled(frame: &mut Frame<'_>, lines: Vec<Line<'static>>, area: Rect, scroll: usize) -> usize {
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total = para.line_count(area.width);
    let max = total.saturating_sub(usize::from(area.height));
    let y = u16::try_from(scroll.min(max)).unwrap_or(u16::MAX);
    frame.render_widget(para.scroll((y, 0)), area);
    max
}

pub fn summary(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut lines = Vec::new();
    if let (Some(row), Some(l)) = (app.selected(), app.loaded.as_ref()) {
        lines.push(Line::styled(row.pr.url(), Style::default().fg(Color::Blue)));
        if let Some(d) = &l.detail {
            let pr = &d.pull_request;
            lines.push(Line::styled(
                format!(
                    "@{} · {} → {} · {} files +{} −{}",
                    pr.author.login,
                    pr.head_ref,
                    pr.base_ref,
                    pr.changed_files,
                    pr.additions,
                    pr.deletions
                ),
                dim(),
            ));
            if l.head.as_deref().is_some_and(|h| h != pr.head_sha) {
                lines.push(Line::styled(
                    "New commits since this review — r to review again.",
                    Style::default().fg(Color::Yellow),
                ));
            }
            lines.push(github_reviews(d));
        }
        let state = match (row.stage, row.started_at) {
            (Stage::Reviewing, Some(at)) => {
                let mins = (chrono::Utc::now() - at).num_minutes();
                format!("reviewing for {mins}m")
            }
            (stage, _) => stage.label().to_owned(),
        };
        let mut status = vec![Span::styled(
            state,
            Style::default().add_modifier(Modifier::BOLD),
        )];
        if row.approved {
            status.push(Span::styled(
                " · approved by you",
                Style::default().fg(Color::Green),
            ));
        }
        lines.push(Line::from(status));
        if let Some(err) = &row.error {
            lines.push(Line::styled(
                format!("error: {err}"),
                Style::default().fg(Color::Red),
            ));
        }
        lines.push(Line::default());
        match &l.summary {
            Some(s) => {
                let mut body = markdown::render(&s.body);
                if let (Some(v), Some(first)) = (Verdict::from_summary(&s.body), body.first_mut()) {
                    *first = first.clone().patch_style(verdict_style(v));
                }
                lines.extend(body);
            }
            None => lines.push(Line::styled(placeholder(row.stage), dim())),
        }
    }
    app.max_scroll = scrolled(frame, lines, area, app.scroll);
}

fn placeholder(stage: Stage) -> &'static str {
    match stage {
        Stage::Queued => "Queued — waiting for the agentd reviewer to pick it up.",
        Stage::Reviewing => "The agent is reviewing this PR.",
        Stage::Failed => "The review failed (see the error above). r to try again.",
        Stage::Ready | Stage::Done => "No summary was written.",
    }
}

fn github_reviews(d: &tandem_core::github::PrDetail) -> Line<'static> {
    let mut spans = vec![Span::styled("GitHub: ", dim())];
    if d.reviews.is_empty() {
        spans.push(Span::styled("no reviews yet", dim()));
    }
    for (i, r) in d.reviews.iter().enumerate() {
        let (word, color) = match r.verdict {
            ReviewVerdict::Approved => ("approved", Color::Green),
            ReviewVerdict::ChangesRequested => ("changes requested", Color::Red),
            ReviewVerdict::Commented => ("commented", Color::Gray),
            ReviewVerdict::Dismissed => ("dismissed", Color::DarkGray),
            ReviewVerdict::Pending => ("pending", Color::DarkGray),
        };
        if i > 0 {
            spans.push(Span::styled(", ", dim()));
        }
        spans.push(Span::styled(
            format!("@{} {word}", r.author.login),
            Style::default().fg(color),
        ));
    }
    spans.push(Span::styled(
        format!(" · {} comments", d.comments.len()),
        dim(),
    ));
    Line::from(spans)
}

pub fn diff(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let App {
        diff,
        loaded,
        scroll,
        max_scroll,
        ..
    } = app;
    let Some(Loaded {
        pr,
        head: Some(head),
        raw_diff: Some(raw),
        ..
    }) = loaded.as_ref()
    else {
        *max_scroll = 0;
        frame.render_widget(
            Paragraph::new("No diff yet — it is fetched when the review starts.").style(dim()),
            area,
        );
        return;
    };
    let rendered = diff.get(&pr.label(), head, raw, area.width);
    let height = usize::from(area.height);
    *max_scroll = rendered.lines.len().saturating_sub(height);
    let from = (*scroll).min(*max_scroll);
    let lines: Vec<Line<'static>> = rendered
        .lines
        .iter()
        .skip(from)
        .take(height)
        .cloned()
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn severity_style(s: CommentSeverity) -> Style {
    match s {
        CommentSeverity::Blocker => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        CommentSeverity::Issue => Style::default().fg(Color::Red),
        CommentSeverity::Suggestion => Style::default().fg(Color::Cyan),
        CommentSeverity::Info => Style::default().fg(Color::Gray),
    }
}

fn severity_word(s: CommentSeverity) -> &'static str {
    match s {
        CommentSeverity::Blocker => "blocker",
        CommentSeverity::Issue => "issue",
        CommentSeverity::Suggestion => "suggestion",
        CommentSeverity::Info => "info",
    }
}

fn place(c: &LocalComment) -> String {
    match (c.path.is_empty(), c.end_line) {
        (true, _) => "PR".to_owned(),
        (false, Some(end)) if end != c.line => format!("{}:{}-{end}", c.path, c.line),
        (false, _) => format!("{}:{}", c.path, c.line),
    }
}

fn state_mark(c: &LocalComment) -> Span<'static> {
    if c.posted_github_id.is_some() {
        Span::styled("posted ", Style::default().fg(Color::Green))
    } else if c.status == CommentStatus::Rejected {
        Span::styled("dismissed ", dim())
    } else {
        Span::raw("")
    }
}

pub fn comments(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let Some(l) = app.loaded.as_ref() else {
        app.max_scroll = 0;
        return;
    };
    if l.comments.is_empty() {
        app.max_scroll = 0;
        frame.render_widget(
            Paragraph::new("No comments from the agent.").style(dim()),
            area,
        );
        return;
    }
    let list_h = u16::try_from(l.comments.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height / 3)
        .max(4);
    let [top, bottom] =
        Layout::vertical([Constraint::Length(list_h), Constraint::Min(3)]).areas(area);
    let items: Vec<ListItem<'_>> = l
        .comments
        .iter()
        .map(|c| {
            let first = c.body.lines().next().unwrap_or("").to_owned();
            let mut spans = vec![
                state_mark(c),
                Span::styled(
                    format!("{:<10} ", severity_word(c.severity)),
                    severity_style(c.severity),
                ),
            ];
            if let Some(tag) = &c.tag {
                let style = if tag == "duplicate" {
                    Style::default().fg(Color::Magenta)
                } else {
                    dim()
                };
                spans.push(Span::styled(format!("{tag} "), style));
            }
            spans.push(Span::styled(
                format!("{} ", place(c)),
                Style::default().fg(Color::Blue),
            ));
            spans.push(Span::raw(first));
            let line = Line::from(spans);
            if c.status == CommentStatus::Rejected {
                ListItem::new(line).style(dim())
            } else {
                ListItem::new(line)
            }
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(dim()),
        )
        .highlight_style(Style::default().bg(Color::Rgb(40, 44, 52)))
        .highlight_symbol("▸ ");
    let mut state = ListState::default().with_selected(Some(app.comment_sel));
    frame.render_stateful_widget(list, top, &mut state);

    let lines = l
        .comments
        .get(app.comment_sel)
        .map(|c| detail(l, c))
        .unwrap_or_default();
    app.max_scroll = scrolled(frame, lines, bottom, app.detail_scroll);
}

fn detail(l: &Loaded, c: &LocalComment) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(severity_word(c.severity), severity_style(c.severity)),
        Span::styled(
            format!(
                " · {} · {} · {}",
                c.tag.as_deref().unwrap_or("-"),
                place(c),
                c.author_name
            ),
            dim(),
        ),
    ])];
    if c.posted_github_id.is_some() {
        lines.push(Line::styled(
            "Posted to GitHub.",
            Style::default().fg(Color::Green),
        ));
    } else if c.status == CommentStatus::Rejected {
        lines.push(Line::styled("Dismissed (x to restore).", dim()));
    }
    lines.push(Line::default());
    lines.extend(markdown::render(&c.body));
    if let Some(fix) = &c.suggestion {
        lines.push(Line::default());
        lines.push(Line::styled(
            "Suggested change:",
            Style::default().add_modifier(Modifier::BOLD),
        ));
        for code in fix.lines() {
            lines.push(Line::styled(
                format!("  {code}"),
                Style::default().fg(Color::Green),
            ));
        }
    }
    for reply in l.replies.get(&c.id).into_iter().flatten() {
        lines.push(Line::default());
        lines.push(Line::styled(
            "── history (local, never posted) ──",
            Style::default().fg(Color::Magenta),
        ));
        lines.extend(markdown::render(&reply.body));
    }
    lines
}
