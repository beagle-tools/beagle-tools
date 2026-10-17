//! The write half of the GitHub client. These methods exist ONLY for
//! explicit user actions in the UI (post a chosen comment, approve a
//! PR) — agents have no path to them.

use base64::Engine as _;
use tandem_core::github::RepoRef;
use tandem_core::{Result, TandemError};

use crate::client::{GithubClient, JSON_ACCEPT};

/// Parameters for a brand-new inline review comment.
#[derive(Debug, Clone)]
pub struct NewInlineComment<'a> {
    /// Head SHA the comment anchors to; GitHub rejects SHAs not in the PR.
    pub commit_id: &'a str,
    /// Repo-relative file path.
    pub path: &'a str,
    /// Line number on the chosen side of the diff.
    pub line: u64,
    /// `true` for the new (RIGHT) side, `false` for the old (LEFT) side.
    pub right_side: bool,
    /// Markdown body.
    pub body: &'a str,
}

/// GraphQL answers 200 with an `errors` array instead of a status code,
/// so every mutation has to look inside the body.
fn graphql_error(value: &serde_json::Value, fallback: &str) -> Result<()> {
    let Some(first) = value
        .get("errors")
        .and_then(|e| e.as_array())
        .and_then(|e| e.first())
    else {
        return Ok(());
    };
    let msg = first
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or(fallback);
    Err(TandemError::GithubApi {
        status: 0,
        message: msg.to_owned(),
    })
}

impl GithubClient {
    pub(crate) async fn post_json(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.api_base);
        let headers = self.headers(JSON_ACCEPT)?;
        tracing::info!(%url, "github POST (explicit user action)");
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

    fn id_of(value: &serde_json::Value) -> Result<u64> {
        value
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| TandemError::GithubApi {
                status: 0,
                message: "response missing id".into(),
            })
    }

    /// Post a plain PR (issue) comment. Returns the new comment id.
    pub async fn post_issue_comment(&self, repo: &RepoRef, number: u64, body: &str) -> Result<u64> {
        let path = format!(
            "/repos/{}/{}/issues/{number}/comments",
            repo.owner, repo.name
        );
        let value = self
            .post_json(&path, serde_json::json!({ "body": body }))
            .await?;
        Self::id_of(&value)
    }

    /// Reply inside an existing review-comment thread.
    pub async fn reply_to_review_comment(
        &self,
        repo: &RepoRef,
        number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<u64> {
        let path = format!(
            "/repos/{}/{}/pulls/{number}/comments/{comment_id}/replies",
            repo.owner, repo.name
        );
        let value = self
            .post_json(&path, serde_json::json!({ "body": body }))
            .await?;
        Self::id_of(&value)
    }

    /// Start a new inline review comment on a diff line.
    pub async fn create_review_comment(
        &self,
        repo: &RepoRef,
        number: u64,
        new: NewInlineComment<'_>,
    ) -> Result<u64> {
        let api_path = format!(
            "/repos/{}/{}/pulls/{number}/comments",
            repo.owner, repo.name
        );
        let value = self
            .post_json(
                &api_path,
                serde_json::json!({
                    "body": new.body,
                    "commit_id": new.commit_id,
                    "path": new.path,
                    "line": new.line,
                    "side": if new.right_side { "RIGHT" } else { "LEFT" },
                }),
            )
            .await?;
        Self::id_of(&value)
    }

    /// Approve the pull request (optionally with a review body).
    pub async fn approve_pull_request(
        &self,
        repo: &RepoRef,
        number: u64,
        body: Option<&str>,
    ) -> Result<()> {
        let path = format!("/repos/{}/{}/pulls/{number}/reviews", repo.owner, repo.name);
        let mut payload = serde_json::json!({ "event": "APPROVE" });
        if let Some(b) = body {
            payload["body"] = serde_json::Value::String(b.to_owned());
        }
        self.post_json(&path, payload).await?;
        Ok(())
    }
}

/// Minimal percent-encoding for a query-string value.
pub(crate) fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Debug, serde::Deserialize)]
struct WireContents {
    content: String,
    sha: String,
}

impl GithubClient {
    /// File text + blob sha at a ref (Contents API; base64-decoded).
    pub async fn get_file(
        &self,
        repo: &RepoRef,
        path: &str,
        git_ref: &str,
    ) -> Result<(String, String)> {
        let api = format!(
            "/repos/{}/{}/contents/{}?ref={}",
            repo.owner,
            repo.name,
            path,
            urlenc(git_ref)
        );
        let wire: WireContents = self.get_json(&api).await?;
        let compact: String = wire.content.split_whitespace().collect();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(compact)
            .map_err(|e| TandemError::GithubApi {
                status: 0,
                message: format!("undecodable file content: {e}"),
            })?;
        let text = String::from_utf8(bytes).map_err(|_| TandemError::GithubApi {
            status: 0,
            message: "file is not valid UTF-8 — cannot apply a text suggestion".into(),
        })?;
        Ok((text, wire.sha))
    }

    /// Commit new file content to a branch (the "commit suggestion"
    /// path — an explicit user action, like every write here).
    pub async fn commit_file(
        &self,
        repo: &RepoRef,
        path: &str,
        branch: &str,
        message: &str,
        content: &str,
        blob_sha: &str,
    ) -> Result<String> {
        let api = format!("/repos/{}/{}/contents/{}", repo.owner, repo.name, path);
        let url = format!("{}{api}", self.api_base);
        tracing::info!(%url, "github PUT (explicit user action)");
        let body = serde_json::json!({
            "message": message,
            "content": base64::engine::general_purpose::STANDARD.encode(content),
            "sha": blob_sha,
            "branch": branch,
        });
        let resp = self
            .http
            .put(&url)
            .headers(self.headers(JSON_ACCEPT)?)
            .json(&body)
            .send()
            .await
            .map_err(|e| TandemError::GithubApi {
                status: 0,
                message: e.to_string(),
            })?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(TandemError::GithubApi {
                status: status.as_u16(),
                message,
            });
        }
        // The commit sha is what makes "committed" checkable after the
        // fact, so it travels back with the result.
        let body: serde_json::Value = resp.json().await.map_err(|e| TandemError::GithubApi {
            status: 0,
            message: e.to_string(),
        })?;
        Ok(body
            .get("commit")
            .and_then(|c| c.get("sha"))
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_owned())
    }
}

impl GithubClient {
    /// Merge the PR now (method: "merge" | "squash" | "rebase").
    pub async fn merge_pull_request(
        &self,
        repo: &RepoRef,
        number: u64,
        method: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{number}/merge",
            self.api_base, repo.owner, repo.name
        );
        tracing::info!(%url, "github PUT merge (explicit user action)");
        let resp = self
            .http
            .put(&url)
            .headers(self.headers(JSON_ACCEPT)?)
            .json(&serde_json::json!({ "merge_method": method }))
            .send()
            .await
            .map_err(|e| TandemError::GithubApi {
                status: 0,
                message: e.to_string(),
            })?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(TandemError::GithubApi {
                status: status.as_u16(),
                message,
            });
        }
        Ok(())
    }

    /// Update the PR branch from its base (the "update branch" button).
    pub async fn update_branch(&self, repo: &RepoRef, number: u64) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{number}/update-branch",
            self.api_base, repo.owner, repo.name
        );
        tracing::info!(%url, "github PUT update-branch (explicit user action)");
        let resp = self
            .http
            .put(&url)
            .headers(self.headers(JSON_ACCEPT)?)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| TandemError::GithubApi {
                status: 0,
                message: e.to_string(),
            })?;
        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(TandemError::GithubApi {
                status: status.as_u16(),
                message,
            });
        }
        Ok(())
    }

    /// Resolve a review thread (GraphQL) — explicit user action.
    pub async fn resolve_review_thread(&self, thread_id: &str) -> Result<()> {
        let mutation = "mutation($id: ID!) {\
            resolveReviewThread(input: {threadId: $id}) { clientMutationId } }";
        let value = self
            .post_json(
                "/graphql",
                serde_json::json!({ "query": mutation, "variables": { "id": thread_id } }),
            )
            .await?;
        graphql_error(&value, "resolve mutation failed")
    }

    /// Enable auto-merge (GraphQL — on merge-queue repos this enqueues).
    pub async fn enable_auto_merge(&self, node_id: &str, method: &str) -> Result<()> {
        let mutation = "mutation($id: ID!, $method: PullRequestMergeMethod!) {\
            enablePullRequestAutoMerge(input: {pullRequestId: $id, mergeMethod: $method}) {\
            clientMutationId } }";
        let value = self
            .post_json(
                "/graphql",
                serde_json::json!({
                    "query": mutation,
                    "variables": { "id": node_id, "method": method.to_uppercase() },
                }),
            )
            .await?;
        graphql_error(&value, "auto-merge mutation failed")
    }

    /// Take a PR out of draft (GraphQL) — explicit user action.
    pub async fn mark_pr_ready_for_review(&self, node_id: &str) -> Result<()> {
        let mutation = "mutation($id: ID!) {\
            markPullRequestReadyForReview(input: {pullRequestId: $id}) {\
            clientMutationId } }";
        let value = self
            .post_json(
                "/graphql",
                serde_json::json!({ "query": mutation, "variables": { "id": node_id } }),
            )
            .await?;
        graphql_error(&value, "ready-for-review mutation failed")
    }
}
