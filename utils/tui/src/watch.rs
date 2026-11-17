//! Live reload: a recursive filesystem watcher that reports "something
//! changed" over a channel, drained once per frame so a burst of writes
//! becomes one reload.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

/// Watches `dir` recursively. Keep the returned watcher alive for as long
/// as events are wanted; [`drain`] the receiver once per frame.
///
/// # Errors
/// [`notify::Error`] when the watcher can't be created or attached.
pub fn watch_dir(dir: &Path) -> notify::Result<(RecommendedWatcher, Receiver<()>)> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            // A send error means the receiver is gone: we are shutting down.
            let _ = tx.send(());
        }
    })?;
    watcher.watch(dir, RecursiveMode::Recursive)?;
    Ok((watcher, rx))
}

/// Empties the channel; `true` if anything arrived.
pub fn drain(rx: &Receiver<()>) -> bool {
    let mut any = false;
    loop {
        match rx.try_recv() {
            Ok(()) => any = true,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
        }
    }
}
