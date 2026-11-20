//! Tests for `prs`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn short_labels_extract_pr_numbers() {
    assert_eq!(short_label("https://github.com/org/repo/pull/123"), "#123");
    assert_eq!(
        short_label("https://github.com/org/repo/pull/9/files"),
        "#9"
    );
    let plain = short_label("https://gitlab.example.com/mrs/42-long-url-goes-here");
    assert!(plain.starts_with("gitlab.example.com"), "{plain}");
}

#[test]
fn glyphs_are_one_cell_wide() {
    for state in PrState::ALL {
        assert_eq!(glyph(state).chars().count(), 1);
    }
}
