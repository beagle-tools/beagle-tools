//! The TUI's event loop: watcher, poller, comment sync and key events.

use std::sync::mpsc;
use std::time::Duration;

use beagle_tui::watch;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use super::{view, App};
use crate::error::{Error, Result};
use crate::poller;

impl App {
    /// Runs the event loop until the user quits.
    ///
    /// # Errors
    /// [`Error::Terminal`] on draw/poll failures, [`Error::Watch`] if the
    /// watcher can't attach.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let recaps = self.store.recaps_dir();
        std::fs::create_dir_all(&recaps).map_err(|e| Error::io(&recaps, e))?;
        let (_watcher, rx) = watch::watch_dir(&recaps)?;

        // Live PR status: a background thread asks `gh` (now, then every
        // 3 min, plus whenever the PR set changes) and reports over a
        // channel, so the UI never blocks on the network.
        let (urls_tx, urls_rx) = mpsc::channel::<Vec<String>>();
        let (status_tx, status_rx) = mpsc::channel();
        poller::spawn(urls_rx, status_tx);
        // Comment threads: one background sync of every open/draft PR now,
        // then every few minutes. It writes files; the watcher reloads.
        crate::comments::spawn_sync(self.store.clone());
        self.auto_archive();
        let mut polled = self.pr_urls();
        let _ = urls_tx.send(polled.clone()); // poller gone = gh unavailable; fine

        while !self.quit {
            while let Ok(batch) = status_rx.try_recv() {
                self.apply_statuses(batch);
            }
            while let Ok(job) = self.jobs_rx.try_recv() {
                self.on_job(job);
            }
            if watch::drain(&rx) {
                self.reload();
            }
            let urls = self.pr_urls();
            if urls != polled || std::mem::take(&mut self.repoll) {
                polled.clone_from(&urls);
                let _ = urls_tx.send(urls); // poller gone = gh unavailable; fine
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
