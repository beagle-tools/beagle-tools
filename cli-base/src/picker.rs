//! The interactive version picker behind `install` / `update` without
//! `--version`: newest first, installed versions marked `●`, the active one
//! `current`; typing filters, Enter picks, Esc aborts.
//!
//! [`Picker`] is a pure state machine (tested without a terminal); only
//! [`pick_version`] touches the terminal, and it always restores it.

use std::io::{self, Write as _};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};

use crate::error::{Error, Result};
use crate::fuzzy;
use crate::version::Version;

/// One selectable version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// The release.
    pub version: Version,
    /// On disk already.
    pub installed: bool,
    /// The active version.
    pub current: bool,
}

/// A key press, decoupled from crossterm so the state machine is testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable character (appended to the filter).
    Char(char),
    /// Delete the last filter character.
    Backspace,
    /// Move the selection up.
    Up,
    /// Move the selection down.
    Down,
    /// Pick the selection.
    Enter,
    /// Abort (Esc or Ctrl-C).
    Esc,
}

/// What a key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Keep going.
    Continue,
    /// The user picked this version.
    Pick(Version),
    /// The user aborted.
    Abort,
}

/// The picker's state.
#[derive(Debug, Clone)]
pub struct Picker {
    items: Vec<Item>,
    query: String,
    selected: usize,
}

impl Picker {
    /// A picker over `items`, which must be newest first.
    pub fn new(items: Vec<Item>) -> Self {
        Self {
            items,
            query: String::new(),
            selected: 0,
        }
    }

    /// The current filter text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The items matching the filter, best match first (newest first while
    /// the filter is empty).
    pub fn visible(&self) -> Vec<Item> {
        let mut scored: Vec<(i32, usize, Item)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                fuzzy::score(&self.query, &item.version.to_string()).map(|s| (s, i, *item))
            })
            .collect();
        scored.sort_by_key(|(score, i, _)| (std::cmp::Reverse(*score), *i));
        scored.into_iter().map(|(_, _, item)| item).collect()
    }

    /// Applies one key press.
    pub fn handle(&mut self, key: Key) -> Step {
        let count = self.visible().len();
        match key {
            Key::Char(c) => {
                self.query.push(c);
                self.selected = 0;
            }
            Key::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            Key::Enter => {
                if let Some(item) = self.visible().get(self.selected) {
                    return Step::Pick(item.version);
                }
            }
            Key::Esc => return Step::Abort,
        }
        Step::Continue
    }

    /// The screen as text lines, at most `height` of them.
    pub fn render(&self, title: &str, height: usize) -> Vec<String> {
        let mut lines = vec![
            format!("{title} — type to filter · ↑/↓ move · enter install · esc cancel"),
            format!("> {}", self.query),
        ];
        let visible = self.visible();
        if visible.is_empty() {
            lines.push("  (no matching versions)".to_owned());
        }
        let room = height.saturating_sub(lines.len()).max(1);
        let start = self.selected.saturating_sub(room - 1);
        let latest = self.items.first().map(|i| i.version);
        for (i, item) in visible.iter().enumerate().skip(start).take(room) {
            let cursor = if i == self.selected { '▸' } else { ' ' };
            let dot = if item.installed { '●' } else { ' ' };
            let mut line = format!("{cursor} {dot} {}", item.version);
            if Some(item.version) == latest {
                line.push_str("  latest");
            }
            if item.current {
                line.push_str("  current");
            }
            lines.push(line);
        }
        lines
    }
}

/// Builds the picker items for `versions` (newest first).
pub fn items(versions: &[Version], installed: &[Version], current: Option<Version>) -> Vec<Item> {
    versions
        .iter()
        .map(|v| Item {
            version: *v,
            installed: installed.contains(v),
            current: current == Some(*v),
        })
        .collect()
}

/// Runs the picker on the terminal (drawn on stderr). `Ok(None)` on abort.
pub fn pick_version(title: &str, items: Vec<Item>) -> Result<Option<Version>> {
    if items.is_empty() {
        return Ok(None);
    }
    let mut picker = Picker::new(items);
    let _guard = TerminalGuard::enter().map_err(Error::Terminal)?;
    loop {
        draw(&picker, title).map_err(Error::Terminal)?;
        let Event::Key(key) = event::read().map_err(Error::Terminal)? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let key = match key.code {
            KeyCode::Char('c' | 'd') if ctrl => Key::Esc,
            KeyCode::Char('p') if ctrl => Key::Up,
            KeyCode::Char('n') if ctrl => Key::Down,
            KeyCode::Char(c) if !ctrl => Key::Char(c),
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            _ => continue,
        };
        match picker.handle(key) {
            Step::Continue => {}
            Step::Pick(version) => return Ok(Some(version)),
            Step::Abort => return Ok(None),
        }
    }
}

fn draw(picker: &Picker, title: &str) -> io::Result<()> {
    let height = terminal::size().map_or(24, |(_, h)| usize::from(h));
    let mut err = io::stderr();
    queue!(
        err,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All)
    )?;
    for line in picker.render(title, height) {
        write!(err, "{line}\r\n")?;
    }
    err.flush()
}

/// Raw mode + alternate screen for the picker's lifetime; restored on drop,
/// so every exit path (errors included) gives the user their shell back.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let guard = Self;
        execute!(io::stderr(), terminal::EnterAlternateScreen, cursor::Hide)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
#[path = "tests/picker.rs"]
mod tests;
