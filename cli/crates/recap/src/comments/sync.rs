//! Syncing every open PR's threads, and listing what waits on a review.

use std::fmt::Write as _;
use std::time::Duration;

use serde::Serialize;

use super::local::{forget_gone, write_json};
use super::Threads;
use super::{dir, error_path, fetch, from_others, load, need, open_only, CommentsConfig, Need};
use crate::error::{Error, Result};
use crate::model::{PrState, RecapId};
use crate::store::Store;

/// Fetches every open or draft recap's threads and writes them. A failure
/// (after the rate-limit retry) is written to [`error_path`] and returned;
/// a full success removes that file.
///
/// # Errors
/// The first recap that could not be fetched.
pub fn sync_all(store: &Store, config: &CommentsConfig, now: &str) -> Result<String> {
    let (rows, _) = store.list()?;
    let mut synced = 0;
    let mut threads = 0;
    let mut failures = Vec::new();
    for row in rows
        .iter()
        .filter(|r| matches!(r.state, PrState::Open | PrState::Draft))
    {
        match fetch(&row.repo, row.number, config) {
            Ok(found) => {
                let Ok(id) = RecapId::new(&row.id) else {
                    continue;
                };
                let author = store
                    .load(&id)
                    .ok()
                    .and_then(|r| r.manifest.author)
                    .unwrap_or_default();
                let found = open_only(from_others(found, &author));
                threads += found.len();
                let recap_dir = store.recap_dir(&id);
                forget_gone(&recap_dir, &found);
                super::gitignore::ensure(&recap_dir);
                write_json(
                    &dir(&recap_dir).join("threads.json"),
                    &Threads {
                        fetched: now.to_owned(),
                        threads: found,
                    },
                )?;
                synced += 1;
            }
            Err(e) => failures.push(format!("{}: {e}", row.id)),
        }
    }
    let err_path = error_path(store);
    if failures.is_empty() {
        let _ = std::fs::remove_file(&err_path); // none there is fine
        Ok(format!("{synced} PR(s), {threads} thread(s)"))
    } else {
        let mut text = format!("comments sync failed at {now}\n");
        for f in &failures {
            let _ = writeln!(text, "- {f}");
        }
        std::fs::write(&err_path, &text).map_err(|e| Error::io(&err_path, e))?;
        Err(Error::Tool {
            tool: "gh",
            message: failures.join("; "),
        })
    }
}

/// How often the TUI's background thread re-syncs comments.
pub const SYNC_INTERVAL: Duration = Duration::from_mins(5);

/// Starts the TUI's comment sync: now, then every [`SYNC_INTERVAL`], one
/// pass over every open/draft PR. It writes files (the TUI's watcher
/// reloads); failures land in [`error_path`]. Quietly does nothing without
/// `gh`. The ignore list is re-read each pass, so a config edit applies
/// without a restart.
pub fn spawn_sync(store: Store) {
    std::thread::spawn(move || {
        if !crate::poller::gh_available() {
            return;
        }
        loop {
            let config = crate::config::load_quietly();
            let now = crate::store::now_unix().to_string();
            let _ = sync_all(&store, &config.comments, &now); // recorded in error_path
            std::thread::sleep(SYNC_INTERVAL);
        }
    });
}

/// One sync pass now, in the background (the TUI's `r`).
pub fn spawn_sync_once(store: Store) {
    std::thread::spawn(move || {
        let config = crate::config::load_quietly();
        let now = crate::store::now_unix().to_string();
        let _ = sync_all(&store, &config.comments, &now); // recorded in error_path
    });
}

/// A thread waiting on an agent review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pending {
    /// Recap id.
    pub recap: String,
    /// PR URL.
    pub pr: String,
    /// Thread id.
    pub thread: String,
    /// `path:line`, `conversation` or `review · …`.
    pub place: String,
    /// Who asked.
    pub author: String,
    /// The comment to answer.
    pub comment: String,
}

/// Every thread, across open and draft recaps, that [`need`] says to
/// review now: someone else spoke last, at least [`super::QUIET_SECS`] ago, it is
/// not an approval without a question, and no review answers it yet.
///
/// # Errors
/// [`Error::Io`] if the store can't be listed.
pub fn pending(store: &Store, now: i64) -> Result<Vec<Pending>> {
    let (rows, _) = store.list()?;
    let mut out = Vec::new();
    for row in rows
        .iter()
        .filter(|r| matches!(r.state, PrState::Open | PrState::Draft))
    {
        let Ok(id) = RecapId::new(&row.id) else {
            continue;
        };
        let recap_dir = store.recap_dir(&id);
        let author = store
            .load(&id)
            .ok()
            .and_then(|r| r.manifest.author)
            .unwrap_or_default();
        for t in load(&recap_dir).threads {
            if need(&recap_dir, &t, &author, now) != Need::Review {
                continue;
            }
            if let Some(last) = t.last_counted() {
                out.push(Pending {
                    recap: row.id.clone(),
                    pr: row.url.clone(),
                    thread: t.id.clone(),
                    place: t.place(),
                    author: last.author.clone(),
                    comment: last.id.clone(),
                });
            }
        }
    }
    Ok(out)
}
