//! The `?` help sheet: every keybinding, fuzzy-filterable (`f`), drawn by
//! the shared `beagle_tui::chrome::draw_help` like every beagle TUI's.

use beagle_tui::chrome::{self, HelpFilter};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::ui::App;

/// Every keybinding row: `(keys, action)`.
pub(crate) const HELP_ROWS: &[(&str, &str)] = &[
    (
        "j / k, ↓ / ↑",
        "select incident · move the content line cursor",
    ),
    ("enter / l", "focus the content pane"),
    ("b / esc", "back to the incident list"),
    ("tab / [ / ], ← / →", "cycle tabs"),
    ("1–9", "jump to a tab"),
    (
        "f",
        "filter: i/x/r/a/v/f status · c/h/m/l sev · p has-PR · s security · / text",
    ),
    ("F", "follow: reloads stick to the bottom (tail -f)"),
    ("s", "hide / show the sidebar (full-width content)"),
    beagle_tui::pane_resize::HELP,
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
    ("i", "copy the incident id to clipboard (/beagle-review)"),
    (
        "x",
        "export RCA to exports/<id>.md (frontmatter + all tabs)",
    ),
    ("E", "open this tab's file in your editor"),
    ("T", "toolbox: toolbox.md + systems/ context"),
    ("o", "links: open attached PRs / URLs on this tab"),
    ("R", "related incidents (shared systems/tags); enter jumps"),
    ("V", "sign off final-review as verified \u{2192} finished"),
    ("t", "set status: pick the RCA's lifecycle stage"),
    ("u", "attach a PR to this incident (paste the URL)"),
    ("I", "attach a ticket (Linear / GitHub issue URL or id)"),
    ("#", "edit tags: add / remove, incl. skip-final-review"),
    ("!", "view load errors / warnings (broken workspaces)"),
    ("d", "delete the selected incident (y/n confirm popup)"),
    ("S", "settings: view + edit the config file"),
    ("m", "agents: switch to the agents monitor (m / esc back)"),
    ("n / p", "next / previous diagram"),
    ("h / l", "pan wide content (diagrams) sideways"),
    ("space / pgdn / pgup", "page through content"),
    ("ctrl-d / ctrl-u", "half a page down / up (content)"),
    ("g / G", "top / bottom"),
    ("r", "reload from disk"),
    ("q / Q / ctrl-c", "quit"),
];

pub(in crate::ui::view) fn draw_help(frame: &mut Frame, app: &App, _area: Rect) {
    let filter = match app.help_filter() {
        Some(q) => HelpFilter::Typing(q),
        None => HelpFilter::Available,
    };
    chrome::draw_help(frame, HELP_ROWS, filter);
}
