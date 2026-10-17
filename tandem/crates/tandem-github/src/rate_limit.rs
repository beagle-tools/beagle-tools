//! Rate-limit-aware request sending, shared by every GET and POST the
//! client makes (reads in `client.rs`, writes in `write.rs`).

use reqwest::header::RETRY_AFTER;
use tandem_core::{Result, TandemError};

use crate::client::GithubClient;

impl GithubClient {
    /// Send a request, waiting out GitHub's rate limit and retrying
    /// (bounded) when the response says to. `Retry-After` covers
    /// secondary/abuse-detection limits (a handful of seconds); a
    /// `x-ratelimit-remaining: 0` primary-limit response waits until
    /// `x-ratelimit-reset`, capped so a far-off reset can't hang a
    /// command for the better part of an hour. Any other 4xx/5xx
    /// returns immediately — a genuine 403 (no access) must not retry.
    pub(crate) async fn send_with_rate_limit_retry(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        const MAX_RETRIES: u32 = 3;
        const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(90);

        let mut attempt = 0;
        loop {
            let resp = build().send().await.map_err(|e| TandemError::GithubApi {
                status: 0,
                message: e.to_string(),
            })?;
            if resp.status().is_success() {
                return Ok(resp);
            }
            if attempt < MAX_RETRIES {
                if let Some(wait) = rate_limit_wait(&resp, MAX_WAIT) {
                    tracing::warn!(
                        seconds = wait.as_secs(),
                        attempt,
                        "github rate limited — waiting before retry"
                    );
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                    continue;
                }
            }
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(TandemError::GithubApi {
                status: status.as_u16(),
                message,
            });
        }
    }
}

/// How long to wait before retrying a rate-limited response, or `None`
/// if this doesn't look like a rate limit at all (e.g. a plain 403 for
/// lacking access, which must not be retried).
fn rate_limit_wait(
    resp: &reqwest::Response,
    max_wait: std::time::Duration,
) -> Option<std::time::Duration> {
    let status = resp.status();
    if status != reqwest::StatusCode::FORBIDDEN && status != reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return None;
    }
    let headers = resp.headers();
    let header_num =
        |name: &str| -> Option<i64> { headers.get(name)?.to_str().ok()?.parse::<i64>().ok() };
    if let Some(secs) = headers
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        return Some(std::time::Duration::from_secs(secs).min(max_wait));
    }
    if header_num("x-ratelimit-remaining") == Some(0) {
        let reset = header_num("x-ratelimit-reset")?;
        let wait_secs = (reset - chrono::Utc::now().timestamp())
            .max(1)
            .unsigned_abs();
        return Some(std::time::Duration::from_secs(wait_secs).min(max_wait));
    }
    None
}
