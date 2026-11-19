//! GitHub work off the UI thread: fetching a PR you just queued (so the
//! list shows its title), and the two writes — posting a comment and
//! approving — which only ever arrive here from a confirmed keypress.
//! The worker has its own store connection; the UI sees its writes on
//! the next reload.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use tandem_cache::{Cache, ReviewStore, ReviewTriage};
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
}

#[derive(Debug, Clone)]
pub enum Done {
    Ok(String),
    Err(String),
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
    }
}
