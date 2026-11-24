//! `beagle scout watch`: stream one agent session's terminal commands, and what they
//! printed, like a terminal someone is typing into.
//!
//! It is a plain streaming printer, not a full-screen app, so it scrolls,
//! searches and copies like any terminal output in a Herdr (or tmux) pane.
//! With no `--session` it picks the session itself — the newest one recorded
//! from the same Herdr tab, else from the same directory, else the newest —
//! and switches when a newer one appears (e.g. after `/clear`).
//!
//! The agent's own scout calls are shown as banners rather than commands:
//! `beagle scout task start` becomes `▶ task 02-… · title`, and `beagle scout log` becomes
//! a `✎` narration line, so the stream also says which task the commands
//! that follow belong to.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::error::{Error, Result};
use crate::model::TaskStatus;
use crate::session::{self, Event};
use crate::store::Store;

/// How often the recording is checked for new lines.
const POLL: Duration = Duration::from_millis(150);
/// How often the auto-picked session is re-checked.
const REPICK: Duration = Duration::from_secs(2);
/// Output longer than this is cut (head and tail kept) unless `--full`.
const MAX_LINES: usize = 40;
const HEAD_LINES: usize = 12;

/// What `beagle scout watch` was asked to do.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // independent display flags, not a state machine
pub struct Options {
    /// Watch exactly this session instead of picking one.
    pub session: Option<String>,
    /// How many past commands to replay on attach.
    pub history: usize,
    /// Show commands and their status, but not their output.
    pub commands_only: bool,
    /// Never cut long output.
    pub full: bool,
    /// Emit ANSI colors.
    pub color: bool,
    /// Print the replay and exit instead of following.
    pub once: bool,
    /// The watcher's own Herdr tab (`HERDR_TAB_ID`), for picking.
    pub tab: Option<String>,
    /// The watcher's working directory, for picking.
    pub cwd: PathBuf,
}

/// Follows a session until interrupted (Ctrl-C).
///
/// # Errors
/// [`Error::Io`] if a recording cannot be read, or stdout cannot be written.
pub fn run(store: &Store, options: &Options) -> Result<()> {
    let root = store.root();
    let mut out = std::io::stdout().lock();
    let mut renderer = Renderer::new(store, options);
    let mut current: Option<Attached> = None;
    let mut last_pick: Option<Instant> = None;
    let mut waiting = false;
    if options.once {
        return if let Some((id, path)) = target(root, options) {
            attach(&mut out, &mut renderer, id, path, options).map(|_| ())
        } else {
            let hint = if options.session.as_deref() == Some("auto") {
                "Follows the newest Claude Code session in this Herdr tab (or directory),\n\
                 and switches to a new one after /clear."
            } else {
                "No commands recorded for this session yet."
            };
            write_out(&mut out, &format!("{hint}\n"))
        };
    }
    loop {
        let due = last_pick.is_none_or(|t| t.elapsed() >= REPICK);
        if due && (options.session.is_none() || current.is_none()) {
            last_pick = Some(Instant::now());
            match target(root, options) {
                Some((id, path)) if current.as_ref().is_none_or(|c| c.id != id) => {
                    let attached = attach(&mut out, &mut renderer, id, path, options)?;
                    current = Some(attached);
                }
                None if current.is_none() && !waiting => {
                    let hint = if options.session.is_some() {
                        "waiting for that session to record a command…"
                    } else {
                        "waiting for a Claude Code session to run a command… \
                         (hooks installed? `beagle scout hook status`)"
                    };
                    write_out(&mut out, &format!("{}\n", renderer.paint.dim(hint)))?;
                    waiting = true;
                }
                _ => {}
            }
        }
        if let Some(attached) = &mut current {
            for event in attached.read_new()? {
                write_out(&mut out, &renderer.render(&event))?;
            }
        }
        thread::sleep(POLL);
    }
}

/// The session to watch right now: the named one if it exists, else the
/// best pick.
fn target(root: &Path, options: &Options) -> Option<(String, PathBuf)> {
    if let Some(id) = &options.session {
        let path = session::file(root, id);
        return path.exists().then(|| (id.clone(), path));
    }
    let sessions = session::list(root);
    session::pick(&sessions, options.tab.as_deref(), &options.cwd)
        .map(|s| (s.id.clone(), s.path.clone()))
}

/// A followed recording: where we are in it, and any half-written line.
struct Attached {
    id: String,
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
}

impl Attached {
    /// Reads whatever was appended since last time and returns the complete
    /// lines as events, keeping a trailing partial line for next time.
    fn read_new(&mut self) -> Result<Vec<Event>> {
        let mut file = File::open(&self.path).map_err(Error::io(&self.path))?;
        let len = file.metadata().map_err(Error::io(&self.path))?.len();
        if len < self.offset {
            self.offset = 0; // truncated or replaced: start over
            self.partial.clear();
        }
        if len == self.offset {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))
            .map_err(Error::io(&self.path))?;
        let read = file
            .read_to_end(&mut self.partial)
            .map_err(Error::io(&self.path))?;
        self.offset += read as u64;
        let mut events = Vec::new();
        while let Some(newline) = self.partial.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=newline).collect();
            if let Some(event) = session::parse_line(&String::from_utf8_lossy(&line)) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

/// Starts following `id`: prints a header and replays the last commands.
fn attach(
    out: &mut impl std::io::Write,
    renderer: &mut Renderer<'_>,
    id: String,
    path: PathBuf,
    options: &Options,
) -> Result<Attached> {
    let mut attached = Attached {
        id,
        path,
        offset: 0,
        partial: Vec::new(),
    };
    let events = attached.read_new()?;
    let starts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::Start { .. }))
        .map(|(i, _)| i)
        .collect();
    let from = if options.history == 0 {
        events.len()
    } else {
        starts
            .len()
            .checked_sub(options.history)
            .and_then(|skip| starts.get(skip).copied())
            .unwrap_or(0)
    };
    renderer.reset();
    let cwd = events.iter().find_map(|e| match e {
        Event::Start { cwd, .. } => cwd.clone(),
        _ => None,
    });
    let mode = if options.session.is_some() {
        "pinned"
    } else if options.tab.is_some() {
        "auto: follows this tab's Claude"
    } else {
        "auto: follows this directory's Claude"
    };
    let header = format!(
        "── watching session {}{} · {mode} · {} earlier command(s) · Ctrl-C to stop ──",
        attached.id,
        cwd.map(|c| format!(" · {c}")).unwrap_or_default(),
        starts.len().min(options.history),
    );
    write_out(out, &format!("\n{}\n", renderer.paint.dim(&header)))?;
    for event in &events[from..] {
        write_out(out, &renderer.render(event))?;
    }
    Ok(attached)
}

fn write_out(out: &mut impl std::io::Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(Error::io("<stdout>"))
}

/// The first 8 characters of a session id: enough to tell sessions apart
/// and to pass back as `beagle scout watch <prefix>`.
pub(crate) fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// Prints `beagle scout watch --list`.
pub fn list(root: &Path) {
    let sessions = session::list(root);
    if sessions.is_empty() {
        println!(
            "no recorded sessions in {} — run `beagle scout hook install`, then start Claude Code",
            session::dir(root).display()
        );
        return;
    }
    for s in sessions {
        let age = SystemTime::now()
            .duration_since(s.modified)
            .unwrap_or_default();
        println!(
            "{}  {:>7} ago  {:>3} cmds  tab {:<8}  {}",
            s.id,
            human_age(age),
            s.commands,
            s.tab.as_deref().unwrap_or("-"),
            s.cwd.as_deref().unwrap_or("-"),
        );
        if let Some(last) = s.last_command {
            println!("    last: {}", one_line(&last, 100));
        }
    }
}

fn duration(ms: Option<u64>) -> String {
    match ms {
        None => String::new(),
        Some(ms) if ms < 1000 => format!(" {ms}ms"),
        Some(ms) if ms < 60_000 => format!(" {:.1}s", Duration::from_millis(ms).as_secs_f64()),
        Some(ms) => format!(" {}m{:02}s", ms / 60_000, (ms % 60_000) / 1000),
    }
}

/// `45s`, `3m`, `2h`, `5d`: the largest whole unit of `age`.
pub(crate) fn human_age(age: Duration) -> String {
    beagle_utils::duration::age_single_unit(i64::try_from(age.as_secs()).unwrap_or(i64::MAX))
}

/// The first line of `text`, cut to `max` characters; one `…` marks a cut
/// or hidden lines.
pub(crate) fn one_line(text: &str, max: usize) -> String {
    let (mut out, more) = beagle_utils::text::first_line(text, max);
    if more > 0 && !out.ends_with('…') {
        out.push('…');
    }
    out
}

mod calls;
mod render;

pub(crate) use calls::{scout_call, scout_calls, ScoutCall};
use render::Renderer;

#[cfg(test)]
mod tests;
