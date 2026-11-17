//! Tests for launch settings (`ui::launch`): config → [`Settings`] → `App`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::tz::{Clock, TimeZone};
use crate::ui::testutil::app_with;

use super::*;

#[test]
fn absent_config_means_defaults() {
    let settings = Settings::from_config(None);
    assert!(!settings.notify);
    assert_eq!(settings.notify_events, crate::config::NotifyEvents::all());
    assert_eq!(settings.timezone, Ok(TimeZone::utc()));
    assert_eq!(settings.clock, Clock::H24);
}

#[test]
fn timezone_clock_and_fixing_event_flow_from_config_to_app() {
    let config = crate::config::parse(
        "timezone = \"America/New_York\"\nclock = \"12h\"\nnotify = true\n\
         [notify_events]\nfixing = true\n",
    )
    .expect("config with timezone, clock and fixing loads");
    let settings = Settings::from_config(Some(&config));
    assert!(settings.notify);
    assert!(settings.notify_events.fixing && !settings.notify_events.review);
    assert_eq!(settings.clock, Clock::H12);
    let eastern = TimeZone::parse("America/New_York").expect("zone");
    assert_eq!(settings.timezone.as_ref(), Ok(&eastern));

    let mut app = app_with(1);
    app.apply_settings(settings);
    assert_eq!(app.timezone, eastern);
    assert_eq!(app.clock, Clock::H12);
    assert!(app.notify_enabled && app.notify_events.fixing);
    assert!(app.status_line().is_none(), "a good zone says nothing");
}

#[test]
fn a_bad_timezone_falls_back_to_utc_with_a_status_note() {
    let config = crate::config::parse("timezone = \"Mars/Olympus\"\n").expect("parses");
    let settings = Settings::from_config(Some(&config));
    assert!(settings.timezone.is_err(), "zone is validated at launch");

    let mut app = app_with(1);
    app.apply_settings(settings);
    assert_eq!(app.timezone, TimeZone::utc());
    assert!(
        app.status_line()
            .is_some_and(|s| s.contains("Mars/Olympus") && s.contains("UTC")),
        "status line explains the fallback"
    );
}
