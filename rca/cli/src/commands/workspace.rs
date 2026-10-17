//! Workspace subcommands: `list`, `init`, `archive`/`unarchive`,
//! `publish`/`unpublish`, `status`, `context`, and `similar`.

use std::env;
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use beagle::model::{RcaId, RcaSummary, Severity, Status};
use beagle::store::{BrokenWorkspace, Store};
use beagle::{config, Error};
use beagle_utils::paths::expand_tilde;

use crate::{cli, effective_root};

/// `beagle list`: workspaces to stdout, warnings to stderr. With `json`,
/// stdout is a JSON array of the matching workspaces and nothing else, so it
/// pipes straight into `jq`.
pub(crate) fn run_list(
    root: Option<PathBuf>,
    status: Option<Status>,
    severity: Option<Severity>,
    archived: bool,
    json: bool,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let listing = if archived {
        store.list_all()?
    } else {
        store.list()?
    };
    let matched: Vec<&RcaSummary> = listing
        .summaries
        .iter()
        .filter(|rca| status.map_or(true, |s| rca.meta.status == s))
        .filter(|rca| severity.map_or(true, |s| rca.meta.severity == s))
        .collect();

    if json {
        println!("{}", cli::json::list(&matched)?);
        // Broken workspaces have no manifest to render, so they join the
        // warnings on stderr rather than polluting the array.
        for broken in &listing.broken {
            eprintln!("warning: broken `{}`: {}", broken.dir_name, broken.reason);
        }
    } else {
        print_list_table(&matched, &listing.broken);
    }
    for warning in &listing.warnings {
        eprintln!("warning: {}", warning.0);
    }
    Ok(())
}

/// The human-readable `beagle list` table: one fixed-width row per
/// workspace, then a row per directory that failed to load.
fn print_list_table(matched: &[&RcaSummary], broken: &[BrokenWorkspace]) {
    for rca in matched {
        let marker = if rca.archived { "  [archived]" } else { "" };
        println!(
            "{:<10} {:<14} {:<30} {}{marker}",
            rca.meta.severity, rca.meta.status, rca.id, rca.meta.title,
        );
    }
    for broken in broken {
        println!("⚠ broken     {:<30} {}", broken.dir_name, broken.reason);
    }
}

/// `beagle init`: scaffold toolbox.md + systems/ agent context templates.
pub(crate) fn run_init(root: Option<PathBuf>) -> Result<(), Error> {
    let store_root = match root {
        Some(explicit) => explicit,
        None => init_project_root()?,
    };
    let store = Store::open(&store_root)?;
    let created = store.init_context()?;
    if created.is_empty() {
        println!("toolbox.md and systems/ already present; nothing created");
    } else {
        for path in created {
            println!("created {}", path.display());
        }
        println!(
            "fill these in — agents read them before every investigation (T shows them in the TUI)"
        );
    }
    Ok(())
}

/// The root for a bare `beagle init`. An existing `.beagle` up-tree wins
/// (init stays idempotent — no prompt). Interactively, asks where the
/// store root should live (default: the working directory) and writes a
/// `.beagle` in the cwd pinning it, so `beagle` run anywhere under here
/// finds the store from then on. Non-interactive (piped, agents) keeps
/// the plain resolution — scripts never block on a prompt.
fn init_project_root() -> Result<PathBuf, Error> {
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    if let Some(existing) = config::find_project_file(&cwd) {
        println!("using {}", existing.display());
        return effective_root(None);
    }
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        return effective_root(None);
    }

    print!("beagle store root? [{}] ", cwd.display());
    std::io::Write::flush(&mut std::io::stdout()).map_err(|e| Error::io(".", e))?;
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| Error::io(".", e))?;
    let answer = answer.trim();

    let typed = expand_tilde(Path::new(answer));
    let resolved = if typed.is_relative() {
        cwd.join(&typed)
    } else {
        typed.clone()
    };
    let root = if answer.is_empty() || resolved == cwd {
        let path = config::write_project_file(&cwd, None)?;
        println!(
            "created {} — this directory is the beagle store root",
            path.display()
        );
        cwd
    } else {
        // The typed form goes in the file (relative stays relative — it
        // resolves against the `.beagle`'s directory), the resolved form
        // opens the store now.
        let path = config::write_project_file(&cwd, Some(&typed))?;
        println!(
            "created {} — beagle here now uses {}",
            path.display(),
            resolved.display()
        );
        resolved
    };
    Ok(root)
}

/// `beagle archive`: move a finished workspace to `rcas/archive/`.
pub(crate) fn run_archive(root: Option<PathBuf>, id: &RcaId, force: bool) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let dest = store.archive(id, force)?;
    println!("archived {id} → {}", dest.display());
    Ok(())
}

/// `beagle unarchive`: move an archived workspace back to the active list.
pub(crate) fn run_unarchive(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let dest = store.unarchive(id)?;
    println!("unarchived {id} → {}", dest.display());
    Ok(())
}

/// `beagle publish` / `beagle unpublish`: toggle an RCA's public flag.
pub(crate) fn run_set_published(
    root: Option<PathBuf>,
    id: &RcaId,
    published: bool,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    store.set_published(id, published)?;
    if published {
        println!("{id} is now public — rebuild the web app to include it");
    } else {
        println!("{id} is private again");
    }
    Ok(())
}

/// `beagle status <id> <status>`: move an RCA through its lifecycle,
/// stamping `updated`.
pub(crate) fn run_set_status(
    root: Option<PathBuf>,
    id: &RcaId,
    status: Status,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    store.set_status(id, status)?;
    println!("{id}: status → {status}");
    Ok(())
}

/// `beagle context <id>`: print the review bundle (writeup + toolbox +
/// relevant systems) to stdout, for a reviewer or the `/beagle-review`
/// skill to capture.
pub(crate) fn run_context(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    print!("{}", store.review_context(id)?);
    Ok(())
}

/// `beagle similar`: related workspaces, highest score first. Archived
/// incidents stay in the candidate set — the archive *is* the knowledge
/// base this command exists to mine.
pub(crate) fn run_similar(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let summaries = store.list_all()?.summaries;
    let target = summaries
        .iter()
        .find(|rca| rca.id == *id)
        .ok_or_else(|| Error::Tool {
            tool: "similar",
            message: format!("no workspace `{id}` under this root"),
        })?;
    let related = beagle::similar::rank(target, &summaries);
    if related.is_empty() {
        println!("no related incidents (nothing shares systems or tags with {id})");
        return Ok(());
    }
    for entry in &related {
        println!(
            "{:<3} {:<10} {:<14} {:<40} {}",
            entry.score,
            entry.rca.meta.severity,
            entry.rca.meta.status,
            entry.rca.id,
            beagle::similar::shared_label(entry),
        );
    }
    Ok(())
}
