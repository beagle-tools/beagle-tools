//! The `?` help sheet: every keybinding, fuzzy-filterable, sized to the
//! widest row.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem};
use ratatui::Frame;

use crate::ui::App;

use super::super::style::{center, truncate};

/// Every keybinding row: `(keys, action)`. Kept as a const so the pane can
/// size itself to the widest row and filter without re-listing.
const HELP_ROWS: &[(&str, &str)] = &[
    (
        "j / k, ↓ / ↑",
        "select incident · move the content line cursor",
    ),
    ("enter / l", "focus the content pane"),
    ("b / esc", "back to the incident list"),
    ("tab / [ / ], ← / →", "cycle tabs"),
    ("1–8", "jump to a tab"),
    (
        "f",
        "filter: i/r/a/v/f status · c/h/m/l sev · p has-PR · s security · / text",
    ),
    ("F", "follow: reloads stick to the bottom (tail -f)"),
    ("s", "hide / show the sidebar (full-width content)"),
    ("a", "show / hide archived incidents (dimmed)"),
    ("/", "search the incident: every tab, live highlight"),
    ("n / N", "next / previous search match"),
    (
        "\\",
        "find everywhere: fuzzy across all incidents; enter jumps",
    ),
    ("c / C", "copy this tab / whole RCA to clipboard"),
    (
        "y",
        "yank the cursor line / selection to clipboard (content)",
    ),
    (
        "v",
        "visual-line select: extend with j/k · y yanks · v/esc cancel",
    ),
    ("Y", "yank the incident id to clipboard (/beagle-review)"),
    (
        "e",
        "export RCA to exports/<id>.md (frontmatter + all tabs)",
    ),
    ("E", "open this tab's file in your editor"),
    ("T", "toolbox: toolbox.md + systems/ context"),
    ("o", "links: open attached PRs / URLs on this tab"),
    ("R", "related incidents (shared systems/tags); enter jumps"),
    ("V", "sign off final-review as verified \u{2192} finished"),
    ("t", "set status: pick the RCA's lifecycle stage"),
    ("P", "attach a PR to this incident (paste the URL)"),
    ("I", "attach a ticket (Linear / GitHub issue URL or id)"),
    ("#", "edit tags: add / remove, incl. skip-final-review"),
    ("!", "view load errors / warnings (broken workspaces)"),
    ("D", "delete the selected incident (y/n confirm popup)"),
    ("S", "settings: view + edit the config file"),
    ("A", "agents: switch to the agents monitor (A / esc back)"),
    ("n / p", "next / previous diagram"),
    ("h / l, ← / →", "pan diagrams horizontally"),
    ("space / pgdn / pgup", "page through content"),
    ("g / G", "top / bottom"),
    ("r", "reload from disk"),
    ("Q / ctrl-c", "quit"),
];

/// Column width reserved for the keys before the action text.
const HELP_KEY_COL: usize = 20;

/// Renders one help row (`  <keys padded><action>`) as styled spans. The
/// keys are yellow; when `query` is filtering, the characters it fuzzy-
/// matched are highlighted (same treatment as the `\` finder) so it's
/// clear why the row matched. Runs of the same style coalesce into spans.
fn help_row_spans(
    keys: &str,
    action: &str,
    key_col: usize,
    query: Option<&str>,
) -> Vec<Span<'static>> {
    let text = format!("  {keys:<key_col$}{action}");
    let keys_end = 2 + keys.chars().count();
    let matched: Vec<usize> = match query {
        Some(q) if !q.is_empty() => crate::fuzzy::indices(q, &text)
            .map(|(_, positions)| positions)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // 0 = matched (highlight), 1 = keys (yellow), 2 = normal.
    let class = |i: usize| -> u8 {
        if matched.contains(&i) {
            0
        } else if (2..keys_end).contains(&i) {
            1
        } else {
            2
        }
    };
    let style_of = |c: u8| match c {
        0 => Style::default()
            .fg(Color::Rgb(214, 160, 30))
            .add_modifier(Modifier::BOLD),
        1 => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_class = 2u8;
    let mut started = false;
    for (i, ch) in text.chars().enumerate() {
        let c = class(i);
        if started && c != buf_class {
            spans.push(Span::styled(std::mem::take(&mut buf), style_of(buf_class)));
        }
        buf_class = c;
        started = true;
        buf.push(ch);
    }
    if started {
        spans.push(Span::styled(buf, style_of(buf_class)));
    }
    spans
}

pub(in crate::ui::view) fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let query = app.help_filter();

    // Width fits the widest full row, so nothing is truncated on a normal
    // terminal (clamped to what's on screen).
    let needed = HELP_ROWS
        .iter()
        .map(|(k, a)| 4 + HELP_KEY_COL.max(k.chars().count()) + a.chars().count())
        .max()
        .unwrap_or(40);
    let width = u16::try_from(needed)
        .unwrap_or(u16::MAX)
        .clamp(40, area.width.saturating_sub(4).max(40));
    // Height comes from the FULL row count, so filtering narrows the list
    // without ever resizing the box.
    let height = u16::try_from(HELP_ROWS.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let popup = center(area, width, height);
    let action_room = usize::from(width).saturating_sub(4 + HELP_KEY_COL);
    let key_col = HELP_KEY_COL;

    let mut items: Vec<ListItem<'_>> = HELP_ROWS
        .iter()
        .filter(|(keys, action)| match query {
            Some(q) if !q.is_empty() => {
                crate::fuzzy::score(q, &format!("{keys} {action}")).is_some()
            }
            _ => true,
        })
        .map(|(keys, action)| {
            ListItem::new(Line::from(help_row_spans(
                keys,
                &truncate(action, action_room),
                key_col,
                query,
            )))
        })
        .collect();
    if items.is_empty() {
        items.push(ListItem::new(Line::styled(
            "  (no matching keys)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let bottom = match query {
        Some(q) => format!(" filter: {q}▌ · esc clears "),
        None => " f filter · any key closes ".to_owned(),
    };
    let block = Block::default()
        .title(" keys ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(bottom).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, popup);
    frame.render_widget(List::new(items).block(block), popup);
}
