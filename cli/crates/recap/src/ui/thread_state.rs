//! The Comments tab's state on [`App`]: which thread is shown, read marks,
//! and the per-recap comment counts.

use std::path::PathBuf;

use super::{App, Tab};
use crate::comments;
use crate::model::RecapId;

impl App {
    /// The selected recap's directory on the current shelf.
    pub(super) fn selected_dir(&self) -> Option<PathBuf> {
        let id = RecapId::new(&self.selected_row()?.id).ok()?;
        Some(self.store.shelf_dir(self.shelf).join(id.as_str()))
    }

    /// Recomputes the PR list's comment counts (unread, reviewed, pending).
    pub(super) fn refresh_marks(&mut self) {
        self.comment_marks = self
            .rows
            .iter()
            .filter_map(|r| {
                let author = self
                    .store
                    .load_from(self.shelf, &RecapId::new(&r.id).ok()?)
                    .ok()?
                    .manifest
                    .author
                    .unwrap_or_default();
                let dir = self.store.shelf_dir(self.shelf).join(&r.id);
                Some((
                    r.id.clone(),
                    comments::marks(&dir, &author, crate::store::now_unix()),
                ))
            })
            .collect();
        self.comments_error = std::fs::read_to_string(comments::error_path(&self.store))
            .ok()
            .and_then(|t| t.lines().nth(1).map(str::to_owned));
    }

    /// The shown thread, as an index into `thread_ids`.
    pub(super) fn current_thread(&self) -> Option<usize> {
        (!self.thread_ids.is_empty()).then(|| {
            self.thread_index
                .min(self.thread_ids.len().saturating_sub(1))
        })
    }

    /// `n` / `N`: the next / previous thread's sub-tab (wrapping round).
    pub(super) fn switch_thread(&mut self, delta: isize) {
        let n = self.thread_ids.len();
        if n == 0 {
            self.status = Some("no comment threads on this PR".to_owned());
            return;
        }
        let at = self.thread_index.min(n - 1);
        self.thread_index = (at + n).wrapping_add_signed(delta) % n;
        self.review_scroll = 0;
        self.build_doc();
        (self.view.scroll, self.view.cursor) = (0, 0);
        self.status = Some(format!("comment {} of {n}", self.thread_index + 1));
    }

    /// `J` / `K` in the Comments tab: scroll the review pane.
    pub(super) fn scroll_review(&mut self, delta: i16) {
        self.review_scroll = self.review_scroll.saturating_add_signed(delta);
    }

    /// Marks the shown thread read (showing it is reading it). Called after
    /// each draw of the Comments tab.
    pub(super) fn mark_current_read(&mut self) {
        self.set_current_read(true);
    }

    /// `u`: mark the shown thread unread again.
    pub(super) fn mark_current_unread(&mut self) {
        if self.tab != Tab::Comments {
            self.status =
                Some("u marks a comment thread unread — open Comments (3) first".to_owned());
            return;
        }
        self.set_current_read(false);
        self.status = Some("marked unread".to_owned());
    }

    fn set_current_read(&mut self, read: bool) {
        if self.tab != Tab::Comments {
            return;
        }
        let (Some(i), Some(dir)) = (self.current_thread(), self.selected_dir()) else {
            return;
        };
        let Some(id) = self.thread_ids.get(i).cloned() else {
            return;
        };
        let threads = comments::load(&dir).threads;
        let Some(thread) = threads.iter().find(|t| t.id == id) else {
            return;
        };
        let last = thread.last_counted().map(|c| c.id.clone());
        if comments::is_read(thread, &self.read_marks) == read {
            return;
        }
        match (read, last) {
            (true, Some(last)) => {
                self.read_marks.insert(id, last);
            }
            _ => {
                self.read_marks.remove(&id);
            }
        }
        if let Err(e) = comments::save_read(&dir, &self.read_marks) {
            self.status = Some(format!("could not save read marks: {e}"));
        }
        self.refresh_marks();
        // Re-render the sub-tabs (• unread) without moving the view.
        self.rebuild_doc_keeping_view();
    }
}
