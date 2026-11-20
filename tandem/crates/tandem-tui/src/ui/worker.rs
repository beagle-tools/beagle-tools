//! GitHub work off the UI thread: fetching a PR you just queued (so the
//! list shows its title), and the two writes — posting a comment and
//! approving — which only ever arrive here from a confirmed keypress.
//! The worker has its own store connection; the UI sees its writes on
//! the next reload.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use tandem_cache::{Cache, ReviewStore, ReviewTriage};
use tandem_core::github::PrState;
use tandem_core::{Result, TandemError};
use tandem_github::GithubClient;

use crate::config::Config;
use crate::gh;
use crate::pr_ref::PrRef;

#[derive(Debug, Clone)]
pub enum Cmd {
    Fetch(PrRef),
    Post(String),
    Approve(PrRef),
    /// The background merged/closed check (see `term::CHECK_STATES`).
    CheckStates(Vec<PrRef>),
}

#[derive(Debug, Clone)]
pub enum Done {
    Ok(String),
    Err(String),
    /// Finished with nothing worth a status line (a check that found no
    /// change). The UI still reloads, so stage changes show.
    Quiet,
}

pub fn spawn(db: PathBuf, config: Config) -> (Sender<Cmd>, Receiver<Done>) {
    let (tx, cmds) = mpsc::channel::<Cmd>();
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
        let mut client: Option<GithubClient> = None;
        for cmd in cmds {
            let result = rt.block_on(async {
                if client.is_none() {
                    client = Some(gh::connect(&config).await?);
                }
                match &client {
                    Some(c) => handle(&cache, c, &cmd).await,
                    None => Err(TandemError::Auth("no GitHub client".into())),
                }
            });
            let msg = match result {
                Ok(msg) if msg.is_empty() => Done::Quiet,
                Ok(msg) => Done::Ok(msg),
                Err(e) => Done::Err(e.to_string()),
            };
            if done.send(msg).is_err() {
                return; // the UI is gone
            }
        }
    });
    (tx, rx)
}

async fn handle(cache: &Cache, client: &GithubClient, cmd: &Cmd) -> Result<String> {
    match cmd {
        Cmd::Fetch(pr) => {
            gh::fetch(client, cache, pr).await?;
            Ok(format!("fetched {}", pr.label()))
        }
        Cmd::Post(id) => {
            let comment = cache
                .get_comment(id)?
                .ok_or_else(|| TandemError::Cache("that comment is gone".into()))?;
            if comment.posted_github_id.is_some() {
                return Ok("already posted".to_owned());
            }
            let github_id = gh::post_comment(client, &comment).await?;
            cache.set_comment_posted(id, github_id)?;
            Ok(format!(
                "posted to GitHub ({}:{})",
                comment.path, comment.line
            ))
        }
        Cmd::Approve(pr) => {
            gh::approve(client, pr).await?;
            cache.set_approved(&pr.repo.slug(), pr.number)?;
            Ok(format!("approved {}", pr.label()))
        }
        Cmd::CheckStates(prs) => check_states(cache, client, prs).await,
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
