//! The blocking event loop: input with a timeout, watcher events over a
//! channel, and burst-coalesced reloads. Idle CPU is ~0%.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use notify::{RecursiveMode, Watcher as _};
use ratatui::DefaultTerminal;

use crate::error::{Error, Result};
use crate::model::TaskStatus;
use crate::store::Store;
use crate::transcript;

use super::keys::Flow;
use super::{view, App};

impl App {
    /// Runs the TUI until the user quits.
    ///
    /// # Errors
    /// [`Error::Terminal`] on draw/input failures, [`Error::Watch`] if the
    /// store root cannot be watched.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let (tx, rx) = mpsc::channel::<()>();
        // The callback does the minimum: signal "something changed", for
        // changes the TUI actually shows.
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok_and(|event| event.paths.iter().any(|p| shown(p))) {
                    let _ = tx.send(()); // receiver gone means we are shutting down
                }
            })?;
        watcher.watch(self.store.root(), RecursiveMode::Recursive)?;
        spawn_session_pipeline(self.store.root().to_owned(), self.on_screen.clone());

        loop {
            if drain(&rx) {
                self.status = Some(
                    self.reload()
                        .unwrap_or_else(|| "reloaded (files changed on disk)".to_owned()),
                );
                if self.follow {
                    self.scroll = usize::MAX;
                }
            }
            self.share_on_screen();
            self.ensure_pane();
            // The loop turns at least every 250 ms (the poll timeout), so
            // bumping this here animates the working spinner at ~4 fps.
            self.tick = self.tick.wrapping_add(1);
            terminal
                .draw(|frame| view::draw(frame, &mut self))
                .map_err(Error::Terminal)?;

            if event::poll(Duration::from_millis(250)).map_err(Error::Terminal)? {
                match event::read().map_err(Error::Terminal)? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        match self.handle_key(key) {
                            Flow::Quit => return Ok(()),
                            Flow::Edit(path) => self.edit(terminal, &path)?,
                            Flow::Continue => {}
                        }
                    }
                    // Resize redraws on the next turn anyway.
                    _ => {}
                }
            }
        }
    }

    /// Tells the session pipeline thread which task is on screen.
    fn share_on_screen(&self) {
        let current = self
            .selected_project()
            .zip(self.selected_task())
            .and_then(|(p, t)| {
                (!t.meta.sessions.is_empty()).then(|| (p.dir.clone(), t.meta.sessions.clone()))
            });
        if let Ok(mut shared) = self.on_screen.lock() {
            if *shared != current {
                *shared = current;
            }
        }
    }

    /// Hands the terminal to the editor, then takes it back.
    fn edit(&mut self, terminal: &mut DefaultTerminal, path: &Path) -> Result<()> {
        ratatui::restore();
        let outcome = crate::editor::open(path);
        *terminal = ratatui::try_init().map_err(Error::Terminal)?;
        terminal.clear().map_err(Error::Terminal)?;
        self.status = Some(match outcome {
            Ok(()) => self
                .reload()
                .unwrap_or_else(|| "back from the editor".to_owned()),
            Err(e) => e.to_string(),
        });
        Ok(())
    }
}

/// The fallback session pipeline (hooks normally run it): a background
/// thread that, about once a second, runs [`transcript::etl`] for every
/// attached session of a task being worked and of the task on screen.
/// Writing a `.log` fires the store watcher, and the TUI reloads and shows
/// it. All parsing and rendering happens here, never on the UI thread.
fn spawn_session_pipeline(root: PathBuf, on_screen: super::OnScreen) {
    let _ = thread::Builder::new()
        .name("scout-sessions".to_owned())
        .spawn(move || {
            let store = Store::new(&root);
            let home = beagle_utils::paths::home_dir();
            let mut transcripts: HashMap<String, PathBuf> = HashMap::new();
            let mut turns: u32 = 0;
            loop {
                thread::sleep(Duration::from_secs(1));
                // Every 30 seconds: pause tasks that went quiet.
                turns = turns.wrapping_add(1);
                if turns.is_multiple_of(30) {
                    let (config, _) = crate::config::load();
                    crate::autopause::sweep(&store, &config, home.as_deref());
                }
                let Some(home) = &home else { continue };
                let mut wanted: Vec<(PathBuf, String)> = Vec::new();
                if let Ok(listing) = store.list() {
                    for project in listing.projects {
                        for task in project.tasks {
                            if task.meta.status == TaskStatus::Working {
                                for id in task.meta.sessions {
                                    wanted.push((project.dir.clone(), id));
                                }
                            }
                        }
                    }
                }
                if let Some((dir, ids)) = on_screen.lock().ok().and_then(|s| s.clone()) {
                    wanted.extend(ids.into_iter().map(|id| (dir.clone(), id)));
                }
                wanted.sort();
                wanted.dedup();
                for (dir, id) in wanted {
                    let path = match transcripts.get(&id) {
                        Some(path) => path.clone(),
                        None => match transcript::find_transcript(home, &id) {
                            Some(path) => {
                                transcripts.insert(id.clone(), path.clone());
                                path
                            }
                            None => continue,
                        },
                    };
                    let _ = transcript::etl(&path, &dir, &id, &store);
                }
            }
        });
}

/// Whether a change to `path` can change what the TUI shows. Not:
/// `beagle scout watch` hook recordings (`.sessions/`, written on every command of
/// every Claude session), session-sync position markers (`*.cursor`,
/// rewritten whenever a transcript grows), and atomic-write temp files.
/// Reloading on those made the TUI reload constantly.
fn shown(path: &Path) -> bool {
    let hidden_dir = path
        .components()
        .any(|c| c.as_os_str() == crate::session::SESSIONS_DIR);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let pipeline_scratch = [".cursor", ".lock", ".jsonl"]
        .iter()
        .any(|ext| name.ends_with(ext))
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|d| d == crate::transcript::CACHE_DIR);
    // `beagle_utils::fs::write_atomic` temp files: `.<name>.tmp<N>`.
    let atomic_temp = name.starts_with('.') && name.contains(".tmp");
    !(hidden_dir || pipeline_scratch || name.ends_with(".cursor") || atomic_temp)
}

/// Empties the channel; true if anything arrived. A burst of writes (an
/// agent saving several files) becomes one reload.
fn drain(rx: &Receiver<()>) -> bool {
    let mut any = false;
    loop {
        match rx.try_recv() {
            Ok(()) => any = true,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
        }
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn hook_recordings_and_cursors_do_not_reload() {
        assert!(shown(Path::new("/h/.scout/p/tasks/01-a.md")));
        assert!(
            shown(Path::new("/h/.scout/p/sessions/abc.log")),
            "the rendered view"
        );
        assert!(
            !shown(Path::new("/h/.scout/p/sessions/abc.jsonl")),
            "pipeline internals"
        );
        assert!(!shown(Path::new("/h/.scout/p/sessions/abc.lock")));
        assert!(!shown(Path::new("/h/.scout/.sessions/abc.jsonl")));
        assert!(!shown(Path::new("/h/.scout/p/sessions/abc.cursor")));
        assert!(!shown(Path::new("/h/.scout/p/tasks/.01-a.md.tmp0")));
        assert!(!shown(Path::new("/h/.scout/p/sessions/.abc.log.tmp3")));
    }
}
