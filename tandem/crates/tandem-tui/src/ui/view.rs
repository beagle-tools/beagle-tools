//! Drawing: the queue, the tab bar, the footer and the overlays. The tab
//! bodies are in [`super::tabs`]. Pure apart from recording how far the
//! current tab can scroll.

use beagle_tui::pane_resize;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use super::keys::HELP;
use super::{tabs, Action, App, Focus, Mode, Tab};
use crate::state::{Row, Stage};
use crate::verdict::Verdict;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let [main, footer] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    if app.zoom {
        draw_content(frame, app, main);
    } else {
        let list_width = app.pane.width(main.width);
        let [left, right] =
            Layout::horizontal([Constraint::Length(list_width), Constraint::Min(20)]).areas(main);
        draw_list(frame, app, left);
        draw_content(frame, app, right);
    }
    draw_footer(frame, app, footer);
    match &app.mode {
        Mode::Help => draw_help(frame),
        Mode::Confirm(action) => draw_confirm(frame, app, action),
        Mode::Normal | Mode::Input(_) => {}
    }
}

fn border(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

pub fn verdict_style(v: Verdict) -> Style {
    let color = match v {
        Verdict::Mergeable => Color::Green,
        Verdict::WithSuggestions => Color::Cyan,
        Verdict::WithUnknowns => Color::Yellow,
        Verdict::NotMergeable => Color::Red,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn stage_glyph(row: &Row, tick: usize) -> Span<'static> {
    let (glyph, color) = match row.stage {
        Stage::Ready => ("●", Color::Green),
        Stage::Reviewing => (beagle_tui::widgets::spinner(tick), Color::Yellow),
        Stage::Queued => ("◌", Color::DarkGray),
        Stage::Failed => ("✖", Color::Red),
        Stage::Done => ("✓", Color::DarkGray),
        Stage::Merged => ("⇡", Color::Magenta),
        Stage::Closed => ("⊘", Color::DarkGray),
    };
    Span::styled(glyph, Style::default().fg(color))
}

fn draw_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(4));
    let items: Vec<ListItem<'_>> = app
        .rows
        .iter()
        .map(|row| {
            let dim = matches!(row.stage, Stage::Done | Stage::Merged | Stage::Closed);
            let mut head = vec![stage_glyph(row, app.tick), Span::raw(" ")];
            let label_style = if dim {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            head.push(Span::styled(row.pr.label(), label_style));
            if let Some(v) = row.verdict {
                head.push(Span::raw(" "));
                head.push(Span::styled(v.badge(), verdict_style(v)));
            }
            if row.open_comments > 0 && !dim {
                head.push(Span::styled(
                    format!(" {}✎", row.open_comments),
                    Style::default().fg(Color::Magenta),
                ));
            }
            if row.approved {
                head.push(Span::styled(" ✔", Style::default().fg(Color::Green)));
            }
            let sub = row
                .title
                .clone()
                .unwrap_or_else(|| row.stage.label().to_owned());
            let sub: String = sub.chars().take(width.saturating_sub(2)).collect();
            ListItem::new(vec![
                Line::from(head),
                Line::styled(format!("  {sub}"), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();
    let ready = app.rows.iter().filter(|r| r.stage == Stage::Ready).count();
    let title = format!(" Reviews · {ready} ready ");
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border(app.focus == Focus::List))
                .title(title),
        )
        .highlight_style(Style::default().bg(Color::Rgb(40, 44, 52)));
    let mut state = ListState::default().with_selected((!app.rows.is_empty()).then_some(app.sel));
    frame.render_stateful_widget(list, area, &mut state);
    if app.rows.is_empty() {
        let hint = Paragraph::new(
            "Nothing queued.\n\nPress a to queue a PR,\nor run `beagle tandem add <url>`.",
        )
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, inset(area));
    }
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(3),
    }
}

fn draw_content(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = app.selected().map_or(" No PR selected ".to_owned(), |r| {
        format!(" {} · {} ", r.pr.label(), r.title.as_deref().unwrap_or("…"))
    });
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border(app.focus == Focus::Content))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [bar, body] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
    let count = app.loaded.as_ref().map_or(0, |l| l.comments.len());
    let titles: Vec<String> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| match t {
            Tab::Comments => format!("{} {} ({count})", i + 1, t.title()),
            _ => format!("{} {}", i + 1, t.title()),
        })
        .collect();
    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
    let tab_bar = Tabs::new(titles)
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");
    frame.render_widget(tab_bar, bar);
    app.page = usize::from(body.height.max(1));
    match app.tab {
        Tab::Summary => tabs::summary(frame, app, body),
        Tab::Diff => tabs::diff(frame, app, body),
        Tab::Comments => tabs::comments(frame, app, body),
    }
}

fn draw_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.pane.is_active() {
        let line = Line::from(vec![
            Span::styled(
                format!(" {}", pane_resize::HINT),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                app.status
                    .as_ref()
                    .map(|(m, _)| format!("   {m}"))
                    .unwrap_or_default(),
                Style::default().fg(Color::Yellow),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }
    let line = match (&app.mode, &app.status) {
        (Mode::Input(text), _) => Line::from(vec![
            Span::styled(" queue PR: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{text}█")),
            Span::styled("   Enter queue · Esc cancel", Style::default().fg(Color::DarkGray)),
        ]),
        (_, Some((msg, true))) => Line::styled(format!(" {msg}"), Style::default().fg(Color::Red)),
        (_, Some((msg, false))) => Line::styled(format!(" {msg}"), Style::default().fg(Color::Green)),
        (_, None) => Line::styled(
            " a queue · Enter open · z zoom · w resize · p post · A approve · m gone-through · r re-review · ? help · q quit",
            Style::default().fg(Color::DarkGray),
        ),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_help(frame: &mut Frame<'_>) {
    let lines: Vec<Line<'_>> = HELP
        .iter()
        .map(|(keys, what)| {
            Line::from(vec![
                Span::styled(format!("{keys:>22}  "), Style::default().fg(Color::Cyan)),
                Span::raw(*what),
            ])
        })
        .collect();
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let area = centered(frame.area(), 76, height);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Keys ")),
        area,
    );
}

fn draw_confirm(frame: &mut Frame<'_>, app: &App, action: &Action) {
    let account = app
        .config
        .github_user
        .clone()
        .unwrap_or_else(|| "gh's active account".to_owned());
    let (title, text) = match action {
        Action::Post { preview, .. } => (
            format!(" Post this comment to GitHub as {account}? "),
            preview.clone(),
        ),
        Action::Approve(pr) => (
            format!(" Approve {} as {account}? ", pr.label()),
            "This submits an APPROVE review on GitHub.".to_owned(),
        ),
    };
    let area = centered(frame.area(), 90, 20);
    frame.render_widget(Clear, area);
    let [body, keys] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
        .areas(Block::default().borders(Borders::ALL).inner(area));
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(title),
        area,
    );
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), body);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "y yes · n no",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        keys,
    );
}
