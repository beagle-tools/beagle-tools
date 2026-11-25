// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;

use super::*;

#[test]
fn slots_stop_at_the_limit_and_come_back_when_dropped() {
    let conns = Connections::new(2);
    let a = conns.acquire().unwrap();
    let b = conns.acquire().unwrap();
    assert!(conns.acquire().is_none());
    assert_eq!(conns.active(), 2);
    drop(a);
    assert_eq!(conns.active(), 1);
    let c = conns.acquire().unwrap();
    drop((b, c));
    assert_eq!(conns.active(), 0);
}

#[test]
fn a_slot_is_freed_even_when_its_thread_panics() {
    let conns = Connections::new(1);
    let slot = conns.acquire().unwrap();
    let joined = std::thread::spawn(move || {
        let _slot = slot;
        std::panic::resume_unwind(Box::new("boom"));
    })
    .join();
    assert!(joined.is_err());
    assert_eq!(conns.active(), 0);
}

/// A socket that records the timeouts it is given and reads nothing.
#[derive(Default)]
struct Fake {
    read: Cell<Option<Duration>>,
    write: Cell<Option<Duration>>,
}

impl Timeouts for Fake {
    fn read_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.read.set(Some(timeout));
        Ok(())
    }
    fn write_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.write.set(Some(timeout));
        Ok(())
    }
}

impl Read for Fake {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Write for Fake {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn each_read_and_write_waits_at_most_the_io_timeout() {
    let limits = Limits {
        max: 1,
        deadline: Duration::from_secs(60),
        io_timeout: Duration::from_secs(5),
    };
    let mut s = Deadline::new(Fake::default(), limits);
    assert_eq!(s.read(&mut [0; 4]).unwrap(), 0);
    s.write_all(b"x").unwrap();
    assert_eq!(s.inner.read.get(), Some(Duration::from_secs(5)));
    assert_eq!(s.inner.write.get(), Some(Duration::from_secs(5)));
    assert!(!s.expired());
}

#[test]
fn nothing_waits_past_the_deadline() {
    let limits = Limits {
        max: 1,
        deadline: Duration::from_millis(50),
        io_timeout: Duration::from_secs(5),
    };
    let mut s = Deadline::new(Fake::default(), limits);
    assert_eq!(s.read(&mut [0; 4]).unwrap(), 0);
    assert!(s.inner.read.get().unwrap() <= Duration::from_millis(50));
    std::thread::sleep(Duration::from_millis(60));
    let err = s.read(&mut [0; 4]).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    assert!(s.write(b"x").is_err());
    assert!(s.expired());
}

#[test]
fn drops_are_logged_with_a_reason_at_most_once_per_interval() {
    let conns = Connections::new(32);
    let _slot = conns.acquire().unwrap();
    let drops = Drops::default();
    let t0 = Instant::now();
    let first = drops.record(t0, "192.0.2.7", "busy", &conns).unwrap();
    assert_eq!(first, "192.0.2.7 dropped: busy (1/32 connections open)");
    for i in 1..=3 {
        let at = t0 + Duration::from_secs(i);
        assert!(drops.record(at, "192.0.2.7", "busy", &conns).is_none());
    }
    let next = drops
        .record(t0 + DROP_LOG_EVERY, "192.0.2.8", "client left", &conns)
        .unwrap();
    assert_eq!(
        next,
        "192.0.2.8 dropped: client left (1/32 connections open, 3 more dropped since the last line)"
    );
}
