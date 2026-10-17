//! Key handling for the small editing overlays: the `t` status picker,
//! `#` tags editor, `P`/`I` attach prompt, and `D` delete confirmation.
//! Each writes through the store and refreshes the list on success.

use crossterm::event::KeyCode;

use crate::model::Status;
use crate::ui::App;

use super::{AttachKind, AttachPrompt, ConfirmDelete, StatusPicker, TagsEditor};

impl App {
    /// Opens the `t` status picker for the selected incident, with the
    /// current stage highlighted.
    pub(crate) fn open_status_picker(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to set".to_owned());
            return;
        };
        let current = rca.meta.status;
        self.status_picker = Some(StatusPicker {
            id: rca.id.clone(),
            title: rca.meta.title.clone(),
            current,
            selected: Status::ALL.iter().position(|&s| s == current).unwrap_or(0),
        });
    }

    /// Keystrokes while the status picker is open: move, apply, or close.
    /// Re-picking the current stage closes without a write — no spurious
    /// `updated` stamp.
    pub(crate) fn handle_status_picker_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 't') => self.status_picker = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(picker) = self.status_picker.as_mut() {
                    picker.selected = (picker.selected + 1).min(Status::ALL.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(picker) = self.status_picker.as_mut() {
                    picker.selected = picker.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let Some(picker) = self.status_picker.take() else {
                    return;
                };
                let status = Status::ALL[picker.selected.min(Status::ALL.len() - 1)];
                if status == picker.current {
                    return; // already there — close quietly
                }
                match self.store.set_status(&picker.id, status) {
                    Ok(_) => {
                        let _ = self.reload();
                        self.status = Some(format!("{} → {status}", picker.id));
                    }
                    Err(e) => self.status = Some(format!("status change failed: {e}")),
                }
            }
            _ => {}
        }
    }

    /// The status picker, when open.
    pub(crate) fn status_picker(&self) -> Option<&StatusPicker> {
        self.status_picker.as_ref()
    }

    /// Opens the `#` tags editor for the selected incident.
    pub(crate) fn open_tags_editor(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to tag".to_owned());
            return;
        };
        self.tags_editor = Some(TagsEditor {
            id: rca.id.clone(),
            title: rca.meta.title.clone(),
            tags: rca.meta.tags.clone(),
            selected: 0,
            typing: None,
        });
    }

    /// Keystrokes while the tags editor is open. Navigation mode: `j`/`k`
    /// move (the last row is `+ add tag`), `d` deletes the selected tag,
    /// `a` or enter on the add row starts typing, esc closes. Typing mode:
    /// chars/backspace edit, enter commits the new tag, esc backs out to
    /// navigation. Every add/delete writes through [`Store::set_tags`]
    /// immediately — the popup always shows what is on disk.
    pub(crate) fn handle_tags_editor_key(&mut self, code: KeyCode) {
        if self
            .tags_editor
            .as_ref()
            .is_some_and(|editor| editor.typing.is_some())
        {
            self.handle_tags_typing_key(code);
            return;
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q' | '#') => self.tags_editor = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    editor.selected = (editor.selected + 1).min(editor.tags.len());
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    editor.selected = editor.selected.saturating_sub(1);
                }
            }
            KeyCode::Char('a') => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    editor.typing = Some(String::new());
                }
            }
            KeyCode::Enter => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    if editor.selected == editor.tags.len() {
                        editor.typing = Some(String::new());
                    }
                }
            }
            KeyCode::Char('d') => {
                let remaining = self.tags_editor.as_ref().and_then(|editor| {
                    (editor.selected < editor.tags.len()).then(|| {
                        let mut tags = editor.tags.clone();
                        tags.remove(editor.selected);
                        tags
                    })
                });
                if let Some(tags) = remaining {
                    self.write_tags(tags);
                }
            }
            _ => {}
        }
    }

    /// Typing mode of the tags editor: builds the new tag one key at a
    /// time; enter commits it (trimmed; whitespace inside is rejected).
    fn handle_tags_typing_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    editor.typing = None;
                }
            }
            KeyCode::Backspace => {
                if let Some(buffer) = self
                    .tags_editor
                    .as_mut()
                    .and_then(|editor| editor.typing.as_mut())
                {
                    buffer.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(buffer) = self
                    .tags_editor
                    .as_mut()
                    .and_then(|editor| editor.typing.as_mut())
                {
                    buffer.push(c);
                }
            }
            KeyCode::Enter => {
                let Some(typed) = self
                    .tags_editor
                    .as_mut()
                    .and_then(|editor| editor.typing.take())
                else {
                    return;
                };
                let tag = typed.trim().to_owned();
                if tag.is_empty() {
                    return; // nothing typed — just leave typing mode
                }
                if tag.contains(char::is_whitespace) {
                    self.status =
                        Some("tags cannot contain whitespace — use kebab-case".to_owned());
                    return;
                }
                let Some(mut tags) = self.tags_editor.as_ref().map(|e| e.tags.clone()) else {
                    return;
                };
                tags.push(tag);
                self.write_tags(tags);
            }
            _ => {}
        }
    }

    /// Writes `tags` for the workspace the editor is pinned to and syncs
    /// the popup (and the sidebar, via reload) with what landed on disk.
    fn write_tags(&mut self, tags: Vec<String>) {
        let Some(id) = self.tags_editor.as_ref().map(|editor| editor.id.clone()) else {
            return;
        };
        match self.store.set_tags(&id, tags) {
            Ok(meta) => {
                if let Some(editor) = self.tags_editor.as_mut() {
                    editor.tags = meta.tags;
                    editor.selected = editor.selected.min(editor.tags.len());
                }
                let _ = self.reload();
            }
            Err(e) => self.status = Some(format!("tag update failed: {e}")),
        }
    }

    /// The tags editor, when open.
    pub(crate) fn tags_editor(&self) -> Option<&TagsEditor> {
        self.tags_editor.as_ref()
    }

    /// Opens the `P` (PR) or `I` (ticket) attach prompt for the selected
    /// incident.
    pub(crate) fn open_attach_prompt(&mut self, kind: AttachKind) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to attach to".to_owned());
            return;
        };
        self.attach_prompt = Some(AttachPrompt {
            kind,
            id: rca.id.clone(),
            title: rca.meta.title.clone(),
            input: String::new(),
        });
    }

    /// Keystrokes while the attach prompt is open: type the reference,
    /// enter attaches it (validated, idempotent — the same `add_pr` /
    /// `add_ticket` the CLI uses), esc cancels. An invalid reference keeps
    /// the prompt open so it can be fixed.
    pub(crate) fn handle_attach_prompt_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.attach_prompt = None,
            KeyCode::Backspace => {
                if let Some(prompt) = self.attach_prompt.as_mut() {
                    prompt.input.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(prompt) = self.attach_prompt.as_mut() {
                    prompt.input.push(c);
                }
            }
            KeyCode::Enter => {
                let Some(prompt) = self.attach_prompt.as_ref() else {
                    return;
                };
                let (kind, id, reference) = (
                    prompt.kind,
                    prompt.id.clone(),
                    prompt.input.trim().to_owned(),
                );
                if reference.is_empty() {
                    self.attach_prompt = None;
                    return;
                }
                let result = match kind {
                    AttachKind::Pr => self.store.add_pr(&id, &reference),
                    AttachKind::Ticket => self.store.add_ticket(&id, &reference),
                };
                match result {
                    Ok(true) => {
                        self.attach_prompt = None;
                        let _ = self.reload();
                        self.status = Some(format!("attached {reference} to {id}"));
                    }
                    Ok(false) => {
                        self.attach_prompt = None;
                        self.status = Some(format!("{reference} is already attached"));
                    }
                    // Keep the prompt open so a bad reference can be fixed.
                    Err(e) => self.status = Some(format!("{e}")),
                }
            }
            _ => {}
        }
    }

    /// The attach prompt, when open.
    pub(crate) fn attach_prompt(&self) -> Option<&AttachPrompt> {
        self.attach_prompt.as_ref()
    }

    /// Opens the `D` delete confirmation for the selected incident. The
    /// delete itself only happens on an explicit `y` — see
    /// [`Self::handle_confirm_delete_key`].
    pub(crate) fn open_confirm_delete(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to delete".to_owned());
            return;
        };
        self.confirm_delete = Some(ConfirmDelete {
            id: rca.id.clone(),
            title: rca.meta.title.clone(),
        });
    }

    /// Keystrokes while the delete confirmation is open. Only an explicit
    /// `y` deletes; `n`, esc, or `q` cancel. Enter deliberately does
    /// nothing — a queued enter from normal navigation must never confirm
    /// a destructive action.
    pub(crate) fn handle_confirm_delete_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y' | 'Y') => {
                let Some(confirm) = self.confirm_delete.take() else {
                    return;
                };
                match self.store.delete(&confirm.id) {
                    Ok(_) => {
                        let _ = self.reload();
                        self.status = Some(format!("deleted {}", confirm.id));
                    }
                    Err(e) => self.status = Some(format!("delete failed: {e}")),
                }
            }
            KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => {
                self.confirm_delete = None;
                self.status = Some("delete cancelled".to_owned());
            }
            _ => {}
        }
    }

    /// The delete confirmation, when open.
    pub(crate) fn confirm_delete(&self) -> Option<&ConfirmDelete> {
        self.confirm_delete.as_ref()
    }
}
