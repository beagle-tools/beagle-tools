//! The incident sidebar: one two-line row per workspace (badges, title,
//! checklist and PR progress) plus the broken-workspace tail.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use crate::model::{RcaSummary, Status};

use crate::ui::{App, Focus};

use super::style::{pane_block, severity_badge, status_symbol, truncate, SIDEBAR_WIDTH};

pub(super) fn draw_sidebar(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus() == Focus::List;
    let title = if app.has_active_filter() {
        use std::fmt::Write as _;
        let mut title = format!(" Incidents ({}/{})", app.visible_len(), app.rcas().len());
        let facets = app.facet_label();
        if !facets.is_empty() {
            let _ = write!(title, " {facets}");
        }
        if !app.filter().is_empty() {
            let _ = write!(title, " /{}", app.filter());
        }
        title.push(' ');
        title
    } else {
        let archived = app.archived_count();
        let active = app.rcas().len() - archived;
        if app.show_archived() && archived > 0 {
            format!(" Incidents ({active} · {archived} archived) ")
        } else {
            format!(" Incidents ({active}) ")
        }
    };
    let block = pane_block(title, focused);

    let tick = app.tick();
    let mut items: Vec<ListItem<'_>> = app
        .visible_rcas()
        .enumerate()
        .map(|(row, rca)| {
            ListItem::new(rca_list_item(
                rca,
                tick,
                app.has_unread(&rca.id),
                row == app.selected_index(),
                app.checklist_progress(&rca.id),
                pr_summary(app, rca),
            ))
        })
        .collect();
    // Broken workspaces render at the bottom, unselectable but never
    // hidden: a directory that exists on disk must not silently vanish
    // from the incident list. One compact line each — the full reason
    // lives in the `!` errors overlay, not crammed into this narrow pane.
    if !app.broken().is_empty() {
        for broken in app.broken() {
            items.push(ListItem::new(broken_list_item(broken)));
        }
        items.push(ListItem::new(Line::from(Span::styled(
            "   ! or click to view",
            Style::default().fg(Color::DarkGray),
        ))));
    }
    // Selection styling is baked into the items (rca_list_item): a
    // highlight_style here would be patched over every span in the row and
    // wipe out the severity badge's own background — the selected row then
    // looked *less* highlighted than its neighbors.
    let list = List::new(items).block(block);

    let mut state = ListState::default();
    if app.visible_len() > 0 {
        state.select(Some(app.selected_index()));
    }
    frame.render_stateful_widget(list, area, &mut state);
    // Rendering scrolled the list to keep the selection visible; the mouse
    // handler needs the resulting first-visible row to map clicks.
    app.mouse.sidebar_offset = state.offset();
}

/// Background of the selected sidebar row.
pub(super) const SELECTED_BG: Color = Color::Rgb(40, 44, 60);

/// A compact PR indicator for the sidebar detail line, from the merge
/// states beagle already polls: `PR ✓` (all merged), `PR m/t` (some
/// merged), `PR ✗` (closed, none merged), `PR·n` (links known but no `gh`
/// state yet), or — for a `review`/`agent` incident with no PR attached —
/// a dim `no PR` so it's clear the review isn't tracking one. `None` for
/// other statuses without a PR, to keep those rows quiet.
pub(super) fn pr_summary(app: &App, rca: &RcaSummary) -> Option<(String, Color)> {
    use crate::prs::PrState;
    if rca.meta.prs.is_empty() {
        return matches!(rca.meta.status, Status::Review | Status::Agent)
            .then(|| ("no PR".to_owned(), Color::DarkGray));
    }
    let total = rca.meta.prs.len();
    let known: Vec<PrState> = rca
        .meta
        .prs
        .iter()
        .filter_map(|u| app.pr_state(u))
        .collect();
    if known.is_empty() {
        return Some((format!("PR·{total}"), Color::Gray));
    }
    let merged = known.iter().filter(|s| **s == PrState::Merged).count();
    if merged == total {
        return Some(("PR ✓".to_owned(), Color::Green));
    }
    if merged == 0 && known.iter().all(|s| *s == PrState::Closed) {
        return Some(("PR ✗".to_owned(), Color::Red));
    }
    Some((format!("PR {merged}/{total}"), Color::Yellow))
}

/// The two sidebar lines for one workspace. Returned as lines (not a
/// [`ListItem`]) so tests can inspect the span styles.
pub(super) fn rca_list_item(
    rca: &RcaSummary,
    tick: usize,
    has_unread: bool,
    selected: bool,
    progress: Option<(usize, usize)>,
    pr: Option<(String, Color)>,
) -> Vec<Line<'static>> {
    let (badge, mut badge_style) = severity_badge(rca.meta.severity);
    let (symbol, mut symbol_style) = status_symbol(rca.meta.status, tick);

    // The selection background is applied per-span so the badge keeps its
    // own colors; `base` styles everything that has no color identity.
    let mut base = if selected {
        Style::default()
            .bg(SELECTED_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    if rca.archived {
        // Archived rows are uniformly dim — history, not a live incident.
        // The badge drops its background so nothing on the row pops.
        base = base.fg(Color::DarkGray);
        badge_style = base;
        symbol_style = base;
    }
    let tinted = |style: Style| {
        if selected {
            style.bg(SELECTED_BG)
        } else {
            style
        }
    };

    let title_text = truncate(&rca.meta.title, SIDEBAR_WIDTH as usize - 10);
    let title = pad_line(
        vec![
            Span::styled(format!(" {badge} "), badge_style),
            Span::styled(" ", base),
            Span::styled(title_text, base),
        ],
        base,
    );
    let mut detail_spans = vec![
        Span::styled(format!("  {symbol} "), tinted(symbol_style)),
        Span::styled(rca.meta.status.to_string(), tinted(symbol_style)),
    ];
    if rca.archived {
        detail_spans.push(Span::styled(" · archived", tinted(base)));
    }
    // PR state comes right after the status — the thing you most want to
    // know about a review, and placed early so it survives truncation.
    if let Some((label, color)) = pr {
        detail_spans.push(Span::styled(
            format!("  {label}"),
            tinted(Style::default().fg(color)),
        ));
    }
    if let Some((checked, total)) = progress {
        // Checklist progress across all sections: green once complete.
        let style = if checked == total {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default().fg(Color::Gray)
        };
        detail_spans.push(Span::styled(
            format!("  ☑ {checked}/{total}"),
            tinted(style),
        ));
    }
    if has_unread {
        detail_spans.push(Span::styled(
            " ●",
            tinted(Style::default().fg(Color::LightYellow)),
        ));
    }
    vec![title, pad_line(detail_spans, base)]
}

/// One compact sidebar line for a workspace that failed to load: an
/// unmissable marker and the directory name, never hidden. The reason is
/// too long for this pane — it lives in the `!` errors overlay.
pub(super) fn broken_list_item(broken: &crate::store::BrokenWorkspace) -> Line<'static> {
    let width = usize::from(SIDEBAR_WIDTH).saturating_sub(2);
    Line::from(vec![
        Span::styled(
            " ⚠ ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", truncate(&broken.dir_name, width.saturating_sub(5))),
            Style::default().fg(Color::LightRed),
        ),
    ])
}

/// Pads a sidebar line with a filler span so a selection background covers
/// the full row width, not just the text.
pub(super) fn pad_line(spans: Vec<Span<'static>>, base: Style) -> Line<'static> {
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let mut spans = spans;
    let width = usize::from(SIDEBAR_WIDTH).saturating_sub(2); // inside the borders
    if used < width {
        spans.push(Span::styled(" ".repeat(width - used), base));
    }
    Line::from(spans)
}
