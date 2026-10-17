//! Compact human-readable durations and ages.

use std::fmt::Write as _;

/// Compact multi-unit duration: real-time seconds under a minute (`45s`),
/// then minutes are the smallest unit with zero components omitted: `3m`,
/// `1h2m`, `2d3h20m`, `1d2m`. Leftover seconds past a minute are dropped.
///
/// Negative inputs (clock skew) clamp to `0s`.
pub fn human_duration(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        return format!("{secs}s");
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    let mut out = String::new();
    // Writing to a String cannot fail.
    if days > 0 {
        let _ = write!(out, "{days}d");
    }
    if hours > 0 {
        let _ = write!(out, "{hours}h");
    }
    if mins > 0 || out.is_empty() {
        let _ = write!(out, "{mins}m");
    }
    out
}

/// Single-unit age, kubectl style: the largest whole unit only — `45s`,
/// `3m`, `2h`, `5d` (rounded down).
///
/// Negative inputs (clock skew) clamp to `0s`.
pub fn age_single_unit(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        format!("{s}s")
    } else if s < 3_600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3_600)
    } else {
        format!("{}d", s / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Copied from workflows/crates/agenttop/src/format.rs.
    #[test]
    fn human_duration_agenttop_cases() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(45), "45s");
        assert_eq!(human_duration(60), "1m");
        assert_eq!(human_duration(192), "3m");
        assert_eq!(human_duration(3_600), "1h");
        assert_eq!(human_duration(3_720), "1h2m");
        assert_eq!(human_duration(2 * 86_400 + 3 * 3_600 + 20 * 60), "2d3h20m");
        assert_eq!(human_duration(86_400 + 120), "1d2m");
        assert_eq!(human_duration(-5), "0s");
    }

    // Copied from workflows/crates/agentctl/src/fmt.rs (format_duration).
    #[test]
    fn human_duration_agentctl_cases() {
        assert_eq!(human_duration(45), "45s");
        assert_eq!(human_duration(3 * 60 + 12), "3m");
        assert_eq!(human_duration(2 * 3_600 + 5 * 60), "2h5m");
        assert_eq!(human_duration(5 * 86_400 + 3_600), "5d1h");
        assert_eq!(human_duration(86_400 + 20 * 60), "1d20m");
    }

    // Copied from workflows/crates/agentctl/src/fmt.rs (format_age).
    #[test]
    fn age_single_unit_cases() {
        assert_eq!(age_single_unit(0), "0s");
        assert_eq!(age_single_unit(-5), "0s");
        assert_eq!(age_single_unit(45), "45s");
        assert_eq!(age_single_unit(3 * 60 + 20), "3m");
        assert_eq!(age_single_unit(2 * 3_600 + 59), "2h");
        assert_eq!(age_single_unit(5 * 86_400 + 3_600), "5d");
    }
}
