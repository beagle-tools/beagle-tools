//! Per-connection limits: how many connections may be open at once, how
//! long each may live, and a log of the ones the server gives up on.
//!
//! A connection slot is held by a [`Slot`] guard and freed when it drops,
//! and every socket read and write runs against one [`Deadline`] for the
//! whole connection, so no client (idle, half-open, or trickling a byte at
//! a time) can hold a slot longer than [`Limits::deadline`]. Without that
//! bound, a few dozen such clients used to fill every slot and the server
//! closed every new connection without a word.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The server's connection limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Connections served at once; more are closed straight away.
    pub max: usize,
    /// How long one connection may live, start to finish.
    pub deadline: Duration,
    /// How long a single read or write may wait.
    pub io_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max: 32,
            deadline: Duration::from_secs(30),
            io_timeout: Duration::from_secs(15),
        }
    }
}

/// The count of open connections, shared by the listener (which takes
/// slots) and `/v1/health` (which reports it).
#[derive(Debug, Clone)]
pub struct Connections {
    active: Arc<AtomicUsize>,
    max: usize,
}

impl Connections {
    /// No connections yet, at most `max` at once.
    #[must_use]
    pub fn new(max: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }

    /// Connections open right now.
    #[must_use]
    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    /// The most allowed at once.
    #[must_use]
    pub const fn max(&self) -> usize {
        self.max
    }

    /// A slot for one more connection, or `None` when all are taken.
    #[must_use]
    pub fn acquire(&self) -> Option<Slot> {
        self.active
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < self.max).then_some(n + 1)
            })
            .ok()
            .map(|_| Slot(Arc::clone(&self.active)))
    }
}

/// One connection's slot; dropping it (however the connection ends,
/// panics included) frees the slot.
#[derive(Debug)]
pub struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A socket whose timeouts can be set before each read or write.
pub trait Timeouts {
    /// Sets the read timeout.
    ///
    /// # Errors
    /// As the socket's own call.
    fn read_timeout(&self, timeout: Duration) -> io::Result<()>;
    /// Sets the write timeout.
    ///
    /// # Errors
    /// As the socket's own call.
    fn write_timeout(&self, timeout: Duration) -> io::Result<()>;
}

impl Timeouts for TcpStream {
    fn read_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }
    fn write_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.set_write_timeout(Some(timeout))
    }
}

/// A stream with one deadline for the whole connection: each read or
/// write may wait at most the I/O timeout, and never past the deadline.
#[derive(Debug)]
pub struct Deadline<S> {
    inner: S,
    until: Instant,
    io_timeout: Duration,
    expired: bool,
}

impl<S> Deadline<S> {
    /// Whether the connection ran out of time.
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.expired
    }
}

impl<S: Timeouts> Deadline<S> {
    /// Wraps `inner`, which must finish by `limits.deadline` from now.
    pub fn new(inner: S, limits: Limits) -> Self {
        Self {
            inner,
            until: Instant::now() + limits.deadline,
            io_timeout: limits.io_timeout,
            expired: false,
        }
    }

    /// The time the next read or write may wait, or an error once the
    /// deadline has passed.
    fn budget(&mut self) -> io::Result<Duration> {
        let left = self.until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            self.expired = true;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection deadline passed",
            ));
        }
        Ok(left.min(self.io_timeout))
    }

    /// Notes a timed-out read or write that used up the deadline.
    fn check<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        if result.as_ref().is_err_and(|e| {
            matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            )
        }) && Instant::now() >= self.until
        {
            self.expired = true;
        }
        result
    }
}

impl<S: Timeouts + Read> Read for Deadline<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let budget = self.budget()?;
        self.inner.read_timeout(budget)?;
        let result = self.inner.read(buf);
        self.check(result)
    }
}

impl<S: Timeouts + Write> Write for Deadline<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let budget = self.budget()?;
        self.inner.write_timeout(budget)?;
        let result = self.inner.write(buf);
        self.check(result)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// How often dropped connections may be logged: the first one always is,
/// then at most one line per interval, counting the ones in between.
pub const DROP_LOG_EVERY: Duration = Duration::from_secs(10);

/// A rate-limited log of connections closed without an answer.
#[derive(Debug, Default)]
pub struct Drops(Mutex<DropState>);

#[derive(Debug, Default)]
struct DropState {
    last: Option<Instant>,
    unlogged: u64,
}

impl Drops {
    /// Records one dropped connection at `now`; returns the line to log,
    /// or `None` when one was logged less than [`DROP_LOG_EVERY`] ago.
    pub fn record(
        &self,
        now: Instant,
        peer: &str,
        reason: &str,
        conns: &Connections,
    ) -> Option<String> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .last
            .is_some_and(|last| now.duration_since(last) < DROP_LOG_EVERY)
        {
            state.unlogged += 1;
            return None;
        }
        let more = std::mem::take(&mut state.unlogged);
        state.last = Some(now);
        let more = if more == 0 {
            String::new()
        } else {
            format!(", {more} more dropped since the last line")
        };
        Some(format!(
            "{peer} dropped: {reason} ({}/{} connections open{more})",
            conns.active(),
            conns.max()
        ))
    }
}

#[cfg(test)]
#[path = "tests/conn.rs"]
mod tests;
