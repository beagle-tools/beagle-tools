//! Is agentd running?
//!
//! Nothing in the store says: a stopped daemon and an idle one leave the
//! same rows behind, so agenttop looked identical either way — and with the
//! daemon stopped nothing is scheduled, and a run you start sits Pending
//! forever. agentd now writes a heartbeat every tick, and every UI reads it
//! through [`status`]:
//!
//! ```text
//! ~/.local/state/agentd/agentd.heartbeat    <pid> <tick seconds> <unix time>
//! ```
//!
//! - process alive, beat recent       → [`DaemonStatus::Running`]
//! - process alive, beat stale        → [`DaemonStatus::Stalled`] (a hung tick)
//! - process gone                     → [`DaemonStatus::Stopped`]
//!
//! With no heartbeat file (a daemon from before it existed), a running
//! process named `agentd` still counts as running.

use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};

use crate::error::CoreError;
use crate::paths;

/// One heartbeat: who wrote it, how often it ticks, when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beat {
    /// The daemon's process id.
    pub pid: u32,
    /// Its `--tick-seconds`: how stale a beat may be before it is stalled.
    pub tick_seconds: u64,
    /// When the beat was written, as a unix timestamp.
    pub at: i64,
}

/// What agentd is doing, read from its heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    /// Ticking on schedule.
    Running,
    /// The process is there but has not ticked in a while.
    Stalled {
        /// Its last heartbeat.
        last: DateTime<Utc>,
    },
    /// No daemon: nothing is scheduled and nothing queued will start.
    /// `last` is its last heartbeat, when there was one.
    Stopped {
        /// Its last heartbeat, if it ever wrote one.
        last: Option<DateTime<Utc>>,
    },
}

impl DaemonStatus {
    /// True only for [`DaemonStatus::Running`].
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// One line for a header or `agentctl status`; `None` when running.
    #[must_use]
    pub fn banner(&self, now: DateTime<Utc>) -> Option<String> {
        let ago = |t: &DateTime<Utc>| {
            let secs = (now - *t).num_seconds().max(0);
            match secs {
                s if s < 120 => format!("{s}s"),
                s if s < 7200 => format!("{}m", s / 60),
                s if s < 172_800 => format!("{}h", s / 3600),
                s => format!("{}d", s / 86400),
            }
        };
        match self {
            Self::Running => None,
            Self::Stalled { last } => Some(format!(
                "agentd STALLED — no tick for {}; runs are not being started",
                ago(last)
            )),
            Self::Stopped { last } => Some(match last {
                Some(t) => format!(
                    "agentd STOPPED (last seen {} ago) — nothing scheduled, nothing starts",
                    ago(t)
                ),
                None => "agentd STOPPED — nothing scheduled, nothing starts".to_owned(),
            }),
        }
    }
}

/// `<state dir>/agentd.heartbeat`: `<pid> <tick seconds> <unix time>`.
pub fn heartbeat_path() -> Result<PathBuf, CoreError> {
    Ok(paths::state_dir()?.join("agentd.heartbeat"))
}

/// Written by agentd once per tick.
pub fn beat(tick_seconds: u64) -> Result<(), CoreError> {
    let line = format!(
        "{} {tick_seconds} {}\n",
        std::process::id(),
        Utc::now().timestamp()
    );
    let path = heartbeat_path()?;
    std::fs::write(&path, line).map_err(|e| CoreError::io(&path, e))
}

fn parse(text: &str) -> Option<Beat> {
    let mut it = text.split_whitespace();
    Some(Beat {
        pid: it.next()?.parse().ok()?,
        tick_seconds: it.next()?.parse().ok()?,
        at: it.next()?.parse().ok()?,
    })
}

/// The decision, pure: `alive` is whether the heartbeat's pid (or, with no
/// heartbeat, any `agentd` process) is running.
#[must_use]
pub fn classify(beat: Option<Beat>, alive: bool, now: DateTime<Utc>) -> DaemonStatus {
    let last = beat.and_then(|b| Utc.timestamp_opt(b.at, 0).single());
    match (beat, alive) {
        (_, false) => DaemonStatus::Stopped { last },
        (None, true) => DaemonStatus::Running,
        (Some(b), true) => {
            // Three missed ticks, and never less than a minute: a tick that
            // spawns a run or prunes retention can take a while.
            let grace = i64::try_from(b.tick_seconds.saturating_mul(3))
                .unwrap_or(i64::MAX)
                .max(60);
            if now.timestamp() - b.at <= grace {
                DaemonStatus::Running
            } else {
                DaemonStatus::Stalled {
                    last: last.unwrap_or(now),
                }
            }
        }
    }
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn any_agentd() -> bool {
    Command::new("pgrep")
        .args(["-x", "agentd"])
        .stdout(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The daemon's status now. Cheap: one file read and one `kill -0`.
#[must_use]
pub fn status() -> DaemonStatus {
    let beat = heartbeat_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| parse(&t));
    let alive = match beat {
        Some(b) => pid_alive(b.pid),
        None => any_agentd(),
    };
    classify(beat, alive, Utc::now())
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_heartbeat_parses() {
        assert_eq!(
            parse("123 5 1700000000\n"),
            Some(Beat {
                pid: 123,
                tick_seconds: 5,
                at: 1_700_000_000
            })
        );
        assert_eq!(parse("garbage"), None);
    }

    #[test]
    fn running_stalled_and_stopped() {
        let now = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
        let beat = |ago: i64| {
            Some(Beat {
                pid: 1,
                tick_seconds: 5,
                at: 1_700_000_100 - ago,
            })
        };
        assert_eq!(classify(beat(10), true, now), DaemonStatus::Running);
        assert!(matches!(
            classify(beat(300), true, now),
            DaemonStatus::Stalled { .. }
        ));
        assert!(matches!(
            classify(beat(10), false, now),
            DaemonStatus::Stopped { last: Some(_) }
        ));
        assert_eq!(classify(None, true, now), DaemonStatus::Running);
        assert_eq!(
            classify(None, false, now),
            DaemonStatus::Stopped { last: None }
        );
        assert!(DaemonStatus::Stopped { last: None }
            .banner(now)
            .unwrap()
            .contains("STOPPED"));
        assert_eq!(DaemonStatus::Running.banner(now), None);
    }
}
