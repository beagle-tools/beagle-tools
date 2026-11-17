//! Small status pieces of the view: review / CI badges, comment counts,
//! and the content pane's tab strip.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::{App, Tab};
use crate::poller::{PrStatus, Review};

/// Review + CI badges for a PR: `✓ approved`, `○ review`, `✗ changes`, and
/// `CI ✓` / `CI ✗` / `CI …`. Short form for the narrow sidebar.
pub(super) fn badges(status: &PrStatus, short: bool) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let (text, color) = match status.review {
        Some(Review::Approved) => (if short { "✓" } else { "✓ approved" }, Color::Green),
        Some(Review::ChangesRequested) => (
            if short {
                "✗"
            } else {
                "✗ changes requested"
            },
            Color::Red,
        ),
        Some(Review::Required) => (if short { "○" } else { "○ needs review" }, Color::Yellow),
        None => ("", Color::Reset),
    };
    if !text.is_empty() {
        out.push(Span::raw(" "));
        out.push(Span::styled(text, Style::default().fg(color)));
    }
    let c = status.checks;
    if c.any() {
        let (text, color) = if c.failed > 0 {
            (
                format!(
                    "CI ✗{}",
                    if short {
                        String::new()
                    } else {
                        format!(" {} failing", c.failed)
                    }
                ),
                Color::Red,
            )
        } else if c.pending > 0 {
            ("CI …".to_owned(), Color::Yellow)
        } else {
            ("CI ✓".to_owned(), Color::Green)
        };
        out.push(Span::raw(" "));
        out.push(Span::styled(text, Style::default().fg(color)));
    }
    out
}

/// The PR list's comment badges, in words: `2 new` threads you have not
/// read, `2 rev` reviewed by the agent, `1 due` waiting on a review.
pub(super) fn comment_badges(m: crate::comments::Marks) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (n, word, color) in [
        (m.unread, "new", Color::Yellow),
        (m.reviewed, "rev", Color::Magenta),
        (m.pending, "due", Color::DarkGray),
    ] {
        if n > 0 {
            out.push(Span::raw(" "));
            out.push(Span::styled(
                format!("{n} {word}"),
                Style::default().fg(color),
            ));
        }
    }
    out
}

pub(super) fn tab_title(app: &App) -> Line<'static> {
    let on = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let off = Style::default().fg(Color::DarkGray);
    let (s, d, c) = match app.tab {
        Tab::Summary => (on, off, off),
        Tab::Diff => (off, on, off),
        Tab::Comments => (off, off, on),
    };
    let marks = app
        .selected_row()
        .and_then(|r| app.comment_marks.get(&r.id))
        .copied()
        .unwrap_or_default();
    let comments_label = match (marks.total, marks.unread) {
        (0, _) => " 3 Comments ".to_owned(),
        (total, 0) => format!(" 3 Comments ({total}) "),
        (total, new) => format!(" 3 Comments ({total} · {new} new) "),
    };
    let files = app.file_starts.len();
    let diff_label = if app.tab == Tab::Diff && files > 0 {
        format!(" 2 Diff ({files} files) ")
    } else {
        " 2 Diff ".to_owned()
    };
    Line::from(vec![
        Span::raw(" "),
        Span::styled(" 1 Summary ", s),
        Span::raw(" "),
        Span::styled(diff_label, d),
        Span::raw(" "),
        Span::styled(comments_label, c),
        Span::raw(" "),
    ])
}
