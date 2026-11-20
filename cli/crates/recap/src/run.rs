//! The entry point behind [`crate::run`]: `beagle recap` opens the TUI; the
//! subcommands pull PRs from GitHub and give agents a typed way to read and
//! scaffold recaps.

use std::env;
use std::ffi::OsString;
use std::process::ExitCode;

use crate::cli::{self, Command, USAGE};
use crate::model::{PrRef, RecapId};
use crate::store::{self, Shelf, Store};
use crate::{actions, comments, config, gh, poller, ui, Error};

pub(crate) fn main(args: Vec<OsString>) -> ExitCode {
    let args = match cli::parse_os(args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("beagle recap: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("beagle recap: {e}");
            ExitCode::FAILURE
        }
    }
}

fn load_config() -> Result<config::Config, Error> {
    match config::config_path() {
        Some(path) => config::load(&path),
        None => Ok(config::Config::default()),
    }
}

fn store_for(root_flag: Option<&str>) -> Result<Store, Error> {
    Ok(Store::new(config::root_for(root_flag)?))
}

/// `beagle recap new`: pull a PR and scaffold its recap.
fn new(
    store: &Store,
    pr: &str,
    ticket: Option<String>,
    ticket_url: Option<String>,
) -> Result<(), Error> {
    let pr = PrRef::parse(pr)?;
    let id = RecapId::for_pr(&pr);
    if store.recap_dir(&id).exists() {
        return Err(Error::AlreadyExists(
            store.recap_dir(&id).display().to_string(),
        ));
    }
    // Archived on purpose: don't silently bring it back.
    if store.shelf_dir(Shelf::Archived).join(id.as_str()).exists() {
        return Err(Error::Tool {
            tool: "recap",
            message: format!("{id} is archived — `beagle recap unarchive {id}` brings it back"),
        });
    }
    let (mut manifest, diff) = gh::fetch(&pr)?;
    manifest.ticket = ticket.or(manifest.ticket);
    manifest.ticket_url = ticket_url;
    println!("{}", store.create(&id, &manifest, &diff)?.display());
    Ok(())
}

/// A recap id, or a PR (URL / `owner/repo#n`) turned into its id.
fn resolve(target: &str) -> Result<RecapId, Error> {
    PrRef::parse(target).map_or_else(|_| RecapId::new(target), |pr| Ok(RecapId::for_pr(&pr)))
}

fn run(args: cli::Args) -> Result<(), Error> {
    let store = match &args.command {
        Command::Help => {
            print!("{USAGE}");
            return Ok(());
        }
        Command::Version => {
            println!("beagle recap {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Command::Guide => {
            print!("{}", crate::AGENT_GUIDE);
            return Ok(());
        }
        _ => store_for(args.root.as_deref())?,
    };
    match args.command {
        Command::Tui { target } => {
            let id = target.as_deref().map(resolve).transpose()?;
            let mut app = ui::App::new(store, id.as_ref())?;
            app.set_comments_config(load_config()?.comments);
            let mut terminal = ratatui::init();
            let result = app.run(&mut terminal);
            ratatui::restore();
            result
        }
        Command::New {
            pr,
            ticket,
            ticket_url,
        } => new(&store, &pr, ticket, ticket_url),
        Command::Refresh { target, ticket_url } => {
            let id = resolve(&target)?;
            let old = store.load(&id)?;
            let pr = PrRef::parse(&old.manifest.url)?;
            let (mut manifest, diff) = gh::fetch(&pr)?;
            manifest.ticket_url = ticket_url;
            println!("{}", store.refresh(&id, &manifest, &diff)?.display());
            Ok(())
        }
        Command::List { json, archived } => list(
            &store,
            json,
            if archived {
                Shelf::Archived
            } else {
                Shelf::Active
            },
        ),
        Command::Archive { target } => {
            println!("{}", store.archive(&resolve(&target)?)?.display());
            Ok(())
        }
        Command::Unarchive { target } => {
            println!("{}", store.unarchive(&resolve(&target)?)?.display());
            Ok(())
        }
        Command::MergeCheck { target } => merge_check(&store, &target),
        Command::Sync => sync(&store),
        Command::Comments { target } => comments_print(&store, &target),
        Command::CommentsSync => comments_sync(&store),
        Command::CommentsPending { json } => comments_pending(&store, json),
        Command::CommentsReview {
            target,
            thread,
            file,
        } => comments_review(&store, &target, &thread, &file),
        Command::Show { target, diff } => {
            let recap = store.load_any(&resolve(&target)?)?;
            print!("{}", store::export_markdown(&recap, diff));
            Ok(())
        }
        Command::Path { target } => {
            let path = match target {
                Some(t) => store.recap_dir(&resolve(&t)?),
                None => store.recaps_dir(),
            };
            println!("{}", path.display());
            Ok(())
        }
        Command::Help | Command::Version | Command::Guide => Ok(()),
    }
}

fn list(store: &Store, json: bool, shelf: Shelf) -> Result<(), Error> {
    let (rows, warnings) = store.list_shelf(shelf)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!("no recaps in {}", store.shelf_dir(shelf).display());
    } else {
        for r in rows {
            let ticket = r.ticket.unwrap_or_default();
            println!(
                "{:<28} {:<7} {:<11} {}",
                r.id,
                r.state.to_string(),
                ticket,
                r.title
            );
        }
    }
    Ok(())
}

/// `beagle recap sync`: one poll of every PR; record state changes.
fn sync(store: &Store) -> Result<(), Error> {
    if !poller::gh_available() {
        return Err(Error::Tool {
            tool: "gh",
            message: "not installed or not runnable".to_owned(),
        });
    }
    let (rows, _) = store.list()?;
    let urls: Vec<String> = rows.iter().map(|r| r.url.clone()).collect();
    let statuses = poller::poll_all(&urls);
    let mut changed = 0;
    for r in &rows {
        let Some(s) = statuses.get(&r.url) else {
            println!("{:<28} ?       (couldn't read from GitHub)", r.id);
            continue;
        };
        let moved = store.set_state(&RecapId::new(&r.id)?, s.state, s.updated.as_deref())?;
        if moved && s.state != r.state {
            changed += 1;
            println!("{:<28} {} → {}", r.id, r.state, s.state);
        } else {
            println!("{:<28} {}", r.id, s.state);
        }
    }
    println!("{} PRs checked, {changed} changed state", rows.len());
    for r in &rows {
        if let Some(at) = statuses.get(&r.url).and_then(|s| s.merged_at.as_deref()) {
            store.set_merged_at(&RecapId::new(&r.id)?, at)?;
        }
    }
    let archived =
        store.archive_merged_older_than(store::now_unix(), store::AUTO_ARCHIVE_AFTER_SECS)?;
    if !archived.is_empty() {
        println!(
            "archived (merged over 24h ago, kept in the archive): {}",
            archived.join(", ")
        );
    }
    Ok(())
}

/// `beagle recap merge-check`: the TUI's merge decision, read-only.
fn merge_check(store: &Store, target: &str) -> Result<(), Error> {
    let pr = match PrRef::parse(target) {
        Ok(pr) => pr,
        Err(_) => PrRef::parse(&store.load_any(&RecapId::new(target)?)?.manifest.url)?,
    };
    let url = format!("https://github.com/{}/pull/{}", pr.full_repo(), pr.number);
    match actions::check_merge(&url, &pr.full_repo()) {
        Ok(plan) => {
            let methods: Vec<&str> = plan.methods.iter().map(|m| m.label()).collect();
            println!(
                "{pr}: can {} — methods: {} (default first); head {}",
                if plan.auto {
                    "enable auto-merge"
                } else {
                    "merge now"
                },
                methods.join(", "),
                plan.head
            );
        }
        Err(reason) => println!("{pr}: can't merge — {reason}"),
    }
    Ok(())
}

/// `beagle recap comments sync`: every open/draft PR's threads, written locally.
fn comments_sync(store: &Store) -> Result<(), Error> {
    let config = load_config()?;
    let now = store::now_unix().to_string();
    println!("{}", comments::sync_all(store, &config.comments, &now)?);
    Ok(())
}

/// `beagle recap comments pending`: threads waiting on an agent review.
fn comments_pending(store: &Store, json: bool) -> Result<(), Error> {
    let pending = comments::pending(store, store::now_unix())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&pending)?);
    } else if pending.is_empty() {
        println!("nothing waiting on a review");
    } else {
        for p in &pending {
            println!("{}  {}  {}  ({})", p.recap, p.thread, p.place, p.author);
        }
    }
    Ok(())
}

/// `beagle recap comments review <ID> <THREAD> <FILE>`: install an agent's review.
fn comments_review(store: &Store, target: &str, thread: &str, file: &str) -> Result<(), Error> {
    let recap = store.load(&resolve(target)?)?;
    let body = std::fs::read_to_string(file).map_err(|e| Error::io(file, e))?;
    let author = recap.manifest.author.clone().unwrap_or_default();
    let path = comments::save_review(&store.recap_dir(&recap.id), thread, &author, &body)?;
    println!("{}", path.display());
    Ok(())
}

/// `beagle recap comments <ID|PR>`: the PR's reviews and comments, printed.
fn comments_print(store: &Store, target: &str) -> Result<(), Error> {
    // A PR works even without a recap; an id is looked up.
    let pr = match PrRef::parse(target) {
        Ok(pr) => pr,
        Err(_) => PrRef::parse(&store.load_any(&RecapId::new(target)?)?.manifest.url)?,
    };
    print!("{}", gh::comments(&pr)?);
    Ok(())
}
