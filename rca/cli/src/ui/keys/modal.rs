//! Keys while something other than the RCA browser owns input: the agents
//! screen, any open modal overlay, and the `?` help sheet.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::App;

use super::Flow;

impl App {
    /// Keys while the agents screen owns the view: `j`/`k` select an agent,
    /// `x` starts/stops it, `enter` opens its latest session log, `e` edits the
    /// config (and reloads), `A`/`Esc` returns to the RCA browser, `?` opens
    /// help, and `Q` quits.
    pub(super) fn handle_agents_key(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('Q') => return Flow::Quit,
            KeyCode::Char('A') | KeyCode::Esc => self.toggle_screen(),
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_filter = None;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_agent_selection(true),
            KeyCode::Char('k') | KeyCode::Up => self.move_agent_selection(false),
            KeyCode::Char('x') => self.toggle_selected_agent(),
            KeyCode::Enter => {
                if let Some(path) = self.selected_agent_log() {
                    return Flow::Edit(path);
                }
            }
            KeyCode::Char('e') => match crate::agentd::config_path() {
                Some(path) => return Flow::EditAgentsConfig(path),
                None => self.status = Some("could not resolve the agents.toml path".to_owned()),
            },
            _ => {}
        }
        Flow::Continue
    }

    /// Routes the key to whichever modal owns it, returning whether one
    /// did. The delete confirmation is checked first: while it is open,
    /// nothing else may interpret a key — least of all `y`.
    pub(super) fn route_modal_key(&mut self, key: KeyEvent) -> bool {
        if self.attach_prompt.is_some() {
            self.handle_attach_prompt_key(key.code);
        } else if self.confirm_delete.is_some() {
            self.handle_confirm_delete_key(key.code);
        } else if self.status_picker.is_some() {
            self.handle_status_picker_key(key.code);
        } else if self.tags_editor.is_some() {
            self.handle_tags_editor_key(key.code);
        } else if self.show_errors {
            self.handle_errors_key(key.code);
        } else if self.finder.is_some() {
            self.handle_finder_key(key.code, key.modifiers.contains(KeyModifiers::CONTROL));
        } else if self.filter_input != crate::ui::FilterInput::Off {
            self.handle_search_key(key.code);
        } else if self.content_search.as_ref().is_some_and(|s| s.typing) {
            self.handle_content_search_key(key.code);
        } else if self.links.is_some() {
            self.handle_links_key(key.code);
        } else if self.related.is_some() {
            self.handle_related_key(key.code);
        } else if self.settings.is_some() {
            self.handle_settings_key(key.code);
        } else if self.toolbox.is_some() {
            self.handle_toolbox_key(key.code);
        } else if self.show_help {
            self.handle_help_key(key.code);
        } else {
            return false;
        }
        true
    }

    /// Keystrokes while the `?` help sheet is open. Not filtering: `f`
    /// starts a filter, any other key closes (as the sheet always has).
    /// Filtering: chars/backspace edit the query (rows narrow live), esc
    /// clears the filter — a second esc then closes.
    pub(super) fn handle_help_key(&mut self, code: KeyCode) {
        if self.help_filter.is_some() {
            match code {
                KeyCode::Esc => self.help_filter = None,
                KeyCode::Backspace => {
                    if let Some(q) = self.help_filter.as_mut() {
                        q.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(q) = self.help_filter.as_mut() {
                        q.push(c);
                    }
                }
                _ => {}
            }
        } else if code == KeyCode::Char('f') {
            self.help_filter = Some(String::new());
        } else {
            self.close_help();
        }
    }
}
