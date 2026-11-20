//! GitHub, through Tandem's client: fetching a PR into the store, and the
//! only two writes — posting a comment you picked, and approving. Both
//! writes run solely from an explicit, confirmed keypress in the TUI.
//!
//! The account can be pinned by `github_user` in the config: with more
//! than one `gh` account logged in, plain `gh auth token` answers with
//! whichever is active, which may not be the one you review as.

use tandem_cache::Cache;
use tandem_core::diff_parse::parse_unified_diff;
use tandem_core::github::{PrDetail, PrState};
use tandem_core::review::{DiffSide, LocalComment};
use tandem_core::{Result, TandemError};
use tandem_github::{GithubAuth, GithubClient, GithubConfig, NewInlineComment};

use crate::config::Config;
use crate::pr_ref::PrRef;

/// A client authenticated as the configured account.
pub async fn connect(config: &Config) -> Result<GithubClient> {
    let auth = match &config.github_user {
        Some(user) => GithubAuth::Token {
            token: token_for(user).await?,
        },
        None => GithubAuth::GhCli,
    };
    GithubClient::connect(&GithubConfig {
        auth,
        ..GithubConfig::default()
    })
    .await
}

/// Bound on the `gh auth token` lookup.
const GH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

async fn token_for(user: &str) -> Result<String> {
    let owned = user.to_owned();
    // `beagle_utils::gh` is blocking (bounded, killed on expiry), so it runs
    // off the runtime's thread.
    let out = tokio::task::spawn_blocking(move || {
        beagle_utils::gh::run(&["auth", "token", "-u", &owned], GH_TIMEOUT)
    })
    .await
    .map_err(|e| TandemError::Auth(format!("`gh auth token -u {user}` did not finish: {e}")))?;
    let token = match out {
        Ok(stdout) => stdout.trim().to_owned(),
        Err(beagle_utils::gh::Error::Failed(stderr)) => {
            return Err(TandemError::Auth(format!(
                "`gh auth token -u {user}` failed — is {user} logged in to gh? ({stderr})"
            )))
        }
        Err(e) => return Err(TandemError::Auth(format!("`gh auth token -u {user}`: {e}"))),
    };
    if token.is_empty() {
        return Err(TandemError::Auth(format!(
            "`gh auth token -u {user}` printed no token — is {user} logged in to gh?"
        )));
    }
    Ok(token)
}

/// Fetches the PR's detail (with every comment and review on it) and
/// its diff into the store. Returns the detail.
pub async fn fetch(client: &GithubClient, cache: &Cache, pr: &PrRef) -> Result<PrDetail> {
    let detail = client.pull_request_detail(&pr.repo, pr.number).await?;
    let raw = client.pull_request_diff_raw(&pr.repo, pr.number).await?;
    let files = parse_unified_diff(&raw)?;
    let head = &detail.pull_request.head_sha;
    cache.put_diff(&pr.repo, pr.number, head, &raw, &files)?;
    cache.put_pr_detail(&pr.repo, &detail)?;
    Ok(detail)
}

/// Re-reads one PR's state from GitHub into the store. Returns the state
/// the store had before (if it had the PR) and the state now.
///
/// One small request: the PR alone, without its comments or diff. A PR
/// the store has never fetched gets the full detail instead, since there
/// is no stored detail to update.
pub async fn refresh_state(
    client: &GithubClient,
    cache: &Cache,
    pr: &PrRef,
) -> Result<(Option<PrState>, PrState)> {
    if let Some(mut detail) = cache.get_pr_detail(&pr.repo, pr.number)? {
        let before = detail.pull_request.state;
        detail.pull_request = client.pull_request(&pr.repo, pr.number).await?;
        cache.put_pr_detail(&pr.repo, &detail)?;
        Ok((Some(before), detail.pull_request.state))
    } else {
        let detail = client.pull_request_detail(&pr.repo, pr.number).await?;
        cache.put_pr_detail(&pr.repo, &detail)?;
        Ok((None, detail.pull_request.state))
    }
}

/// What gets posted for a local comment: its body, plus the fix when it
/// carries one. The local history reply is for you and stays local.
///
/// A fix on one new-side line becomes a GitHub `suggestion` block (one click
/// to apply). A multi-line fix is posted as a plain code block instead:
/// a suggestion over a range needs the comment anchored to the whole
/// range, which Tandem's single-line inline comment cannot express, and
/// a suggestion anchored to the wrong lines would apply the wrong edit.
pub fn github_body(comment: &LocalComment) -> String {
    let body = comment.body.trim_end();
    let Some(fix) = &comment.suggestion else {
        return body.to_owned();
    };
    let single = comment.end_line.is_none_or(|end| end == comment.line);
    if single && comment.side == DiffSide::New {
        format!("{body}\n\n```suggestion\n{fix}\n```")
    } else {
        let end = comment.end_line.unwrap_or(comment.line);
        format!(
            "{body}\n\nSuggested replacement for lines {}–{end}:\n\n```\n{fix}\n```",
            comment.line
        )
    }
}

/// Posts `comment` as an inline review comment on the commit it was
/// written against. Returns GitHub's comment id.
pub async fn post_comment(client: &GithubClient, comment: &LocalComment) -> Result<u64> {
    let body = github_body(comment);
    if comment.path.is_empty() || comment.line == 0 {
        return client
            .post_issue_comment(&comment.repo, comment.pr_number, &body)
            .await;
    }
    client
        .create_review_comment(
            &comment.repo,
            comment.pr_number,
            NewInlineComment {
                commit_id: &comment.head_sha,
                path: &comment.path,
                line: comment.line,
                right_side: comment.side == DiffSide::New,
                body: &body,
            },
        )
        .await
}

pub async fn approve(client: &GithubClient, pr: &PrRef) -> Result<()> {
    client.approve_pull_request(&pr.repo, pr.number, None).await
}
