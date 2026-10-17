//! The advisory per-extension install lock (`locks/<ext>.lock`).
//!
//! An OS file lock, so it is released when the holder exits — even when it
//! is killed mid-install — and a stale lock file never blocks anyone.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// How long a second `beagle` waits for the lock before giving up.
pub const WAIT: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_millis(100);

/// Held for the duration of an install; dropping it releases the lock.
#[derive(Debug)]
pub struct InstallLock {
    _file: File,
}

/// Takes the lock at `path` for `name`, waiting up to `wait` for another
/// holder. Fails with [`Error::Locked`] after that.
pub fn acquire(path: &Path, name: &str, wait: Duration) -> Result<InstallLock> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| Error::io(path, e))?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(InstallLock { _file: file }),
            Err(TryLockError::WouldBlock) if started.elapsed() < wait => thread::sleep(RETRY),
            Err(TryLockError::WouldBlock) => {
                return Err(Error::Locked {
                    name: name.to_owned(),
                })
            }
            Err(TryLockError::Error(e)) => return Err(Error::io(path, e)),
        }
    }
}
