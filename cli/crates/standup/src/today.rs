//! The local calendar date and timestamp, for scaffolding.
//!
//! Uses the local UTC offset when the platform can report it (call before
//! spawning threads — `time` refuses to read it from a multi-threaded process
//! on some Unixes) and falls back to UTC.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::model::ReportDate;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

/// Today's date in the local zone.
#[must_use]
pub fn today() -> ReportDate {
    ReportDate::from_date(now().date())
}

/// Now as an RFC 3339 timestamp (empty only if formatting fails, which it
/// can't for a real clock reading).
#[must_use]
pub fn now_rfc3339() -> String {
    now()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| now())
        .format(&Rfc3339)
        .unwrap_or_default()
}
