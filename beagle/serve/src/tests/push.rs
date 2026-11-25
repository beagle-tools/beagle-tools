// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use beagle_scout::events::{self, Record};
use beagle_scout::notify::Event;
use serde_json::json;
use time::Duration;

use super::devices::{Device, Registration};
use super::follow::{Follower, MAX_AGE};
use super::*;

fn blocked_record(at: OffsetDateTime) -> Record {
    events::record(
        ("rate-limit-the-api", "Rate-limit the public API"),
        Some(("03-open-pr", "Open the PR")),
        &Event::Blocked {
            task: "03-open-pr · Open the PR".into(),
            reason: Some("Push branch fix-retry and open a PR?".into()),
        },
        at,
    )
}

fn task_done_record(at: OffsetDateTime) -> Record {
    events::record(
        ("p", "T"),
        Some(("01-a", "A")),
        &Event::TaskDone {
            task: "01-a · A".into(),
            progress: (1, 2),
            next: None,
        },
        at,
    )
}

fn phone(settings: settings::Settings) -> Device {
    Device {
        apns_token: Some("ab".repeat(32)),
        environment: Environment::Development,
        topic: "dev.beagletools.beagle".into(),
        name: "iPhone".into(),
        registered: "2026-11-25T00:00:00Z".into(),
        last_seen: "2026-11-25T00:00:00Z".into(),
        settings,
    }
}

fn follower(dir: &Path) -> Follower {
    Follower::new(
        dir.join("events"),
        dir.join("cursor.json"),
        vec!["scout".into()],
    )
}

#[derive(Default)]
struct Recorder(Vec<Pending>);

impl Sender for Recorder {
    fn send(&mut self, pending: &Pending) {
        self.0.push(pending.clone());
    }
}

#[test]
fn defaults_mirror_scouts_own() {
    let d = settings::defaults();
    let scout = &d["scout"];
    assert!(scout.enabled);
    let on: Vec<&str> = scout
        .events
        .iter()
        .filter(|(_, on)| **on)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        on,
        ["blocked", "project_done", "project_started", "task_paused"]
    );
    assert!(settings::allows(&d, "scout", "blocked"));
    assert!(!settings::allows(&d, "scout", "task_done"));
    assert!(!settings::allows(&d, "scout", "nope"));
    assert!(!settings::allows(&d, "rca", "blocked"));
}

#[test]
fn settings_are_checked_and_completed() {
    let s = settings::parse(&json!({"scout": {"enabled": true, "events": {"task_done": true}}}))
        .unwrap();
    assert!(s["scout"].events["task_done"]);
    assert!(
        s["scout"].events["blocked"],
        "missing kinds keep their default"
    );
    assert_eq!(settings::parse(&json!({})).unwrap(), settings::defaults());
    for bad in [
        json!({"rca": {"enabled": true}}),
        json!({"scout": {"enabled": true, "events": {"nope": true}}}),
        json!({"scout": {"enabled": "yes"}}),
        json!({"scout": {"enabled": true, "extra": 1}}),
        json!([1]),
    ] {
        assert!(settings::parse(&bad).is_err(), "{bad}");
    }
    let off = settings::parse(&json!({"scout": {"enabled": false}})).unwrap();
    assert!(!settings::allows(&off, "scout", "blocked"));
}

#[test]
fn registrations_are_validated() {
    let good = Registration {
        apns_token: "ab".repeat(32),
        environment: Environment::Production,
        topic: "dev.beagletools.beagle".into(),
        name: "iPhone".into(),
    };
    assert!(good.validate().is_ok());
    let cases = [
        Registration {
            apns_token: "zz".repeat(32),
            ..good.clone()
        },
        Registration {
            apns_token: "ab".into(),
            ..good.clone()
        },
        Registration {
            topic: "nodots".into(),
            ..good.clone()
        },
        Registration {
            topic: "a..b".into(),
            ..good.clone()
        },
        Registration {
            name: String::new(),
            ..good.clone()
        },
        Registration {
            name: "x".repeat(65),
            ..good.clone()
        },
        Registration {
            name: "bad\nname".into(),
            ..good.clone()
        },
    ];
    for bad in cases {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    assert_eq!(
        devices::device_id("9B1DEB4D-3B7D-4BAD-9BDD-2B0D7B3DCB6D").as_deref(),
        Some("9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d")
    );
    assert!(devices::device_id("../x").is_none());
}

#[test]
fn the_follower_starts_at_the_end_then_reads_new_whole_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("events");
    let now = OffsetDateTime::now_utc();
    events::append(&dir, &blocked_record(now)).unwrap();
    let mut f = follower(tmp.path());
    assert!(f.poll(now).0.is_empty(), "history is never replayed");

    events::append(&dir, &task_done_record(now)).unwrap();
    // A half-written line waits for the next poll.
    let spool = events::spool(&dir, "scout");
    let mut text = std::fs::read_to_string(&spool).unwrap();
    text.push_str("{\"v\":1,");
    std::fs::write(&spool, &text).unwrap();
    let (got, problems) = f.poll(now);
    assert_eq!(
        got.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
        ["task_done"]
    );
    assert_eq!(problems, Vec::<String>::new());

    // A new follower picks up from the saved cursor.
    text.truncate(text.len() - "{\"v\":1,".len());
    std::fs::write(&spool, &text).unwrap();
    events::append(&dir, &blocked_record(now)).unwrap();
    let (got, _) = follower(tmp.path()).poll(now);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind, "blocked");
}

#[test]
fn the_follower_finishes_a_rotated_spool_and_drops_stale_events() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("events");
    let now = OffsetDateTime::now_utc();
    events::append(&dir, &blocked_record(now)).unwrap();
    let mut f = follower(tmp.path());
    f.poll(now);
    events::append(&dir, &task_done_record(now)).unwrap();
    let spool = events::spool(&dir, "scout");
    std::fs::rename(&spool, spool.with_extension("jsonl.1")).unwrap();
    events::append(&dir, &blocked_record(now)).unwrap();
    events::append(&dir, &blocked_record(now - MAX_AGE - Duration::seconds(1))).unwrap();
    let (got, _) = f.poll(now);
    assert_eq!(
        got.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
        ["task_done", "blocked"],
        "the old file's rest, then the new file, without the stale event"
    );
}

#[test]
fn a_scout_status_change_becomes_a_pending_notification_for_a_matching_phone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("events");
    let store = DeviceStore::new(tmp.path().join("devices.json"));
    let mut quiet = settings::defaults();
    quiet.get_mut("scout").unwrap().enabled = false;
    store
        .update(|d| {
            d.devices.insert(
                "aaaaaaaa-0000-0000-0000-000000000001".into(),
                phone(settings::defaults()),
            );
            d.devices
                .insert("aaaaaaaa-0000-0000-0000-000000000002".into(), phone(quiet));
            let mut unregistered = phone(settings::defaults());
            unregistered.apns_token = None;
            d.devices
                .insert("aaaaaaaa-0000-0000-0000-000000000003".into(), unregistered);
        })
        .unwrap();
    let mut f = follower(tmp.path());
    let mut sender = Recorder::default();
    assert_eq!(tick(&mut f, &store, &mut sender), 0);

    // What `beagle scout task block` writes, and a task_done no phone wants.
    let now = OffsetDateTime::now_utc();
    events::append(&dir, &blocked_record(now)).unwrap();
    events::append(&dir, &task_done_record(now)).unwrap();
    assert_eq!(tick(&mut f, &store, &mut sender), 1);
    let p = &sender.0[0];
    assert_eq!(p.device, "aaaaaaaa-0000-0000-0000-000000000001");
    assert_eq!(p.apns_token, "ab".repeat(32));
    assert_eq!(p.topic, "dev.beagletools.beagle");
    assert_eq!(p.record.kind, "blocked");
    assert_eq!(p.record.subtitle, "■ Needs you: 03-open-pr · Open the PR");
    assert_eq!(
        p.record.link,
        "beagle://scout/projects/rate-limit-the-api/tasks/03-open-pr"
    );
    assert_eq!(
        tick(&mut f, &store, &mut sender),
        0,
        "each event is sent once"
    );
}
