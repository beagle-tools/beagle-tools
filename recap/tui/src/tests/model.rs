//! Tests for `model`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn pr_refs_parse_from_urls_and_short_form() {
    let a = PrRef::parse("https://github.com/acme/widgets/pull/1437").unwrap();
    assert_eq!(a.full_repo(), "acme/widgets");
    assert_eq!(a.number, 1437);
    let b = PrRef::parse("https://github.com/o/r/pull/9/files?x=1").unwrap();
    assert_eq!(b.number, 9);
    let c = PrRef::parse("acme/gadgets#1590").unwrap();
    assert_eq!(c.to_string(), "acme/gadgets#1590");
    for bad in [
        "",
        "1437",
        "o/r",
        "https://github.com/o/r/issues/3",
        "o/r#x",
        "o/r#0",
        "../x/r#1",
    ] {
        assert!(PrRef::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn ids_are_derived_and_validated() {
    let pr = PrRef::parse("acme/Big_Edge.Node#685").unwrap();
    assert_eq!(RecapId::for_pr(&pr).as_str(), "big-edge-node-685");
    assert!(RecapId::new("widgets-42").is_ok());
    assert!(RecapId::new("Bad").is_err());
    assert!(RecapId::new("../x").is_err());
}

#[test]
fn tickets_come_from_title_prefixes() {
    assert_eq!(
        ticket_from_title("ABC-753: Give health probes a 5s timeout").as_deref(),
        Some("ABC-753")
    );
    assert_eq!(
        ticket_from_title("OPS-4177 Admit Go spans").as_deref(),
        Some("OPS-4177")
    );
    assert_eq!(ticket_from_title("fix: something"), None);
    assert_eq!(ticket_from_title("abc-1: lower"), None);
}

#[test]
fn states_come_from_github_fields_and_order_the_list() {
    assert_eq!(PrState::from_github("OPEN", true), Some(PrState::Draft));
    assert_eq!(PrState::from_github("OPEN", false), Some(PrState::Open));
    assert_eq!(PrState::from_github("MERGED", false), Some(PrState::Merged));
    assert_eq!(PrState::from_github("CLOSED", true), Some(PrState::Closed));
    assert_eq!(
        PrState::from_github("open", false),
        None,
        "GitHub's own spelling only"
    );
    let mut v = vec![
        PrState::Closed,
        PrState::Merged,
        PrState::Open,
        PrState::Draft,
    ];
    v.sort();
    assert_eq!(v, PrState::ALL);
}

#[test]
fn manifests_are_strict_about_state() {
    let ok = "repo = \"o/r\"\nnumber = 1\ntitle = \"t\"\nurl = \"u\"\nstate = \"draft\"";
    let m: RecapManifest = toml::from_str(ok).unwrap();
    assert_eq!(m.state, PrState::Draft);
    assert!(toml::to_string(&m).unwrap().contains("state = \"draft\""));
    assert!(toml::from_str::<RecapManifest>(&ok.replace("draft", "DRAFT")).is_err());
    assert!(toml::from_str::<RecapManifest>(&ok.replace("draft", "wip")).is_err());
    assert!(toml::from_str::<RecapManifest>(&format!("{ok}\nbogus = 1")).is_err());
}

#[test]
fn unix_seconds_parses_github_timestamps() {
    assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(unix_seconds("2026-10-27T18:44:40Z"), Some(1_793_126_680));
    assert_eq!(
        unix_seconds("2026-10-27T18:44:40.123Z"),
        Some(1_793_126_680)
    );
    assert_eq!(
        unix_seconds("2026-10-27T14:44:40-04:00"),
        Some(1_793_126_680)
    );
    assert_eq!(
        unix_seconds("2024-02-29T00:00:00Z").unwrap()
            - unix_seconds("2024-02-28T00:00:00Z").unwrap(),
        86_400
    );
    for bad in [
        "",
        "2026-10-27",
        "2026-13-01T00:00:00Z",
        "yesterday",
        "2026-10-27T25:00:00Z",
    ] {
        assert_eq!(unix_seconds(bad), None, "{bad}");
    }
}
