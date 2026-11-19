//! The terminal loop: draw, poll keys every 250 ms, re-read the store
//! every 2 s (reviews arrive from the agentd job in another process),
//! and pick up finished GitHub work.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use tandem_cache::Cache;
use tandem_core::Result;

use super::keys::Flow;
use super::{view, App};
use crate::config::Config;

const POLL: Duration = Duration::from_millis(250);
const RELOAD: Duration = Duration::from_secs(2);

pub fn run(cache: Cache, db: PathBuf, config: Config) -> Result<()> {
    let mut app = App::new(cache, config);
    app.start_worker(db);
    app.reload()?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let mut last_reload = Instant::now();
    loop {
        terminal.draw(|frame| view::draw(frame, app))?;
        if event::poll(POLL)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key) == Flow::Quit {
                    return Ok(());
                }
            }
        }
        app.tick = app.tick.wrapping_add(1);
        if app.drain_worker() || last_reload.elapsed() >= RELOAD {
            if let Err(e) = app.reload() {
                app.error(e.to_string());
            }
            last_reload = Instant::now();
        }
    }
}
