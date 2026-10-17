//! Search-backed PR retrieval: per-repo search, the account-wide inbox
//! scopes, and approved-by-me — all hydrated in full and augmented with
//! review status.

use tandem_core::github::{GithubReview, PullRequest, RepoRef, ReviewVerdict};
use tandem_core::Result;

use crate::client::GithubClient;
use crate::wire::{effective_reviews, WireReview, WireSearch};
use crate::write::urlenc;

impl GithubClient {
    /// Server-side search across ALL open PRs of a repo (title + body),
    /// so results aren't limited to the pages the client has loaded.
    /// Matches are fetched in full so they render like any listed PR.
    pub async fn search_open_prs(&self, repo: &RepoRef, query: &str) -> Result<Vec<PullRequest>> {
        let q = format!("repo:{}/{} is:pr is:open {query}", repo.owner, repo.name);
        let path = format!("/search/issues?q={}&per_page=30", urlenc(&q));
        let found: WireSearch = self.get_json(&path).await?;

        let mut set = tokio::task::JoinSet::new();
        for item in found.items {
            let client = self.clone();
            let repo = repo.clone();
            set.spawn(async move { client.pull_request_settled(&repo, item.number).await });
        }
        let mut prs = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Ok(pr)) = joined {
                prs.push(pr);
            }
        }
        prs.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
        Ok(prs)
    }

    /// Open PRs across ALL repos matching a global search query (e.g.
    /// `review-requested:@me`), hydrated in full, newest-activity first.
    pub async fn search_global_prs(&self, query: &str) -> Result<Vec<PullRequest>> {
        let q = urlenc(&format!("is:pr is:open {query}"));
        let path = format!("/search/issues?q={q}&per_page=30&sort=updated");
        let found: WireSearch = self.get_json(&path).await?;

        let mut set = tokio::task::JoinSet::new();
        for item in found.items {
            let Some(repo) = item
                .repository_url
                .as_deref()
                .and_then(|u| u.split_once("/repos/"))
                .and_then(|(_, slug)| RepoRef::parse(slug))
            else {
                continue;
            };
            let client = self.clone();
            set.spawn(async move { client.pull_request_settled(&repo, item.number).await });
        }
        let mut prs = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Ok(pr)) = joined {
                prs.push(pr);
            }
        }
        prs.sort_by_key(|p| std::cmp::Reverse(p.updated_at));

        // Same approved+green augmentation the repo list gets, once per
        // distinct repo in the result set.
        let mut repos: Vec<RepoRef> = prs.iter().map(|p| p.repo.clone()).collect();
        repos.sort_by_key(RepoRef::slug);
        repos.dedup();
        for repo in &repos {
            self.augment_review_status(repo, &mut prs).await;
        }
        Ok(prs)
    }

    /// Raw review events on one PR.
    async fn pr_reviews(&self, repo: &RepoRef, number: u64) -> Result<Vec<GithubReview>> {
        let raw: Vec<WireReview> = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}/reviews?per_page=100",
                repo.owner, repo.name
            ))
            .await?;
        Ok(raw.into_iter().map(Into::into).collect())
    }

    /// Open PRs the authenticated user has APPROVED that are still open
    /// (i.e. waiting on other reviewers or the author). Search can't
    /// express "approved by me", so candidates come from reviewed-by:@me
    /// and each one's reviews are checked for your effective approval.
    pub async fn approved_by_me(&self, repo: Option<&str>) -> Result<Vec<PullRequest>> {
        let viewer = self.viewer_login().await?;
        let query = match repo {
            Some(slug) => format!("reviewed-by:@me repo:{slug}"),
            None => "reviewed-by:@me".to_owned(),
        };
        let candidates = self.search_global_prs(&query).await?;

        let mut set = tokio::task::JoinSet::new();
        for pr in candidates {
            let client = self.clone();
            let viewer = viewer.clone();
            set.spawn(async move {
                let reviews = client.pr_reviews(&pr.repo, pr.number).await.ok()?;
                let approved = effective_reviews(reviews)
                    .into_iter()
                    .any(|r| r.author.login == viewer && r.verdict == ReviewVerdict::Approved);
                approved.then_some(pr)
            });
        }
        let mut prs = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(Some(pr)) = joined {
                prs.push(pr);
            }
        }
        prs.sort_by_key(|p| std::cmp::Reverse(p.updated_at));
        Ok(prs)
    }
}
