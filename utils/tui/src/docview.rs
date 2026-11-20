//! A scrolling document pane: a laid-out document with a vim-style line
//! cursor, a visual-line selection, vertical scroll and horizontal pan.
//!
//! The app owns one [`DocView`] per content pane, replaces its document with
//! [`DocView::set_doc`], calls [`DocView::layout`] from its draw code with
//! the pane width, and forwards movement keys to [`DocView::nav_key`]. The
//! fields are public so apps (and their tests) can read and restore the view
//! position, e.g. across a live reload.

use crossterm::event::KeyCode;
use ratatui::text::Line;

use crate::clipboard;
use crate::wrap::{self, DocLine};

/// How far `h` / `l` pan, in columns.
const PAN_STEP: u16 = 8;

/// The state of one scrolling document pane.
#[derive(Debug, Clone, PartialEq)]
pub struct DocView {
    /// The document, as built by the app.
    pub doc: Vec<DocLine>,
    /// `doc` laid out for `laid_width`, rebuilt on resize.
    pub laid: Vec<Line<'static>>,
    /// For each row of `laid`, the index of the `doc` line it came from.
    pub owners: Vec<usize>,
    /// The width `laid` was built for (0 = not laid out yet).
    pub laid_width: u16,
    /// Widest row of `laid`, for bounding horizontal panning.
    pub laid_max_width: u16,
    /// Pane inner width from the last draw.
    pub content_width: u16,
    /// The line cursor: a row of `laid`.
    pub cursor: usize,
    /// Visual-line selection anchor (`v`), if one is active.
    pub anchor: Option<usize>,
    /// Viewport height from the last draw, for paging and clamping.
    pub viewport: u16,
    /// First visible row.
    pub scroll: u16,
    /// Columns panned to the right.
    pub pan: u16,
}

impl Default for DocView {
    fn default() -> Self {
        Self::new()
    }
}

impl DocView {
    /// An empty view sized for a typical terminal until the first draw.
    #[must_use]
    pub fn new() -> Self {
        Self {
            doc: Vec::new(),
            laid: Vec::new(),
            owners: Vec::new(),
            laid_width: 0,
            laid_max_width: 0,
            content_width: 80,
            cursor: 0,
            anchor: None,
            viewport: 20,
            scroll: 0,
            pan: 0,
        }
    }

    /// Replaces the document and forces a re-layout on the next draw. The
    /// position is kept (callers reset it with [`DocView::reset_position`]).
    pub fn set_doc(&mut self, doc: Vec<DocLine>) {
        self.doc = doc;
        self.invalidate();
    }

    /// Drops the layout so the next [`DocView::layout`] rebuilds it; the old
    /// rows belong to the old document. Ends any selection.
    pub fn invalidate(&mut self) {
        self.laid.clear();
        self.owners.clear();
        self.laid_width = 0;
        self.anchor = None;
    }

    /// Back to the top-left, cursor on the first row.
    pub fn reset_position(&mut self) {
        self.scroll = 0;
        self.pan = 0;
        self.cursor = 0;
    }

    /// `(scroll, pan, cursor)`, to restore after a rebuild.
    #[must_use]
    pub fn position(&self) -> (u16, u16, usize) {
        (self.scroll, self.pan, self.cursor)
    }

    /// Restores a [`DocView::position`].
    pub fn set_position(&mut self, (scroll, pan, cursor): (u16, u16, usize)) {
        (self.scroll, self.pan, self.cursor) = (scroll, pan, cursor);
    }

    /// Lays the document out for `width` columns when that differs from the
    /// last layout, keeping the cursor on the same source line.
    pub fn layout(&mut self, width: u16) {
        if width == self.laid_width {
            return;
        }
        let owner = self.owners.get(self.cursor).copied();
        (self.laid, self.owners) = wrap::layout_with_owners(&self.doc, width);
        self.laid_width = width;
        self.laid_max_width = self
            .laid
            .iter()
            .map(|l| u16::try_from(l.width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        if let Some(owner) = owner {
            self.cursor = self.owners.iter().position(|o| *o == owner).unwrap_or(0);
        }
        self.anchor = None;
        self.clamp_cursor();
    }

    /// How far the pane can pan right: only as far as the widest line needs
    /// to come fully into view.
    #[must_use]
    pub fn max_pan(&self) -> u16 {
        self.laid_max_width.saturating_sub(self.content_width)
    }

    /// The last useful scroll offset.
    #[must_use]
    pub fn max_scroll(&self) -> u16 {
        let len = u16::try_from(self.laid.len()).unwrap_or(u16::MAX);
        len.saturating_sub(self.viewport)
    }

    /// Keeps the cursor inside the document and on screen.
    pub fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.laid.len().saturating_sub(1));
        self.reveal_cursor();
    }

    /// Scrolls just enough to keep the cursor row on screen.
    pub fn reveal_cursor(&mut self) {
        let cursor = u16::try_from(self.cursor).unwrap_or(u16::MAX);
        let height = self.viewport.max(1);
        if cursor < self.scroll {
            self.scroll = cursor;
        } else if cursor >= self.scroll.saturating_add(height) {
            self.scroll = cursor + 1 - height;
        }
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// Moves the line cursor by `delta` rows; the view follows it.
    pub fn move_cursor(&mut self, delta: isize) {
        self.cursor = self.cursor.saturating_add_signed(delta);
        self.clamp_cursor();
    }

    /// Called when the pane gains focus: ends any selection and, if the
    /// cursor is scrolled out of view, puts it on the first visible row.
    pub fn enter(&mut self) {
        self.anchor = None;
        let top = usize::from(self.scroll);
        let on_screen = (top..top + usize::from(self.viewport)).contains(&self.cursor);
        if !on_screen {
            self.cursor = top;
            self.clamp_cursor();
        }
    }

    /// Half a viewport, at least one row.
    #[must_use]
    pub fn half_page(&self) -> isize {
        isize::from(i16::try_from(self.viewport / 2).unwrap_or(i16::MAX).max(1))
    }

    /// A viewport less two rows of overlap, at least one row.
    #[must_use]
    pub fn page(&self) -> isize {
        isize::from(
            i16::try_from(self.viewport.saturating_sub(2))
                .unwrap_or(i16::MAX)
                .max(1),
        )
    }

    /// The shared movement keys, usable whichever pane has focus:
    /// `J`/`K` line, `ctrl-d`/`ctrl-u` half page, `space`/`PgDn`/`PgUp`
    /// page, `g`/`G`/`Home`/`End` ends, `h`/`l`/`←`/`→` pan. Returns
    /// `false` (and changes nothing) for any other key.
    pub fn nav_key(&mut self, code: KeyCode, ctrl: bool) -> bool {
        match code {
            KeyCode::Char('d') if ctrl => self.move_cursor(self.half_page()),
            KeyCode::Char('u') if ctrl => self.move_cursor(-self.half_page()),
            KeyCode::Char('J') => self.move_cursor(1),
            KeyCode::Char('K') => self.move_cursor(-1),
            KeyCode::Char(' ') | KeyCode::PageDown => self.move_cursor(self.page()),
            KeyCode::PageUp => self.move_cursor(-self.page()),
            KeyCode::Char('g') | KeyCode::Home => self.move_cursor(isize::MIN),
            KeyCode::Char('G') | KeyCode::End => self.move_cursor(isize::MAX),
            KeyCode::Char('h') | KeyCode::Left => self.pan = self.pan.saturating_sub(PAN_STEP),
            KeyCode::Char('l') | KeyCode::Right => {
                self.pan = self.pan.saturating_add(PAN_STEP).min(self.max_pan());
            }
            _ => return false,
        }
        true
    }

    /// `v`: starts a visual-line selection at the cursor, or ends one.
    /// Returns the status hint to show when a selection starts.
    pub fn toggle_visual(&mut self) -> Option<&'static str> {
        if self.anchor.take().is_none() {
            self.anchor = Some(self.cursor);
            return Some("visual: j/k extend · y copy · v/esc cancel");
        }
        None
    }

    /// The selected rows as `(first, last)`: the visual selection, or just
    /// the cursor row.
    #[must_use]
    pub fn selection(&self) -> (usize, usize) {
        let anchor = self.anchor.unwrap_or(self.cursor);
        (anchor.min(self.cursor), anchor.max(self.cursor))
    }

    /// The text a yank copies: every source line touched by the selection,
    /// each once, in order.
    #[must_use]
    pub fn selection_text(&self) -> String {
        let (first, last) = self.selection();
        let mut owners: Vec<usize> = self.owners.get(first..=last).unwrap_or_default().to_vec();
        owners.dedup();
        owners
            .iter()
            .filter_map(|&i| self.doc.get(i))
            .map(wrap::plain_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `y`: copies the selection (or the cursor line) to the clipboard and
    /// ends the selection. Returns the status line to show.
    pub fn yank(&mut self) -> String {
        let (first, last) = self.selection();
        let text = self.selection_text();
        self.anchor = None;
        let rows = last - first + 1;
        match clipboard::copy(&text) {
            Ok(_) if rows == 1 => "copied 1 line".to_owned(),
            Ok(_) => format!("copied {rows} lines"),
            Err(e) => e.to_string(),
        }
    }

    /// The rows on screen: `viewport` rows from `scroll`. Only this slice is
    /// handed to the widget, never a per-frame copy of the whole document.
    #[must_use]
    pub fn visible(&self) -> Vec<Line<'static>> {
        self.laid
            .iter()
            .skip(usize::from(self.scroll))
            .take(usize::from(self.viewport))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
#[path = "tests/docview.rs"]
mod tests;
