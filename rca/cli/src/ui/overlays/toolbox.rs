//! The `T` toolbox overlay: `toolbox.md` plus every `systems/*.md`,
//! rendered once on open and scrolled while it owns the keys.

use crossterm::event::KeyCode;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};

use crate::markdown;
use crate::ui::App;

impl App {
    /// Builds and opens the toolbox overlay: the root `toolbox.md` followed
    /// by the `systems/*.md` docs matching the selected workspace's systems
    /// (all of them when nothing is selected or the workspace lists none).
    ///
    /// A missing toolbox is scaffolded on the spot (`beagle init`) —
    /// pressing `T` means the user wants one, and `init_context` never
    /// overwrites existing files, so this is always safe.
    pub(crate) fn open_toolbox(&mut self) {
        let dim = Style::default().fg(Color::DarkGray);
        let mut lines: Vec<Line<'static>> = Vec::new();

        if matches!(self.store.read_toolbox(), Ok(None)) {
            match self.store.init_context() {
                Ok(created) if !created.is_empty() => {
                    lines.push(Line::styled(
                        "Scaffolded toolbox.md + systems/ (what `beagle init` does) — \
                         fill them in for your stack; agents read them before every \
                         investigation.",
                        Style::default().fg(Color::LightGreen),
                    ));
                    lines.push(Line::from(""));
                }
                Ok(_) => {}
                Err(e) => lines.push(Line::styled(
                    format!("could not scaffold the toolbox: {e}"),
                    Style::default().fg(Color::Red),
                )),
            }
        }

        match self.store.read_toolbox() {
            Ok(Some(content)) => lines.extend(markdown::to_text(&content).lines),
            Ok(None) => lines.push(Line::styled(
                "No toolbox.md yet — run `beagle init` to scaffold the \
                 investigation context (toolbox.md + systems/).",
                dim,
            )),
            Err(e) => lines.push(Line::styled(
                format!("toolbox load error: {e}"),
                Style::default().fg(Color::Red),
            )),
        }

        let docs = match self.store.list_system_docs() {
            Ok(docs) => docs,
            Err(e) => {
                lines.push(Line::styled(
                    format!("systems/ load error: {e}"),
                    Style::default().fg(Color::Red),
                ));
                Vec::new()
            }
        };
        let wanted: Vec<String> = self
            .selected_rca()
            .map(|rca| rca.meta.systems.clone())
            .unwrap_or_default();
        let shown = docs
            .iter()
            .filter(|doc| wanted.is_empty() || wanted.contains(&doc.name));
        for doc in shown {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!("─── systems/{}.md ───", doc.name),
                dim,
            ));
            lines.push(Line::from(""));
            match self.store.read_system_doc(doc) {
                Ok(Some(content)) => lines.extend(markdown::to_text(&content).lines),
                Ok(None) => {}
                Err(e) => lines.push(Line::styled(
                    format!("load error: {e}"),
                    Style::default().fg(Color::Red),
                )),
            }
        }
        if !wanted.is_empty() && !docs.iter().any(|d| wanted.contains(&d.name)) {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!(
                    "(no systems/ docs for: {} — add systems/<name>.md to give \
                     agents per-system context)",
                    wanted.join(", ")
                ),
                dim,
            ));
        }

        self.toolbox = Some(Text::from(lines));
        self.toolbox_scroll = 0;
    }

    /// Keystrokes while the toolbox overlay is open: scroll or close.
    pub(crate) fn handle_toolbox_key(&mut self, code: KeyCode) {
        let (content_lines, height) = self.toolbox_viewport;
        let max = content_lines.saturating_sub(height);
        let page = height.saturating_sub(1).max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'T') => self.toolbox = None,
            KeyCode::Char('j') | KeyCode::Down => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_add(1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::PageDown => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_add(page).min(max);
            }
            KeyCode::PageUp => self.toolbox_scroll = self.toolbox_scroll.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.toolbox_scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.toolbox_scroll = max,
            _ => {}
        }
    }

    /// The toolbox overlay content, when open.
    pub(crate) fn toolbox(&self) -> Option<&Text<'static>> {
        self.toolbox.as_ref()
    }

    pub(crate) fn toolbox_scroll(&self) -> u16 {
        self.toolbox_scroll
    }
}
