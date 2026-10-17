//! Async GitHub REST client.
//!
//! Reads are Tandem's bread and butter. The handful of write methods at
//! the bottom (post comment / reply / approve) exist ONLY for explicit
//! user actions in the UI — agents have no path to them.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use tandem_core::diff::FileDiff;
use tandem_core::diff_parse::parse_unified_diff;
use tandem_core::github::{GithubComment, PrDetail, PullRequest, RepoRef};
use tandem_core::{Result, TandemError};

use crate::auth::GithubConfig;
use crate::wire::{effective_reviews, WireCheckRunList, WireComment, WirePull, WireReview};

pub(crate) const JSON_ACCEPT: &str = "application/vnd.github+json";
const DIFF_ACCEPT: &str = "application/vnd.github.v3.diff";

/// REST/GraphQL client bound to one API base and token. Cheap to clone
/// (the HTTP pool is shared); reads live here, user-only writes in
/// `write.rs`.
#[derive(Debug, Clone)]
pub struct GithubClient {
    pub(crate) http: reqwest::Client,
    pub(crate) api_base: String,
    token: Option<String>,
}

impl GithubClient {
    /// Build a client, resolving the configured auth strategy up front so
    /// auth failures surface immediately (and loudly) rather than as
    /// mysterious 401s later.
    pub async fn connect(config: &GithubConfig) -> Result<Self> {
        let token = config.auth.resolve_token().await?;
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| TandemError::Config(format!("failed to build http client: {e}")))?;
        Ok(Self {
            http,
            api_base: config.api_base.trim_end_matches('/').to_owned(),
            token,
        })
    }

    pub(crate) fn headers(&self, accept: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, header_value(accept)?);
        headers.insert(USER_AGENT, header_value("tandem-code-review")?);
        headers.insert("X-GitHub-Api-Version", header_value("2022-11-28")?);
        if let Some(token) = &self.token {
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {token}"))?);
        }
        Ok(headers)
    }

    async fn get(&self, path: &str, accept: &str) -> Result<reqwest::Response> {
        let url = format!("{}{path}", self.api_base);
        let headers = self.headers(accept)?;
        tracing::debug!(%url, "github GET");
        self.send_with_rate_limit_retry(|| self.http.get(&url).headers(headers.clone()))
            .await
    }

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.get(path, JSON_ACCEPT).await?;
        resp.json::<T>().await.map_err(|e| TandemError::GithubApi {
            status: 0,
            message: format!("bad body: {e}"),
        })
    }

    /// Page size for PR listings; also how the UI decides "has more".
    pub const PR_PAGE_SIZE: usize = 30;

    /// One page of open PRs (most recently updated first), 1-indexed.
    ///
    /// The list endpoint omits `additions`/`deletions`/`changed_files`,
    /// so each PR is re-fetched individually (concurrently) to fill the
    /// size stats the sidebar shows.
    pub async fn list_pull_requests(&self, repo: &RepoRef, page: u32) -> Result<Vec<PullRequest>> {
        let path = format!(
            "/repos/{}/{}/pulls?state=open&sort=updated&direction=desc&per_page={}&page={}",
            repo.owner,
            repo.name,
            Self::PR_PAGE_SIZE,
            page.max(1)
        );
        let pulls: Vec<WirePull> = self.get_json(&path).await?;
        let mut prs: Vec<PullRequest> = pulls.into_iter().map(|p| p.into_domain(repo)).collect();

        let mut set = tokio::task::JoinSet::new();
        for (index, pr) in prs.iter().enumerate() {
            let client = self.clone();
            let owner = repo.owner.clone();
            let name = repo.name.clone();
            let number = pr.number;
            set.spawn(async move {
                let repo = RepoRef { owner, name };
                (index, client.pull_request_settled(&repo, number).await)
            });
        }
        while let Some(joined) = set.join_next().await {
            let Ok((index, Ok(detail))) = joined else {
                // A single failed enrichment shouldn't sink the list;
                // that PR just keeps zeroed stats until opened.
                continue;
            };
            if let Some(pr) = prs.get_mut(index) {
                *pr = detail;
            }
        }
        self.augment_review_status(repo, &mut prs).await;
        Ok(prs)
    }

    /// One GraphQL query per page filling reviewDecision + check
    /// rollup — the reliable "approved and green" signal that
    /// `mergeable_state` can't provide on merge-queue repos. Best-effort:
    /// failures leave the fields None.
    pub(crate) async fn augment_review_status(&self, repo: &RepoRef, prs: &mut [PullRequest]) {
        const QUERY: &str = "query($owner:String!,$name:String!){\
            repository(owner:$owner,name:$name){\
            pullRequests(states:OPEN,first:50,orderBy:{field:UPDATED_AT,direction:DESC}){\
            nodes{number reviewDecision \
            reviewThreads(first:100){nodes{isResolved}} \
            commits(last:1){nodes{commit{statusCheckRollup{state}}}}}}}}";
        let body = serde_json::json!({
            "query": QUERY,
            "variables": { "owner": repo.owner, "name": repo.name },
        });
        let Ok(value) = self.post_graphql_query(body).await else {
            return;
        };
        let nodes = value
            .pointer("/data/repository/pullRequests/nodes")
            .and_then(|n| n.as_array());
        for node in nodes.into_iter().flatten() {
            let Some(number) = node.get("number").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            if let Some(pr) = prs
                .iter_mut()
                .find(|p| p.number == number && p.repo == *repo)
            {
                pr.review_decision = node
                    .get("reviewDecision")
                    .and_then(|d| d.as_str())
                    .map(str::to_owned);
                pr.checks_state = node
                    .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
                    .and_then(|s| s.as_str())
                    .map(str::to_owned);
                pr.unresolved_threads = node
                    .pointer("/reviewThreads/nodes")
                    .and_then(|n| n.as_array())
                    .map_or(0, |nodes| {
                        nodes
                            .iter()
                            .filter(|t| {
                                t.get("isResolved").and_then(serde_json::Value::as_bool)
                                    == Some(false)
                            })
                            .count() as u64
                    });
            }
        }
    }

    /// One PR's core data (cheap single call — used for merge checks).
    pub async fn pull_request(&self, repo: &RepoRef, number: u64) -> Result<PullRequest> {
        let pull: WirePull = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}",
                repo.owner, repo.name
            ))
            .await?;
        Ok(pull.into_domain(repo))
    }

    /// Review-thread resolution states via GraphQL (best-effort).
    async fn fetch_thread_meta(
        &self,
        repo: &RepoRef,
        number: u64,
    ) -> Vec<tandem_core::github::ReviewThreadMeta> {
        const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){\
            repository(owner:$owner,name:$name){pullRequest(number:$number){\
            reviewThreads(first:100){nodes{id isResolved \
            comments(first:1){nodes{databaseId}}}}}}}";
        let body = serde_json::json!({
            "query": QUERY,
            "variables": { "owner": repo.owner, "name": repo.name, "number": number },
        });
        let Ok(value) = self.post_graphql_query(body).await else {
            return Vec::new();
        };
        let nodes = value
            .pointer("/data/repository/pullRequest/reviewThreads/nodes")
            .and_then(|n| n.as_array());
        nodes
            .into_iter()
            .flatten()
            .filter_map(|node| {
                Some(tandem_core::github::ReviewThreadMeta {
                    id: node.get("id")?.as_str()?.to_owned(),
                    resolved: node.get("isResolved")?.as_bool()?,
                    root_comment_id: node.pointer("/comments/nodes/0/databaseId")?.as_u64()?,
                })
            })
            .collect()
    }

    /// Names of currently failing check runs (for the hover tip).
    pub async fn failing_checks(&self, repo: &RepoRef, number: u64) -> Result<Vec<String>> {
        use tandem_core::github::CheckState;
        let pr = self.pull_request(repo, number).await?;
        let checks: WireCheckRunList = self
            .get_json(&format!(
                "/repos/{}/{}/commits/{}/check-runs?per_page=100",
                repo.owner, repo.name, pr.head_sha
            ))
            .await?;
        Ok(checks
            .check_runs
            .into_iter()
            .map(tandem_core::github::CheckRun::from)
            .filter(|c| c.state == CheckState::Failure)
            .map(|c| c.name)
            .collect())
    }

    /// Like [`Self::pull_request`], but retries once when GitHub is
    /// still lazily computing `mergeable_state` (first fetch after a
    /// push often reports "unknown").
    pub async fn pull_request_settled(&self, repo: &RepoRef, number: u64) -> Result<PullRequest> {
        let pr = self.pull_request(repo, number).await?;
        let unsettled = matches!(pr.mergeable_state.as_deref(), None | Some("unknown"));
        if !unsettled {
            return Ok(pr);
        }
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        self.pull_request(repo, number).await
    }

    /// Fetch up to 3 pages (300 items) of a listing endpoint. The path
    /// must already contain `per_page=100`.
    async fn get_json_paged<T: serde::de::DeserializeOwned>(&self, base: &str) -> Result<Vec<T>> {
        let mut all = Vec::new();
        for page in 1..=3 {
            let mut batch: Vec<T> = self.get_json(&format!("{base}&page={page}")).await?;
            let n = batch.len();
            all.append(&mut batch);
            if n < 100 {
                break;
            }
        }
        Ok(all)
    }

    /// Full detail bundle: PR, checks, reviews, and all comments.
    pub async fn pull_request_detail(&self, repo: &RepoRef, number: u64) -> Result<PrDetail> {
        let base = format!("/repos/{}/{}", repo.owner, repo.name);

        let pull: WirePull = self.get_json(&format!("{base}/pulls/{number}")).await?;
        let pull_request = pull.into_domain(repo);

        let checks: WireCheckRunList = self
            .get_json(&format!(
                "{base}/commits/{}/check-runs?per_page=100",
                pull_request.head_sha
            ))
            .await?;
        let reviews: Vec<WireReview> = self
            .get_json_paged(&format!("{base}/pulls/{number}/reviews?per_page=100"))
            .await?;

        // GitHub splits discussion into issue comments (thread) and review
        // comments (inline); Tandem shows both, paginated.
        let issue_comments: Vec<WireComment> = self
            .get_json_paged(&format!("{base}/issues/{number}/comments?per_page=100"))
            .await?;
        let review_comments: Vec<WireComment> = self
            .get_json_paged(&format!("{base}/pulls/{number}/comments?per_page=100"))
            .await?;

        let mut comments: Vec<GithubComment> = issue_comments.into_iter().map(Into::into).collect();
        comments.extend(review_comments.into_iter().map(GithubComment::from));
        comments.sort_by_key(|c| c.created_at);

        let all_reviews: Vec<tandem_core::github::GithubReview> =
            reviews.into_iter().map(Into::into).collect();
        let mut review_bodies: Vec<_> = all_reviews
            .iter()
            .filter(|r| !r.body.trim().is_empty())
            .cloned()
            .collect();
        review_bodies.sort_by_key(|r| r.submitted_at);

        let review_threads = self.fetch_thread_meta(repo, number).await;

        Ok(PrDetail {
            pull_request,
            checks: checks.check_runs.into_iter().map(Into::into).collect(),
            reviews: effective_reviews(all_reviews),
            comments,
            review_bodies,
            review_threads,
        })
    }

    /// The PR's raw unified diff text (callers parse and/or cache it).
    pub async fn pull_request_diff_raw(&self, repo: &RepoRef, number: u64) -> Result<String> {
        let path = format!("/repos/{}/{}/pulls/{number}", repo.owner, repo.name);
        let resp = self.get(&path, DIFF_ACCEPT).await?;
        resp.text().await.map_err(|e| TandemError::GithubApi {
            status: 0,
            message: format!("bad body: {e}"),
        })
    }

    /// Convenience: the diff parsed into structured files.
    pub async fn pull_request_diff(&self, repo: &RepoRef, number: u64) -> Result<Vec<FileDiff>> {
        let raw = self.pull_request_diff_raw(repo, number).await?;
        parse_unified_diff(&raw)
    }

    /// GraphQL is POST-only even for reads. Distinct from
    /// `write::post_json`, which is reserved for explicit user-triggered
    /// writes and logs accordingly — this is just a read.
    pub(crate) async fn post_graphql_query(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/graphql", self.api_base);
        let headers = self.headers(JSON_ACCEPT)?;
        tracing::debug!(%url, "github POST (graphql read)");
        let resp = self
            .send_with_rate_limit_retry(|| {
                self.http.post(&url).headers(headers.clone()).json(&body)
            })
            .await?;
        resp.json().await.map_err(|e| TandemError::GithubApi {
            status: 0,
            message: format!("bad body: {e}"),
        })
    }
}

fn header_value(s: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(s).map_err(|e| TandemError::Config(format!("bad header value: {e}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::write::urlenc;

    #[test]
    fn encodes_query_values() {
        assert_eq!(
            urlenc("repo:o/r is:pr fix bug"),
            "repo%3Ao%2Fr%20is%3Apr%20fix%20bug"
        );
        assert_eq!(urlenc("safe-value_1.2~x"), "safe-value_1.2~x");
    }
}
