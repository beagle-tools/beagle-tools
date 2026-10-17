//! Drawing: a pure projection of [`App`] onto the frame.
//!
//! The only mutation allowed here is feeding viewport geometry back into the
//! app (for scroll clamping) — no state transitions, no I/O, and no markdown
//! parsing (content is pre-rendered by the app when it changes, not per
//! frame).
//!
//! Submodules: `header` (title, meta, tab bar, banner), `sidebar` (the
//! incident list), `content` (the tab body), `status_bar`, `agents` (the
//! agents screen), `popups` (every modal overlay), and `style` (badges,
//! glyphs, layout helpers).

mod agents;
mod content;
mod header;
mod popups;
mod sidebar;
mod status_bar;
mod style;

use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{App, Screen};

use agents::draw_agents;
use content::{draw_content, draw_welcome};
use header::{banner_fits, draw_banner, flow_tabs, header_paragraph, BANNER_COLS};
use popups::{
    draw_attach_prompt, draw_confirm_delete, draw_errors, draw_finder, draw_help, draw_links,
    draw_related, draw_settings, draw_status_picker, draw_tags_editor, draw_toolbox,
};
use sidebar::draw_sidebar;
use status_bar::draw_status_bar;
use style::{horizontal, inset, vertical, SIDEBAR_WIDTH};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let [main, status_bar] = vertical(frame.area(), &[Constraint::Min(0), Constraint::Length(1)]);
    match app.screen() {
        Screen::Agents => {
            app.mouse.sidebar = Rect::default();
            draw_agents(frame, app, main);
        }
        Screen::Rcas => draw_rcas(frame, app, main),
    }
    draw_status_bar(frame, app, status_bar);

    if app.toolbox().is_some() {
        draw_toolbox(frame, app, frame.area());
    }
    if app.errors_visible() {
        draw_errors(frame, app, frame.area());
    }
    draw_links(frame, app, frame.area());
    draw_related(frame, app, frame.area());
    draw_settings(frame, app, frame.area());
    draw_finder(frame, app, frame.area());
    draw_status_picker(frame, app, frame.area());
    draw_tags_editor(frame, app, frame.area());
    draw_attach_prompt(frame, app, frame.area());
    draw_confirm_delete(frame, app, frame.area());
    if app.help_visible() {
        draw_help(frame, app, frame.area());
    }
}

/// Draws the RCA browser: the workspace sidebar plus the tabbed content pane.
fn draw_rcas(frame: &mut Frame, app: &mut App, area: Rect) {
    let sidebar_width = if app.sidebar_collapsed() {
        0
    } else {
        SIDEBAR_WIDTH
    };
    let [sidebar, content] = horizontal(
        area,
        &[Constraint::Length(sidebar_width), Constraint::Min(0)],
    );

    app.mouse.sidebar = if app.sidebar_collapsed() {
        Rect::default()
    } else {
        sidebar
    };
    if !app.sidebar_collapsed() {
        draw_sidebar(frame, app, sidebar);
    }
    draw_workspace(frame, app, content);
}

fn draw_workspace(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(rca) = app.selected_rca().cloned() else {
        app.mouse.tabs.clear();
        app.mouse.content = area;
        draw_welcome(frame, area);
        return;
    };

    // The banner sits beside the header/tab rows at the top right, so it
    // never pushes content down by more than the difference between its
    // height and the rows the header already uses. Hidden when it would
    // squeeze the header below MIN_HEADER_COLS.
    let banner_cols = if banner_fits(area) { BANNER_COLS } else { 0 };
    let head_width = area.width.saturating_sub(banner_cols);

    // Header and tab bar heights are computed from the actual width so
    // nothing is ever cut off on narrow terminals — both flow onto extra
    // lines instead of truncating.
    let prs: Vec<(String, Option<crate::prs::PrState>)> = rca
        .meta
        .prs
        .iter()
        .map(|url| (url.clone(), app.pr_state(url)))
        .collect();
    let activity = app
        .last_activity(&rca.id)
        .and_then(|mtime| mtime.elapsed().ok());
    let header = header_paragraph(
        &rca,
        app.tick(),
        &prs,
        activity,
        app.checklist_progress(&rca.id),
    );
    let header_width = head_width.saturating_sub(1).max(1); // inset by 1 below
    let header_height = u16::try_from(header.line_count(header_width))
        .unwrap_or(2)
        .min(6);

    let tabs = app.visible_tabs();
    let unread: Vec<bool> = tabs
        .iter()
        .map(|tab| app.is_unread(&rca.id, *tab))
        .collect();
    let (tab_lines, tab_hits) = flow_tabs(app.tab(), head_width, &tabs, &unread);
    let tab_height = u16::try_from(tab_lines.len()).unwrap_or(1);

    let banner_height = if banner_cols > 0 {
        crate::banner::HEIGHT
    } else {
        0
    };
    let top_height = (header_height + tab_height).max(banner_height);
    let [top, body] = vertical(area, &[Constraint::Length(top_height), Constraint::Min(0)]);
    let [head_col, banner_col] =
        horizontal(top, &[Constraint::Min(0), Constraint::Length(banner_cols)]);
    let [header_area, tab_bar] = vertical(
        head_col,
        &[Constraint::Length(header_height), Constraint::Min(0)],
    );

    frame.render_widget(header, inset(header_area, 1));
    frame.render_widget(Paragraph::new(tab_lines), tab_bar);
    if banner_cols > 0 {
        draw_banner(frame, banner_col);
    }

    // Label positions → clickable screen rects for the mouse handler.
    app.mouse.tabs = tab_hits
        .iter()
        .filter(|hit| hit.line < tab_bar.height)
        .map(|hit| {
            (
                hit.tab,
                Rect {
                    x: tab_bar.x + hit.x,
                    y: tab_bar.y + hit.line,
                    width: hit.width.min(tab_bar.width.saturating_sub(hit.x)),
                    height: 1,
                },
            )
        })
        .collect();
    app.mouse.content = body;

    draw_content(frame, app, body);
}

#[cfg(test)]
#[path = "tests/view.rs"]
mod tests;
