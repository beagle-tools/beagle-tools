// Tests fail loudly on setup errors; unwrap/expect is the clearest way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use time::macros::datetime;

fn blocked() -> Event {
    Event::Blocked {
        task: "03-open-pr · Open the PR".into(),
        reason: Some("Push branch fix-retry and open a PR?".into()),
    }
}

fn sample() -> Record {
    record(
        ("rate-limit-the-api", "Rate-limit the public API"),
        Some(("03-open-pr", "Open the PR")),
        &blocked(),
        datetime!(2026-11-25 18:51:21.5 UTC),
    )
}

#[test]
fn record_carries_the_banner_text_and_a_link() {
    let r = sample();
    assert_eq!(r.v, 1);
    assert_eq!(r.tool, "scout");
    assert_eq!(r.kind, "blocked");
    assert_eq!(r.time, "2026-11-25T18:51:21Z");
    assert_eq!(r.title, "Scout · rate-limit-the-api");
    assert_eq!(r.subtitle, "■ Needs you: 03-open-pr · Open the PR");
    assert_eq!(r.body, "Push branch fix-retry and open a PR?");
    assert_eq!(r.task.as_deref(), Some("03-open-pr"));
    assert_eq!(
        r.link,
        "beagle://scout/projects/rate-limit-the-api/tasks/03-open-pr"
    );
    assert_eq!(r.id.len(), 36);
    assert_ne!(r.id, sample().id);
}

#[test]
fn project_events_link_to_the_project() {
    let r = record(
        ("p", "T"),
        None,
        &Event::ProjectDone { tasks: 3 },
        OffsetDateTime::UNIX_EPOCH,
    );
    assert_eq!(r.kind, "project_done");
    assert_eq!(r.task, None);
    assert_eq!(r.link, "beagle://scout/projects/p");
}

#[test]
fn the_body_is_capped() {
    let r = record(
        ("p", "T"),
        Some(("01-a", "A")),
        &Event::Blocked {
            task: "01-a · A".into(),
            reason: Some("x".repeat(5000)),
        },
        OffsetDateTime::UNIX_EPOCH,
    );
    assert_eq!(r.body.chars().count(), MAX_BODY_CHARS);
}

#[test]
fn catalog_defaults_match_scouts_own() {
    let defaults = NotifyEvents::default();
    for k in CATALOG {
        assert_eq!(
            default_for(&defaults, k.kind),
            Some(k.default),
            "{}",
            k.kind
        );
        assert_eq!(k.sound, k.kind == "blocked");
    }
    assert_eq!(default_for(&defaults, "nope"), None);
}

#[test]
fn every_event_kind_is_in_the_catalog() {
    let events = [
        Event::ProjectStarted {
            first_task: "01".into(),
        },
        Event::ProjectDone { tasks: 1 },
        blocked(),
        Event::TaskDone {
            task: "01".into(),
            progress: (1, 2),
            next: None,
        },
        Event::TaskStarted { task: "01".into() },
        Event::TaskPaused {
            task: "01".into(),
            quiet: "30m".into(),
        },
    ];
    for e in &events {
        assert!(CATALOG.iter().any(|k| k.kind == kind(e)), "{}", kind(e));
    }
}

#[test]
fn append_writes_lines_and_rotates_a_full_spool() {
    let dir = tempfile::tempdir().unwrap();
    append(dir.path(), &sample()).unwrap();
    append(dir.path(), &sample()).unwrap();
    let path = spool(dir.path(), "scout");
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<Record> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].kind, "blocked");

    let full = "x".repeat(usize::try_from(MAX_SPOOL_BYTES).unwrap());
    std::fs::write(&path, full).unwrap();
    append(dir.path(), &sample()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
    assert!(dir.path().join("scout.jsonl.1").is_file());
}
