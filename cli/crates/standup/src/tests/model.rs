//! Tests for `model`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn report_dates_round_trip_and_reject_garbage() {
    let d = ReportDate::parse("2026-10-27").unwrap();
    assert_eq!(d.to_string(), "2026-10-27");
    assert_eq!(d.weekday(), "Tuesday");
    assert_eq!(d.short(), "Tue 10-27");
    assert!(ReportDate::parse("2026-02-30").is_err());
    assert!(ReportDate::parse("27/10/2026").is_err());
    assert!(ReportDate::parse("").is_err());
    assert!(ReportDate::parse("2026-10-26").unwrap() < d);
}

#[test]
fn ticket_ids_are_uppercased_and_filesystem_safe() {
    assert_eq!(TicketId::new("abc-123").unwrap().as_str(), "ABC-123");
    assert!(TicketId::new("../etc").is_err());
    assert!(TicketId::new("a b").is_err());
    assert!(TicketId::new("").is_err());
    assert!(TicketId::new(&"x".repeat(33)).is_err());
}

#[test]
fn manifests_reject_unknown_fields() {
    let ok: TicketManifest = toml::from_str("id = \"ABC-1\"\ntitle = \"t\"").unwrap();
    assert_eq!(ok.prs, Vec::<String>::new());
    assert!(toml::from_str::<TicketManifest>("id = \"ABC-1\"\ntitle = \"t\"\nbogus = 1").is_err());
    assert!(toml::from_str::<ReportManifest>("bogus = 1").is_err());
}

#[test]
fn states_bucket_sensibly() {
    assert_eq!(
        StateKind::classify(Some("In Progress")),
        StateKind::Progress
    );
    assert_eq!(StateKind::classify(Some("In Review")), StateKind::Review);
    assert_eq!(StateKind::classify(Some("Done")), StateKind::Done);
    assert_eq!(StateKind::classify(Some("Blocked")), StateKind::Blocked);
    assert_eq!(StateKind::classify(Some("Todo")), StateKind::Todo);
    assert_eq!(StateKind::classify(Some("Canceled")), StateKind::Other);
    assert_eq!(StateKind::classify(None), StateKind::Other);
}
