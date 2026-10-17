//! Tests for version parsing, ordering and nearest-version suggestions.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

const RCA: &str = "beagle-rca-v";

fn v(s: &str) -> Version {
    s.parse().expect("valid test version")
}

#[test]
fn parse_for_accepts_bare_v_prefixed_and_full_tag() {
    assert_eq!(Version::parse_for("0.1.0", RCA).unwrap(), v("0.1.0"));
    assert_eq!(Version::parse_for("v0.1.0", RCA).unwrap(), v("0.1.0"));
    assert_eq!(
        Version::parse_for("beagle-rca-v0.1.0", RCA).unwrap(),
        v("0.1.0")
    );
}

#[test]
fn parse_for_rejects_another_apps_tag() {
    let err = Version::parse_for("beagle-agentd-v0.1.0", RCA).unwrap_err();
    assert!(matches!(err, Error::InvalidVersion { .. }), "{err}");
}

#[test]
fn version_rejects_garbage() {
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "1.2.x",
        "v",
        "0.2.0-rc1",
        "1..2",
        "+1.2.3",
    ] {
        assert!(bad.parse::<Version>().is_err(), "`{bad}` must be rejected");
    }
}

#[test]
fn from_tag_filters_by_prefix() {
    assert_eq!(
        Version::from_tag("beagle-rca-v1.2.3", RCA),
        Some(v("1.2.3"))
    );
    assert_eq!(Version::from_tag("beagle-tandem-v1.2.3", RCA), None);
    assert_eq!(Version::from_tag("beagle-rca-v1.2.3-rc1", RCA), None);
    assert_eq!(v("1.2.3").tag(RCA), "beagle-rca-v1.2.3");
}

#[test]
fn versions_order_numerically() {
    assert!(v("0.10.0") > v("0.9.9"));
    assert!(v("1.0.0") > v("0.99.99"));
}

#[test]
fn nearest_takes_the_closest_five_in_order() {
    let known: Vec<Version> = (0..10).map(|p| Version::new(0, 1, p)).collect();
    let picked = nearest(v("0.1.5"), &known, 5);
    assert_eq!(picked.len(), 5);
    assert!(picked.contains(&v("0.1.5")) || picked.contains(&v("0.1.4")));
    assert!(picked.windows(2).all(|w| w[0] < w[1]), "ascending");
    assert_eq!(nearest(v("9.9.9"), &[], 5), []);
    assert_eq!(nearest(v("0.0.1"), &known[..2], 5).len(), 2);
}

#[test]
fn serde_round_trips_as_a_string() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Doc {
        v: Version,
    }
    let doc = Doc { v: v("1.2.3") };
    let text = toml::to_string(&doc).unwrap();
    assert_eq!(text.trim(), "v = \"1.2.3\"");
    assert_eq!(toml::from_str::<Doc>(&text).unwrap(), doc);
    assert!(toml::from_str::<Doc>("v = \"1.2\"").is_err());
}
