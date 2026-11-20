//! The content pane's document: building it for the selected recap and
//! tab, switching tabs, and laying it out (delta and the Comments tab
//! depend on the pane width). Split from `ui/mod.rs`.

use super::{doc, threads, App, Focus, Tab};

impl App {
    pub(super) fn rebuild_doc(&mut self) {
        self.build_doc();
        self.view.reset_position();
    }

    pub(super) fn rebuild_doc_keeping_view(&mut self) {
        let position = self.view.position();
        self.build_doc();
        self.view.set_position(position);
    }

    pub(super) fn build_doc(&mut self) {
        let doc;
        (doc, self.file_starts) = match (&self.recap, self.tab) {
            (Some(r), Tab::Summary) => (doc::summary(r), Vec::new()),
            // delta needs the pane width: `layout` renders it on first draw.
            (Some(_), Tab::Diff) if self.use_delta => (Vec::new(), Vec::new()),
            (Some(r), Tab::Diff) => {
                let d = doc::diff(r);
                (d.lines, d.file_starts)
            }
            (Some(r), Tab::Comments) => match self.selected_dir() {
                Some(dir) => {
                    let t = threads::document(
                        r,
                        &dir,
                        &self.read_marks,
                        &self.comments_config,
                        self.comments_width,
                        self.thread_index,
                    );
                    self.thread_ids = t.ids;
                    (t.lines, Vec::new())
                }
                None => (Vec::new(), Vec::new()),
            },
            (None, _) => (Vec::new(), Vec::new()),
        };
        self.delta_width = 0;
        self.view.set_doc(doc);
    }

    pub(super) fn set_tab(&mut self, tab: Tab) {
        if tab == self.tab {
            return;
        }
        self.tab_pos[self.tab.index()] = (self.view.scroll, self.view.cursor);
        self.tab = tab;
        // The review pane exists only on the Comments tab.
        if tab != Tab::Comments && self.focus == Focus::Review {
            self.focus = Focus::Content;
        }
        self.build_doc();
        (self.view.scroll, self.view.cursor) = self.tab_pos[tab.index()];
        self.view.pan = 0;
    }

    /// Renders the Diff tab with delta at `width`, if that's what's in use
    /// and it isn't already at this width. Falls back to the built-in
    /// renderer (for good) if delta fails.
    pub(super) fn render_delta_at(&mut self, width: u16) {
        if self.tab != Tab::Diff || !self.use_delta || self.delta_width == width {
            return;
        }
        let Some(recap) = &self.recap else { return };
        let d = if let Some(d) = crate::diff::render_delta(&recap.diff, width) {
            d
        } else {
            self.use_delta = false;
            self.status = Some("delta failed; showing the built-in diff".to_owned());
            doc::diff(recap)
        };
        self.file_starts = d.file_starts;
        self.view.doc = d.lines;
        self.delta_width = width;
        self.view.laid_width = 0; // force the re-layout below
    }

    pub(super) fn layout(&mut self, width: u16) {
        self.render_delta_at(width);
        // The Comments tab wraps its own text (so every row keeps its
        // gutter): rebuild it when the pane is a new width.
        if self.tab == Tab::Comments && self.comments_width != width {
            self.comments_width = width;
            self.rebuild_doc_keeping_view();
        }
        self.view.layout(width);
    }
}
