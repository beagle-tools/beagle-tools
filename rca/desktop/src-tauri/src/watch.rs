//! Live reload: a `notify` watcher on the store's `rcas/` directory that
//! tells the frontend "something changed" on the `workspaces-changed`
//! event, so the open view re-renders while an agent writes the workspace
//! (as the TUI does).
//!
//! Editors and agents write in bursts, so events are debounced: the first
//! change starts a window that every further change extends, and one event
//! goes out once the store has been quiet for [`QUIET`].

use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher as _};
use tauri::{AppHandle, Emitter};

/// The event name the frontend listens on.
const EVENT: &str = "workspaces-changed";
/// How long the store must be quiet before one event goes out.
const QUIET: Duration = Duration::from_millis(250);

/// Starts the watcher thread for the app's lifetime. A store that can't be
/// opened or watched leaves the app working without live reload (the
/// manual reload paths still work), with the reason on stderr.
pub(crate) fn spawn_store_watcher(app: AppHandle) {
    thread::spawn(move || {
        if let Err(message) = watch(&app) {
            eprintln!("beagle desktop: live reload is off: {message}");
        }
    });
}

fn watch(app: &AppHandle) -> Result<(), String> {
    let store = crate::commands::open_store().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel::<()>();
    // The callback only signals; all work happens on this thread.
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            let _ = tx.send(()); // receiver gone means the app is shutting down
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(store.watch_root(), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    while next_burst(&rx, QUIET) {
        // An emit only fails once the app is gone.
        if app.emit(EVENT, ()).is_err() {
            break;
        }
    }
    Ok(())
}

/// Blocks until a change arrives, then waits until `quiet` passes with no
/// further change. `true` once a burst has settled; `false` when the
/// watcher is gone (the channel closed) and no burst is pending.
pub(crate) fn next_burst(rx: &Receiver<()>, quiet: Duration) -> bool {
    if rx.recv().is_err() {
        return false;
    }
    loop {
        match rx.recv_timeout(quiet) {
            Ok(()) => {}
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return true,
        }
    }
}

#[cfg(test)]
#[path = "tests/watch.rs"]
mod tests;
