//! GitHub work off the UI thread: fetching a PR you just queued (so the
//! list shows its title), and the two writes — posting a comment and
//! approving — which only ever arrive here from a confirmed keypress.
//! The worker has its own store connection; the UI sees its writes on
//! the next reload.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use beagle_utils::gh::Account;
use tandem_cache::{Cache, ReviewStore, ReviewTriage};
use tandem_core::github::PrState;
use tandem_core::{Result, TandemError};
use tandem_github::GithubClient;

use crate::gh;
use crate::pr_ref::PrRef;

/// One piece of work. Reads go out as `reader` (gh's active account when
/// `None`); writes name their own account and never fall back.
#[derive(Debug, Clone)]
pub struct Job {
    pub reader: Option<String>,
    pub cmd: Cmd,
}

#[derive(Debug, Clone)]
pub enum Cmd {
    Fetch(PrRef),
    /// Post the comment with this id, as `user`.
    Post {
        comment_id: String,
        user: String,
    },
    /// Approve the PR, as `user`.
    Approve {
        pr: PrRef,
        user: String,
    },
    /// The background merged/closed check (see `term::CHECK_STATES`).
    CheckStates(Vec<PrRef>),
    /// List the accounts gh is logged into.
    Accounts,
}

#[derive(Debug, Clone)]
pub enum Done {
    Ok(String),
    Err(String),
    /// Finished with nothing worth a status line (a check that found no
    /// change). The UI still reloads, so stage changes show.
    Quiet,
    /// gh's github.com accounts.
    Accounts(Vec<Account>),
}

pub fn spawn(db: PathBuf) -> (Sender<Job>, Receiver<Done>) {
    let (tx, jobs) = mpsc::channel::<Job>();
    let (done, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let setup = Cache::open(&db).and_then(|cache| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            Ok((cache, rt))
        });
        let (cache, rt) = match setup {
            Ok(ok) => ok,
            Err(e) => {
                // The UI may already be gone; then nobody is left to tell.
                let _ = done.send(Done::Err(format!("GitHub worker failed to start: {e}")));
                return;
            }
        };
        // One client per account, made on first use.
        let mut clients: HashMap<Option<String>, GithubClient> = HashMap::new();
        for job in jobs {
            let msg = if matches!(job.cmd, Cmd::Accounts) {
                match gh::accounts() {
                    Ok(list) => Done::Accounts(list),
                    Err(e) => Done::Err(e.to_string()),
                }
            } else {
                let account = acting_as(&job);
                let result = rt.block_on(async {
                    if !clients.contains_key(&account) {
                        let client = gh::connect_as(account.as_deref()).await?;
                        clients.insert(account.clone(), client);
                    }
                    match clients.get(&account) {
                        Some(c) => handle(&cache, c, &job.cmd).await,
                        None => Err(TandemError::Auth("no GitHub client".into())),
                    }
                });
                match result {
                    Ok(msg) if msg.is_empty() => Done::Quiet,
                    Ok(msg) => Done::Ok(msg),
                    Err(e) => Done::Err(e.to_string()),
                }
            };
            if done.send(msg).is_err() {
                return; // the UI is gone
            }
        }
    });
    (tx, rx)
}

/// The account a job runs as: a write's own, else the reader.
pub fn acting_as(job: &Job) -> Option<String> {
    match &job.cmd {
        Cmd::Post { user, .. } | Cmd::Approve { user, .. } => Some(user.clone()),
        _ => job.reader.clone(),
    }
}

async fn handle(cache: &Cache, client: &GithubClient, cmd: &Cmd) -> Result<String> {
    match cmd {
        Cmd::Fetch(pr) => {
            gh::fetch(client, cache, pr).await?;
            Ok(format!("fetched {}", pr.label()))
        }
        Cmd::Post { comment_id, user } => {
            let comment = cache
                .get_comment(comment_id)?
                .ok_or_else(|| TandemError::Cache("that comment is gone".into()))?;
            if comment.posted_github_id.is_some() {
                return Ok("already posted".to_owned());
            }
            let github_id = gh::post_comment(client, &comment).await?;
            cache.set_comment_posted(comment_id, github_id)?;
            Ok(format!(
                "posted to GitHub as {user} ({}:{})",
                comment.path, comment.line
            ))
        }
        Cmd::Approve { pr, user } => {
            gh::approve(client, pr).await?;
            cache.set_approved(&pr.repo.slug(), pr.number)?;
            Ok(format!("approved {} as {user}", pr.label()))
        }
        Cmd::CheckStates(prs) => check_states(cache, client, prs).await,
        Cmd::Accounts => Ok(String::new()),
    }
}

/// Refreshes each PR's state; names the ones that just merged or closed.
/// An empty message means nothing changed, so the status line is left
/// alone. One PR failing (deleted repo, lost access) doesn't stop the rest.
async fn check_states(cache: &Cache, client: &GithubClient, prs: &[PrRef]) -> Result<String> {
    let mut changed = Vec::new();
    let mut last_err = None;
    for pr in prs {
        match gh::refresh_state(client, cache, pr).await {
            Ok((before, now)) => {
                if before != Some(now) && now != PrState::Open {
                    let what = if now == PrState::Merged {
                        "merged"
                    } else {
                        "closed"
                    };
                    changed.push(format!("{} was {what}", pr.label()));
                }
            }
            Err(e) => last_err = Some(e),
        }
    }
    match (changed.is_empty(), last_err) {
        (true, Some(e)) => Err(e),
        _ => Ok(changed.join(" · ")),
    }
}
