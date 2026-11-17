//! The interactive release picker behind `… version list`: a full-screen
//! list of versions; enter returns the chosen one.

use beagle_utils::update::{Channel, Version};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::DefaultTerminal;

/// Runs the picker over `versions` (newest first): j/k or arrows move,
/// enter returns the chosen version, q/esc/ctrl-c returns `None`. Sets up
/// and restores the terminal itself; `name` titles the list.
///
/// # Errors
/// Terminal draw or input failures.
pub fn pick_version(
    name: &str,
    channel: &Channel,
    versions: &[Version],
    current: Version,
) -> std::io::Result<Option<Version>> {
    if versions.is_empty() {
        return Ok(None);
    }
    let mut terminal = ratatui::init();
    let picked = run(&mut terminal, name, channel, versions, current);
    ratatui::restore();
    picked
}

fn run(
    terminal: &mut DefaultTerminal,
    name: &str,
    channel: &Channel,
    versions: &[Version],
    current: Version,
) -> std::io::Result<Option<Version>> {
    let title = format!(" {name} versions — enter install · j/k move · q cancel ");
    let items: Vec<ListItem<'static>> = versions
        .iter()
        .enumerate()
        .map(|(i, v)| ListItem::new(release_line(&channel.tag(*v), *v == current, i == 0)))
        .collect();
    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| {
            let block = Block::default()
                .title(title.as_str())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow));
            let list = List::new(items.clone()).block(block).highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 44, 60))
                    .add_modifier(Modifier::BOLD),
            );
            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(list, frame.area(), &mut state);
        })?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(None);
        }
        let last = versions.len() - 1;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => selected = (selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => selected = 0,
            KeyCode::Char('G') | KeyCode::End => selected = last,
            KeyCode::Enter => return Ok(versions.get(selected).copied()),
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            _ => {}
        }
    }
}

/// One picker row: the tag plus `latest` / `current` markers.
fn release_line(tag: &str, is_current: bool, is_latest: bool) -> Line<'static> {
    let mut spans = vec![Span::raw(format!(" {tag:<20}"))];
    if is_latest {
        spans.push(Span::styled(
            " latest",
            Style::default().fg(Color::LightGreen),
        ));
    }
    if is_current {
        spans.push(Span::styled(
            " ← current",
            Style::default().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
}

/// The plain listing `… version list` prints when stdout is not a
/// terminal: one tag per line with `latest` / `current` markers.
#[must_use]
pub fn plain_listing(channel: &Channel, versions: &[Version], current: Version) -> String {
    let mut out = String::new();
    for (i, v) in versions.iter().enumerate() {
        out.push_str(&channel.tag(*v));
        if i == 0 {
            out.push_str("  latest");
        }
        if *v == current {
            out.push_str("  current");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)] // panicking is the correct failure mode in tests
mod tests {
    use super::*;

    const CHANNEL: Channel = Channel {
        repo: "acme/widgets",
        tag_prefix: "widgets-v",
        asset_prefix: "widgets",
        binary: "widgets",
        companions: &[],
    };

    #[test]
    fn plain_listing_marks_latest_and_current() {
        let versions: Vec<Version> = ["0.3.0", "0.2.0", "0.1.0"]
            .iter()
            .map(|s| s.parse().expect("version"))
            .collect();
        let current = "0.2.0".parse().expect("version");
        assert_eq!(
            plain_listing(&CHANNEL, &versions, current),
            "widgets-v0.3.0  latest\nwidgets-v0.2.0  current\nwidgets-v0.1.0\n"
        );
    }

    #[test]
    fn rows_carry_both_markers_for_a_current_latest() {
        let line = release_line("widgets-v1.0.0", true, true);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("latest") && text.contains("current"),
            "{text}"
        );
    }
}
