//! The Comments tab: one thread at a time, picked from a strip of sub-tabs
//! (`n`/`N` or `]`/`[`), showing only what **people** wrote — bots and the
//! agent's automated replies are left out. Beside it, the agent's review of
//! that thread (`J`/`K` scroll it), in the same voice as the Summary.
//!
//! Showing a thread is reading it; `u` marks it unread again. All of it is
//! local — see [`crate::comments`].

use beagle_tui::doc::dim;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::App;
use crate::comments::{self, Comment, CommentsConfig, Need, Thread, ThreadKind};
use crate::markdown;
use crate::store::Recap;
use crate::wrap::DocLine;

/// The tab's document and every thread's id (the sub-tabs, in order).
pub(super) struct ThreadDoc {
    pub lines: Vec<DocLine>,
    pub ids: Vec<String>,
}

/// Widest a thread's text runs, however wide the pane.
const TEXT_WIDTH: u16 = 100;

/// `lines` wrapped to `width` minus the gutter, with `prefix` in front of
/// every row (wrapped ones too), so a comment reads as one block.
fn indented(lines: &[DocLine], prefix: &'static str, style: Style, width: u16) -> Vec<DocLine> {
    let inner = width
        .min(TEXT_WIDTH)
        .saturating_sub(u16::try_from(prefix.chars().count()).unwrap_or(0))
        .max(20);
    crate::wrap::layout(lines, inner)
        .into_iter()
        .map(|mut line| {
            line.spans.insert(0, Span::styled(prefix, style));
            DocLine::pre(line)
        })
        .collect()
}

fn when(rfc3339: &str) -> String {
    // 2026-11-01T11:00:00Z → 2026-11-01 11:00
    rfc3339.get(..16).unwrap_or(rfc3339).replace('T', " ")
}

/// Whether a comment is shown: people only — no bots (ignored authors), no
/// automated replies (bodies with a configured marker).
fn is_person(c: &Comment, config: &CommentsConfig) -> bool {
    !c.context && !config.is_automated(&c.body)
}

/// Who started the thread (the first person in it).
fn who(thread: &Thread, config: &CommentsConfig) -> String {
    thread
        .comments
        .iter()
        .find(|c| is_person(c, config))
        .map_or_else(|| "someone".to_owned(), |c| c.author.clone())
}

/// Where the comment is, in plain words: the file and line for a comment on
/// code, or "the PR" for one on the PR as a whole.
fn where_on(thread: &Thread) -> String {
    match &thread.kind {
        ThreadKind::Inline { path, line, .. } => {
            line.map_or_else(|| path.clone(), |l| format!("{path}:{l}"))
        }
        ThreadKind::Conversation | ThreadKind::Review { .. } => "the PR".to_owned(),
    }
}

/// A sub-tab's label: the person, plus the file for a comment on code.
fn short_place(thread: &Thread, config: &CommentsConfig) -> String {
    match &thread.kind {
        ThreadKind::Inline { path, line, .. } => {
            let file = path.rsplit('/').next().unwrap_or(path);
            let at = line.map_or_else(|| file.to_owned(), |l| format!("{file}:{l}"));
            format!("{} · {at}", who(thread, config))
        }
        ThreadKind::Conversation | ThreadKind::Review { .. } => who(thread, config),
    }
}

/// The first line of what the person wrote, cut short — the footer's
/// preview of the next comment.
fn preview(thread: &Thread, config: &CommentsConfig) -> String {
    let text = thread
        .comments
        .iter()
        .find(|c| is_person(c, config))
        .and_then(|c| c.body.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("")
        .trim()
        .trim_start_matches('>')
        .trim();
    if text.chars().count() > 60 {
        format!("{}…", text.chars().take(59).collect::<String>())
    } else {
        text.to_owned()
    }
}

/// The strip of sub-tabs: one per thread, the shown one highlighted, `•`
/// on those with something unread.
fn tab_strip(
    threads: &[Thread],
    selected: usize,
    read: &comments::ReadMarks,
    config: &CommentsConfig,
) -> DocLine {
    let mut spans = vec![Span::styled("comments  ", dim())];
    for (i, t) in threads.iter().enumerate() {
        let unread = if comments::is_read(t, read) {
            ""
        } else {
            "•"
        };
        let label = format!(" {} {unread}{} ", i + 1, short_place(t, config));
        spans.push(if i == selected {
            Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else if unread.is_empty() {
            Span::styled(label, dim())
        } else {
            Span::styled(label, Style::default().fg(Color::Yellow))
        });
        spans.push(Span::raw(" "));
    }
    DocLine::prose(Line::from(spans))
}

/// `Comment 2 of 3 · on helm/hpa.yaml:12` with the keys that move.
fn heading(thread: &Thread, at: usize, total: usize) -> Vec<DocLine> {
    let cyan = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let mut first = vec![
        Span::styled(format!("Comment {} of {total}", at + 1), cyan),
        Span::styled(
            format!("  ·  on {}", where_on(thread)),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ];
    if let ThreadKind::Inline { outdated: true, .. } = &thread.kind {
        first.push(Span::styled("  · the code has changed since", dim()));
    }
    let keys = if total > 1 {
        "‹ N previous  ·  n next ›    J / K scroll the review  ·  u mark unread"
    } else {
        "the only comment    J / K scroll the review  ·  u mark unread"
    };
    vec![
        DocLine::prose(Line::from(first)),
        DocLine::prose(Line::styled(keys, dim())),
    ]
}

/// Builds the Comments tab for `recap` (files in `dir`): the sub-tab strip
/// and the `selected` thread's comments from people.
pub(super) fn document(
    recap: &Recap,
    dir: &std::path::Path,
    read: &comments::ReadMarks,
    config: &CommentsConfig,
    width: u16,
    selected: usize,
) -> ThreadDoc {
    let threads = comments::load(dir).threads;
    let author = recap.manifest.author.clone().unwrap_or_default();
    let ids = threads.iter().map(|t| t.id.clone()).collect();
    let mut lines = Vec::new();
    let Some(thread) = threads.get(selected.min(threads.len().saturating_sub(1))) else {
        lines.push(DocLine::prose(Line::styled(
            "No open comments from people on this PR.",
            dim(),
        )));
        lines.push(DocLine::prose(Line::styled(
            "Bots, ignored authors, automated replies and resolved threads are left out. \
             Synced every few minutes while recap is open; r syncs now.",
            dim(),
        )));
        return ThreadDoc { lines, ids };
    };
    let at = selected.min(threads.len() - 1);
    lines.push(tab_strip(&threads, at, read, config));
    lines.push(DocLine::prose(Line::default()));
    lines.extend(heading(thread, at, threads.len()));
    let hidden = thread
        .comments
        .iter()
        .filter(|c| !is_person(c, config))
        .count();
    for c in thread.comments.iter().filter(|c| is_person(c, config)) {
        let name = if c.author == author {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };
        lines.push(DocLine::prose(Line::default()));
        lines.push(DocLine::prose(Line::from(vec![
            Span::styled("● ", name),
            Span::styled(c.author.clone(), name),
            Span::styled(format!("  ·  {}", when(&c.created)), dim()),
        ])));
        lines.extend(indented(&markdown::to_lines(&c.body), "│ ", dim(), width));
    }
    if hidden > 0 {
        lines.push(DocLine::prose(Line::default()));
        lines.push(DocLine::prose(Line::styled(
            format!(
                "({hidden} bot or automated comment{} in this thread not shown — o opens the PR)",
                if hidden == 1 { "" } else { "s" }
            ),
            dim(),
        )));
    }
    // Make it obvious there is more: the next comment, by name, with a
    // preview — or that this was the last one.
    lines.push(DocLine::prose(Line::default()));
    let bar = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    if let Some(next) = threads.get(at + 1) {
        lines.push(DocLine::prose(Line::from(vec![
            Span::styled(
                format!(" next: comment {} of {} ", at + 2, threads.len()),
                bar,
            ),
            Span::styled(
                format!(
                    "  {}: “{}”  — press n",
                    who(next, config),
                    preview(next, config)
                ),
                Style::default().fg(Color::Yellow),
            ),
        ])));
    } else if threads.len() > 1 {
        lines.push(DocLine::prose(Line::styled(
            format!(
                "── that's all {} comments — n goes back to the first ──",
                threads.len()
            ),
            dim(),
        )));
    }
    ThreadDoc { lines, ids }
}

/// The right-hand pane: the agent's whole review of the shown thread, or
/// why there is none. Laid out for `width`.
pub(super) fn review_pane(app: &App, width: u16) -> (String, Vec<Line<'static>>) {
    let plain = |text: String| vec![DocLine::prose(Line::styled(text, dim()))];
    let (Some(i), Some(dir)) = (app.current_thread(), app.selected_dir()) else {
        return (" agent review ".to_owned(), Vec::new());
    };
    let threads = comments::load(&dir).threads;
    let Some(thread) = app
        .thread_ids
        .get(i)
        .and_then(|id| threads.iter().find(|t| &t.id == id))
    else {
        return (" agent review ".to_owned(), Vec::new());
    };
    let author = app
        .recap
        .as_ref()
        .and_then(|r| r.manifest.author.clone())
        .unwrap_or_default();
    let now = crate::store::now_unix();
    let review = comments::load_review(&dir, &thread.id);
    let mut doc = Vec::new();
    if let Some(r) = &review {
        if !comments::is_reviewed(&dir, thread, &author) {
            doc.push(DocLine::prose(Line::styled(
                "New comments since this review — an updated one is on its way.",
                Style::default().fg(Color::Yellow),
            )));
            doc.push(DocLine::prose(Line::default()));
        }
        doc.extend(markdown::to_lines(&r.body));
    } else {
        let waiting_on = thread
            .last_from_others(&author)
            .map_or_else(|| "the reviewer".to_owned(), |c| c.author.clone());
        doc.extend(plain(match comments::need(&dir, thread, &author, now) {
            Need::Review => {
                "Waiting on an agent review (`beagle recap comments pending` lists it).".to_owned()
            }
            Need::Settling => {
                "Recent comment: reviews wait 10 minutes for the reviewer to finish, then run."
                    .to_owned()
            }
            Need::TheirTurn => {
                format!(
                    "You replied last — it is {waiting_on}'s turn. Nothing for the agent to do."
                )
            }
            Need::Nothing | Need::Reviewed => "Nothing here needs a review.".to_owned(),
        }));
    }
    (
        format!(
            " agent review · comment {} of {} · tab to focus, j/k scroll ",
            i + 1,
            app.thread_ids.len()
        ),
        crate::wrap::layout(&doc, width),
    )
}
