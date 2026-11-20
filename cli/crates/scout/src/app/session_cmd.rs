//! Agent sessions: `task attach`, `session show` and `watch`.

use std::env;
use std::io::IsTerminal as _;
use std::path::PathBuf;

use crate::cli::WatchArgs;
use crate::store::Store;
use crate::{picker, session, transcript, watch, Error, Result};

use super::{claude_session, home, short};

/// `beagle scout task attach`: link a Claude session to a task. The TUI does
/// the rest (finding the transcript, syncing it, rendering the Session tab).
pub(super) fn attach(
    store: &Store,
    project: &str,
    task: &str,
    session: Option<String>,
) -> Result<()> {
    let session = session
        .or_else(claude_session)
        .filter(|s| !s.is_empty())
        .ok_or(Error::NoSession)?;
    let p = store.project(project)?;
    let id = p
        .resolve_task(task)
        .ok_or_else(|| Error::TaskNotFound {
            project: project.to_owned(),
            task: task.to_owned(),
        })?
        .id
        .clone();
    if store.attach_session(project, &id, &session)? {
        println!(
            "{id}: attached session {} — open its Session tab in `beagle scout`",
            short(&session)
        );
    } else {
        println!("{id}: session {} was already attached", short(&session));
    }
    Ok(())
}

/// `beagle scout session show`: run the pipeline for the task's sessions,
/// then print the rendered view (what the TUI's Session tab shows).
/// `--follow` keeps running it and prints what is new.
pub(super) fn show(store: &Store, project: &str, task: &str, follow: bool) -> Result<()> {
    let p = store.project(project)?;
    let t = p
        .resolve_task(task)
        .ok_or_else(|| Error::TaskNotFound {
            project: project.to_owned(),
            task: task.to_owned(),
        })?
        .clone();
    if t.meta.sessions.is_empty() {
        println!(
            "no sessions attached to {} — `beagle scout task attach {project} {}`",
            t.id, t.number
        );
        return Ok(());
    }
    let home = home()?;
    let color = std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none();
    let snapshot = || -> String {
        let mut out = String::new();
        for id in &t.meta.sessions {
            if let Some(src) = transcript::find_transcript(&home, id) {
                // A failed sync leaves the last rendered view in place,
                // which is still the best thing to print.
                let _ = transcript::etl(&src, &p.dir, id, store);
            }
            let view = transcript::view_file(&transcript::cache_file(&p.dir, id), &t.id);
            // Not rendered yet: nothing to print for this session.
            out.push_str(&std::fs::read_to_string(view).unwrap_or_default());
        }
        if color {
            out
        } else {
            beagle_tui::ansi::strip(&out)
        }
    };
    let mut printed = snapshot();
    print!("{printed}");
    if !follow {
        return Ok(());
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let now = snapshot();
        if let Some(tail) = now.strip_prefix(&printed) {
            print!("{tail}");
        } else if now != printed {
            print!("{now}");
        }
        printed = now;
    }
}

/// `beagle scout watch`: pick (or pin, or follow) a recorded session and
/// stream its commands.
pub(super) fn watch(store: &Store, args: WatchArgs) -> Result<()> {
    let tab = env::var("HERDR_TAB_ID").ok().filter(|t| !t.is_empty());
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let session = if let Some(wanted) = args.session {
        match session::resolve(store.root(), &wanted) {
            Ok(id) => Some(id),
            Err(matches) => {
                eprintln!("{wanted:?} matches more than one session:");
                for id in matches {
                    eprintln!("  {id}");
                }
                return Err(Error::AmbiguousSession(wanted));
            }
        }
    } else if args.auto || args.once || !interactive {
        None
    } else {
        // Like `claude --resume`: pick from a list, or "auto" to follow.
        let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("beagle"));
        match picker::choose(store.root(), tab.as_deref(), &exe)? {
            picker::Choice::Auto => None,
            picker::Choice::Session(id) => Some(id),
            picker::Choice::Cancel => return Ok(()),
        }
    };
    watch::run(
        store,
        &watch::Options {
            session,
            history: args.history,
            commands_only: args.commands_only,
            full: args.full,
            color: args.color
                || (std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none()),
            once: args.once,
            tab,
            cwd: env::current_dir().unwrap_or_default(),
        },
    )
}
