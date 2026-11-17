//! Tests for `ui::doc`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn tldr_takes_the_first_line_under_the_heading() {
    let s = "## TL;DR\n\nThe gateway **drops** `keepalive` conns.\nmore\n## Next\n";
    assert_eq!(
        tldr(s).as_deref(),
        Some("The gateway drops keepalive conns.")
    );
    assert_eq!(tldr("## TL;DR\n\n## Next\nx").as_deref(), None);
    assert_eq!(tldr("no heading"), None);
}
