//! The TUI: a report's items on the left (Overview, then one row per
//! ticket), the selected item's write-up on the right.
//!
//! All state lives in [`App`]; key handling ([`App::handle_key`]) is pure
//! state transition plus the occasional opener/clipboard call, so it is unit
//! tested without a terminal. Filesystem changes arrive from a `notify`
//! watcher over a channel and are coalesced into one reload per burst.

mod doc;
mod keys;
mod view;

use std::time::Duration;

use beagle_tui::watch;
use beagle_tui::{DefaultWidth, DocView, SideResize};
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::error::{Error, Result};
use crate::model::ReportDate;
use crate::store::{Report, Store, Ticket};

/// Which pane keys act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The item list.
    List,
    /// The write-up.
    Content,
}

/// The report picker overlay: every report, newest first.
#[derive(Debug)]
pub(crate) struct Picker {
    rows: Vec<crate::store::ReportSummary>,
    selected: usize,
}

/// The whole TUI state.
#[derive(Debug)]
pub struct App {
    store: Store,
    /// Report dates on disk, newest first.
    dates: Vec<ReportDate>,
    /// Index into `dates` of the report on screen.
    current: usize,
    report: Option<Report>,
    load_error: Option<String>,
    /// 0 = overview, `n` = ticket `n - 1`.
    selected: usize,
    focus: Focus,
    /// The write-up pane: the selected item's document and the position in it.
    view: DocView,
    status: Option<String>,
    help: bool,
    /// The report picker (`d`): its rows and the highlighted one.
    picker: Option<Picker>,
    quit: bool,
    /// The list's width and `w`'s resize mode (not saved across runs).
    side: SideResize,
}

impl App {
    /// Opens the store on `date`, or the newest report when `None`.
    ///
    /// # Errors
    /// [`Error::Io`] if the store root can't be listed.
    pub fn new(store: Store, date: Option<ReportDate>) -> Result<Self> {
        let dates = store.dates()?;
        let current = date
            .and_then(|d| dates.iter().position(|x| *x == d))
            .unwrap_or(0);
        let mut app = Self {
            store,
            dates,
            current,
            report: None,
            load_error: None,
            selected: 0,
            focus: Focus::List,
            view: DocView::new(),
            status: None,
            help: false,
            side: SideResize::new("list", DefaultWidth::Fixed(view::SIDEBAR_WIDTH)),
            picker: None,
            quit: false,
        };
        if let Some(d) = date {
            if !app.dates.contains(&d) {
                app.status = Some(format!("no report for {d}; showing the newest"));
            }
        }
        app.load_current();
        Ok(app)
    }

    fn current_date(&self) -> Option<ReportDate> {
        self.dates.get(self.current).copied()
    }

    fn load_current(&mut self) {
        self.report = None;
        self.load_error = None;
        if let Some(date) = self.current_date() {
            match self.store.load(date) {
                Ok(report) => self.report = Some(report),
                Err(e) => self.load_error = Some(e.to_string()),
            }
        }
        self.selected = self.selected.min(self.item_count().saturating_sub(1));
        self.rebuild_doc();
    }

    /// Re-reads the store, keeping the same report and ticket selected. If
    /// the newest report was on screen and a newer one appeared, follows it.
    pub fn reload(&mut self) {
        let date = self.current_date();
        let was_newest = self.current == 0;
        let ticket = self.selected_ticket().map(|t| t.id.clone());
        match self.store.dates() {
            Ok(dates) => self.dates = dates,
            Err(e) => self.status = Some(e.to_string()),
        }
        let newest = self.dates.first().copied();
        self.current = match date {
            Some(d) if was_newest && newest.is_some_and(|n| n > d) => {
                self.status = Some(format!(
                    "new report: {}",
                    newest.map(|n| n.to_string()).unwrap_or_default()
                ));
                0
            }
            Some(d) => self.dates.iter().position(|x| *x == d).unwrap_or(0),
            None => 0,
        };
        // A live reload (an agent writing) must not throw the reader back
        // to the top: keep the view and the cursor where they were.
        let keep_view = self.view.position();
        self.load_current();
        if let (Some(id), Some(report)) = (ticket, &self.report) {
            if let Some(i) = report.tickets.iter().position(|t| t.id == id) {
                self.selected = i + 1;
                self.rebuild_doc();
            }
        }
        if self.current_date() == date {
            self.view.set_position(keep_view);
        }
    }

    fn item_count(&self) -> usize {
        self.report.as_ref().map_or(1, |r| r.tickets.len() + 1)
    }

    fn selected_ticket(&self) -> Option<&Ticket> {
        let report = self.report.as_ref()?;
        self.selected
            .checked_sub(1)
            .and_then(|i| report.tickets.get(i))
    }

    fn rebuild_doc(&mut self) {
        let doc = match (&self.report, self.selected_ticket()) {
            (_, Some(t)) => doc::ticket(t),
            (Some(r), None) => doc::overview(r),
            (None, None) => Vec::new(),
        };
        self.view.set_doc(doc);
        self.view.reset_position();
    }

    /// Whether the user asked to quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Runs the event loop until the user quits.
    ///
    /// # Errors
    /// [`Error::Terminal`] on draw/poll failures, [`Error::Watch`] if the
    /// watcher can't attach.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let reports = self.store.reports_dir();
        std::fs::create_dir_all(&reports).map_err(|e| Error::io(&reports, e))?;
        let (_watcher, rx) = watch::watch_dir(&reports)?;

        while !self.quit {
            if watch::drain(&rx) {
                self.reload();
            }
            terminal.draw(|frame| view::draw(frame, &mut self))?;
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/ui.rs"]
mod tests;
#[cfg(test)]
#[path = "../tests/ui_keymap.rs"]
mod tests_keymap;
