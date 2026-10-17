//! Pulling URLs back out of the files agents write.
//!
//! A stage reports its PR by writing the URL into `result.md` or its
//! write-up, in prose, for a human. Three things then need to find it
//! again: retention (keep the write-up while the PR is open), agenttop
//! and Agentdesk (offer the link), and they must agree — a PR that the
//! sweeper can see but the UI cannot is a write-up that vanishes with no
//! way to have reached it.
//!
//! This lived in two places and had already drifted: one trimmed a
//! trailing `>` and the other did not, so `<https://…/pull/9>` resolved
//! to a different URL depending on who asked.

use std::path::Path;

/// Punctuation that is part of the prose, not the URL. Markdown wraps
/// links in `<>`, `[]`, backticks and parens, and a sentence ends with
/// `.` or `,` — none of which belong in the href.
const TRAILING: [char; 8] = [')', ',', '.', '`', '>', ']', ';', '!'];

/// The first `https://` URL containing `needle`, trimmed of trailing
/// prose punctuation.
///
/// The URL is found WITHIN the word rather than at the start of it,
/// because `[#1333](https://…/pull/1333)` is how a model asked to "link
/// the PR" most often writes one, and splitting on whitespace there
/// yields `PR](https://…`. Both earlier copies of this required the
/// word to begin with `https://` and so missed every markdown inline
/// link — which, for retention, meant deleting a write-up whose PR was
/// still open.
pub fn first_url(body: &str, needle: &str) -> Option<String> {
    body.split_whitespace()
        .filter_map(|word| word.find("https://").map(|at| &word[at..]))
        .find(|url| url.contains(needle))
        .map(|url| url.trim_end_matches(TRAILING).to_owned())
}

/// [`first_url`] against a file. A missing or unreadable file is `None`:
/// every caller is asking "is there a link here", and an unreadable file
/// answers that question the same way an absent one does.
pub fn first_url_in_file(path: &Path, needle: &str) -> Option<String> {
    first_url(&std::fs::read_to_string(path).ok()?, needle)
}

/// The first GitHub pull-request URL in a file.
pub fn pr_url_in_file(path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    first_url(&body, "/pull/").filter(|u| u.starts_with("https://github.com/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PR: &str = "https://github.com/superblocksteam/dedicated-deployment/pull/1333";

    #[test]
    fn trailing_prose_punctuation_is_not_part_of_the_url() {
        for wrapped in [
            format!("- **PR:** {PR} (draft)\n"),
            format!("see {PR}.\n"),
            format!("opened {PR}, then tested\n"),
            format!("`{PR}`\n"),
            format!("<{PR}>\n"),
            format!("[the PR]({PR})\n"),
        ] {
            assert_eq!(
                first_url(&wrapped, "/pull/").as_deref(),
                Some(PR),
                "{wrapped:?}"
            );
        }
    }

    #[test]
    fn the_first_match_wins() {
        let body = format!("{PR}\nand later https://github.com/x/y/pull/2\n");
        assert_eq!(first_url(&body, "/pull/").as_deref(), Some(PR));
    }

    #[test]
    fn a_url_that_does_not_match_the_needle_is_ignored() {
        let body = "https://github.com/superblocksteam/dedicated-deployment/issues/4\n";
        assert_eq!(first_url(body, "/pull/"), None);
    }

    #[test]
    fn a_markdown_inline_link_is_found() {
        // The shape a model actually writes when told to "link the PR".
        let body = format!("**Draft PR:** [#1333]({PR})\n");
        assert_eq!(first_url(&body, "/pull/").as_deref(), Some(PR));
    }

    #[test]
    fn http_is_not_matched() {
        // Only https. A plain-http github URL is a typo, not a PR link.
        assert_eq!(first_url("http://github.com/a/b/pull/1", "/pull/"), None);
    }

    #[test]
    fn no_url_is_none_rather_than_empty() {
        assert_eq!(first_url("no links here at all\n", "/pull/"), None);
    }
}
