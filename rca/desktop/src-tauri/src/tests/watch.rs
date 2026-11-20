//! Tests for the live-reload debounce.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::next_burst;

const QUIET: Duration = Duration::from_millis(50);

#[test]
fn a_burst_of_changes_is_one_event() {
    let (tx, rx) = mpsc::channel();
    for _ in 0..20 {
        tx.send(()).ok();
    }
    assert!(next_burst(&rx, QUIET));
    // Everything in the burst was consumed: nothing is left for a second event.
    assert!(rx.try_recv().is_err());
}

#[test]
fn the_window_extends_while_changes_keep_coming() {
    let (tx, rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        for _ in 0..5 {
            tx.send(()).ok();
            thread::sleep(Duration::from_millis(20));
        }
        tx
    });
    let start = Instant::now();
    assert!(next_burst(&rx, QUIET));
    // Five writes 20 ms apart, then the quiet window.
    assert!(start.elapsed() >= Duration::from_millis(80));
    drop(writer.join());
}

#[test]
fn a_closed_watcher_ends_the_loop() {
    let (tx, rx) = mpsc::channel::<()>();
    drop(tx);
    assert!(!next_burst(&rx, QUIET));
}

#[test]
fn a_pending_burst_still_fires_when_the_watcher_closes() {
    let (tx, rx) = mpsc::channel();
    tx.send(()).ok();
    drop(tx);
    assert!(next_burst(&rx, QUIET));
    assert!(!next_burst(&rx, QUIET));
}
