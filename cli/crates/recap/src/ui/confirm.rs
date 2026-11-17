//! The confirmation dialog for an action that changes GitHub.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::Confirm;

/// The title, colour and body lines of a confirmation dialog.
fn confirm_body(confirm: &Confirm) -> (&'static str, Color, Vec<Line<'static>>) {
    let dim = Style::default().fg(Color::DarkGray);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    match confirm {
        Confirm::Ready { label, undo, .. } => (
            if *undo {
                " move back to draft? "
            } else {
                " mark ready for review? "
            },
            Color::Cyan,
            vec![
                Line::from(vec![Span::raw(" "), Span::styled(label.clone(), bold)]),
                Line::styled(
                    if *undo {
                        " Converts the PR back to a draft on GitHub."
                    } else {
                        " Takes the PR out of draft so reviewers are asked to review it."
                    },
                    dim,
                ),
            ],
        ),
        Confirm::Close { label, .. } => (
            " close without merging? ",
            Color::Red,
            vec![
                Line::from(vec![Span::raw(" "), Span::styled(label.clone(), bold)]),
                Line::styled(" Closes the PR on GitHub (it can be reopened there),", dim),
                Line::styled(
                    " then moves this recap to the archive (A shows it, U restores).",
                    dim,
                ),
            ],
        ),
        Confirm::Merge {
            label,
            plan,
            choice,
            ..
        } => {
            let mut lines = vec![
                Line::from(vec![Span::raw(" "), Span::styled(label.clone(), bold)]),
                Line::styled(
                    if plan.auto {
                        " Checks are still running or an approval is pending: this turns on"
                    } else {
                        " Approved, every check green, and GitHub says it can merge."
                    },
                    dim,
                ),
            ];
            if plan.auto {
                lines.push(Line::styled(
                    " auto-merge — GitHub merges it once everything passes.",
                    dim,
                ));
            }
            lines.push(Line::styled(
                " Only merges the exact commit that was checked.",
                dim,
            ));
            lines.push(Line::default());
            for (i, m) in plan.methods.iter().enumerate() {
                let picked = i == *choice;
                lines.push(Line::from(vec![
                    Span::styled(
                        if picked { " ▶ " } else { "   " },
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(m.label(), if picked { bold } else { dim }),
                    Span::styled(if i == 0 { "  (repo default)" } else { "" }, dim),
                ]));
            }
            (
                if plan.auto {
                    " enable auto-merge? "
                } else {
                    " merge? "
                },
                Color::Green,
                lines,
            )
        }
    }
}

/// The confirmation dialog for an action that changes GitHub.
pub(super) fn draw_confirm(frame: &mut Frame<'_>, confirm: &Confirm) {
    let dim = Style::default().fg(Color::DarkGray);
    let (title, color, mut lines) = confirm_body(confirm);
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(" y/enter ", Style::default().fg(Color::Black).bg(color)),
        Span::styled(" do it   ", dim),
        Span::styled(
            " n/esc ",
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ),
        Span::styled(" cancel", dim),
    ]));
    let rect = beagle_tui::widgets::centered(
        frame.area(),
        72,
        u16::try_from(lines.len() + 2).unwrap_or(u16::MAX),
    );
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
                .title(title),
        ),
        rect,
    );
}
