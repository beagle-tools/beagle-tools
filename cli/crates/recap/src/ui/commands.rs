//! What the action keys do: copy and open links, confirm and run GitHub
//! actions, and move recaps to and from the archive.

use crossterm::event::KeyCode;

use super::{App, Confirm, Job, Tab};
use crate::model::{PrState, RecapId};
use crate::store::Shelf;
use crate::{actions, clipboard, links};

impl App {
    /// `i`: copy the recap id (`widgets-42`) — what every `beagle recap`
    /// command takes, handy for pointing an agent at it.
    pub(super) fn copy_recap_id(&mut self) {
        let Some(id) = self.selected_row().map(|r| r.id.clone()) else {
            return;
        };
        self.status = Some(match clipboard::copy(&id) {
            Ok(_) => format!("copied {id}"),
            Err(e) => e.to_string(),
        });
    }

    pub(super) fn copy_pr_url(&mut self) {
        let Some(url) = self.selected_row().map(|r| r.url.clone()) else {
            return;
        };
        self.status = Some(match clipboard::copy(&url) {
            Ok(_) => format!("copied {url}"),
            Err(e) => e.to_string(),
        });
    }

    pub(super) fn open_pr(&mut self) {
        let Some(url) = self.selected_row().map(|r| r.url.clone()) else {
            return;
        };
        self.status = Some(match links::open_url(&url) {
            Ok(()) => "opened the PR".to_owned(),
            Err(e) => e.to_string(),
        });
    }

    pub(super) fn open_ticket(&mut self) {
        let Some(m) = self.recap.as_ref().map(|r| &r.manifest) else {
            return;
        };
        let (ticket, url) = (m.ticket.clone(), m.ticket_url.clone());
        self.status = Some(match (ticket, url) {
            (_, Some(url)) => match links::open_url(&url) {
                Ok(()) => "opened the ticket".to_owned(),
                Err(e) => e.to_string(),
            },
            (Some(t), None) => format!("{t} has no ticket_url in recap.toml"),
            (None, None) => "this PR has no ticket".to_owned(),
        });
    }

    /// Keys while a confirmation is up: y/enter go, n/esc cancel, j/k/tab
    /// pick the merge method.
    pub(super) fn confirm_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => self.run_confirmed(),
            KeyCode::Char('n' | 'q') | KeyCode::Esc => {
                self.confirm = None;
                self.status = Some("cancelled".to_owned());
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.cycle_method(1),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => self.cycle_method(-1),
            _ => {}
        }
    }

    fn cycle_method(&mut self, delta: isize) {
        if let Some(Confirm::Merge { plan, choice, .. }) = self.confirm.as_mut() {
            let n = plan.methods.len();
            if n > 0 {
                *choice = (choice.wrapping_add_signed(delta) + n) % n;
            }
        }
    }

    /// The selected PR as `(id, url, label, state)`, if there is one and
    /// nothing else is running. Explains itself in the status line when not.
    fn actionable(&mut self) -> Option<(String, String, String, PrState)> {
        if self.shelf == Shelf::Archived {
            self.status = Some("this is the archive — U restores a recap first".to_owned());
            return None;
        }
        if let Some(busy) = &self.busy {
            self.status = Some(format!("still {busy}…"));
            return None;
        }
        let r = self.selected_row()?;
        let repo = r.repo.rsplit('/').next().unwrap_or(&r.repo);
        Some((
            r.id.clone(),
            r.url.clone(),
            format!("{repo}#{}", r.number),
            r.state,
        ))
    }

    pub(super) fn ask_ready(&mut self) {
        let Some((id, url, label, state)) = self.actionable() else {
            return;
        };
        let undo = match state {
            PrState::Draft => false,
            PrState::Open => true,
            PrState::Merged | PrState::Closed => {
                self.status = Some(format!("{label} is {state} — nothing to mark ready"));
                return;
            }
        };
        self.confirm = Some(Confirm::Ready {
            id,
            url,
            label,
            undo,
        });
    }

    pub(super) fn ask_close(&mut self) {
        let Some((id, url, label, state)) = self.actionable() else {
            return;
        };
        if matches!(state, PrState::Merged | PrState::Closed) {
            self.status = Some(format!(
                "{label} is already {state} — `a` archives its recap"
            ));
            return;
        }
        self.confirm = Some(Confirm::Close { id, url, label });
    }

    pub(super) fn ask_merge(&mut self) {
        let Some((id, url, label, state)) = self.actionable() else {
            return;
        };
        if state != PrState::Open {
            self.status = Some(match state {
                PrState::Draft => format!("{label} is a draft — D marks it ready first"),
                _ => format!("{label} is already {state}"),
            });
            return;
        }
        // Re-read GitHub before offering to merge: the list may be minutes old.
        let repo = self
            .selected_row()
            .map(|r| r.repo.clone())
            .unwrap_or_default();
        self.spawn_job(format!("checking whether {label} can merge"), move || {
            Job::MergeChecked {
                id,
                url: url.clone(),
                label,
                result: actions::check_merge(&url, &repo),
            }
        });
    }

    pub(super) fn archive_selected(&mut self) {
        if self.shelf == Shelf::Archived {
            self.status = Some("already in the archive — U restores it".to_owned());
            return;
        }
        let Some(id) = self.selected_row().map(|r| r.id.clone()) else {
            return;
        };
        self.status = Some(
            match RecapId::new(&id).and_then(|rid| self.store.archive(&rid)) {
                Ok(_) => format!("archived {id} (A shows the archive, U restores)"),
                Err(e) => e.to_string(),
            },
        );
        self.reload();
    }

    pub(super) fn unarchive_selected(&mut self) {
        if self.shelf != Shelf::Archived {
            self.status = Some("U restores from the archive — press A to open it".to_owned());
            return;
        }
        let Some(id) = self.selected_row().map(|r| r.id.clone()) else {
            return;
        };
        self.status = Some(
            match RecapId::new(&id).and_then(|rid| self.store.unarchive(&rid)) {
                Ok(_) => format!("restored {id}"),
                Err(e) => e.to_string(),
            },
        );
        self.reload();
    }

    pub(super) fn toggle_shelf(&mut self) {
        self.shelf = match self.shelf {
            Shelf::Active => Shelf::Archived,
            Shelf::Archived => Shelf::Active,
        };
        self.selected = 0;
        self.tab = Tab::Summary;
        self.reload();
        self.status = Some(match self.shelf {
            Shelf::Archived => "the archive — U restores · A goes back".to_owned(),
            Shelf::Active => "back to your PRs".to_owned(),
        });
    }
}
