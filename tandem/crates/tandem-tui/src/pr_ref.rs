//! A PR as people write it: a GitHub URL or `owner/repo#n`.

use tandem_core::github::RepoRef;

/// A pull request reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrRef {
    pub repo: RepoRef,
    pub number: u64,
}

impl PrRef {
    /// Parses `https://github.com/o/r/pull/7[/files…]`, `github.com/o/r/pull/7`
    /// or `o/r#7`. Whitespace around it is ignored.
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if let Some((repo, number)) = s.split_once('#') {
            return Self::build(repo, number);
        }
        let path = s
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .strip_prefix("github.com/")?;
        let mut parts = path.split('/');
        let owner = parts.next()?;
        let name = parts.next()?;
        if parts.next()? != "pull" {
            return None;
        }
        Self::build(&format!("{owner}/{name}"), parts.next()?)
    }

    fn build(repo: &str, number: &str) -> Option<Self> {
        let number: u64 = number.parse().ok().filter(|n| *n > 0)?;
        Some(Self {
            repo: RepoRef::parse(repo)?,
            number,
        })
    }

    /// `owner/repo#n`.
    pub fn label(&self) -> String {
        format!("{}#{}", self.repo.slug(), self.number)
    }

    pub fn url(&self) -> String {
        format!(
            "https://github.com/{}/pull/{}",
            self.repo.slug(),
            self.number
        )
    }
}

#[cfg(test)]
// Tests panic on a broken fixture; that is the failure mode we want.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls_and_short_refs() {
        let want = PrRef::parse("o/r#7").unwrap();
        assert_eq!(want.label(), "o/r#7");
        for s in [
            "https://github.com/o/r/pull/7",
            "https://github.com/o/r/pull/7/files",
            " github.com/o/r/pull/7 ",
        ] {
            assert_eq!(PrRef::parse(s).unwrap(), want, "{s}");
        }
        assert_eq!(want.url(), "https://github.com/o/r/pull/7");
    }

    #[test]
    fn rejects_everything_else() {
        for s in [
            "",
            "o/r",
            "o/r#0",
            "o/r#x",
            "https://github.com/o/r/issues/7",
            "https://example.com/o/r/pull/7",
        ] {
            assert!(PrRef::parse(s).is_none(), "{s}");
        }
    }
}
