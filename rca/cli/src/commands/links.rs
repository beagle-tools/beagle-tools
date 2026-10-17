//! Attached links: `beagle pr add|list` and `beagle ticket add|list|sync`.

use std::path::PathBuf;

use beagle::model::RcaId;
use beagle::store::Store;
use beagle::Error;

use crate::{effective_config, effective_root};

/// `beagle pr add <id> <url>`: attach a remediation PR.
pub(crate) fn run_pr_add(root: Option<PathBuf>, id: &RcaId, url: &str) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    if store.add_pr(id, url)? {
        println!("attached {url} to {id}");
    } else {
        println!("{url} is already attached to {id}");
    }
    Ok(())
}

/// `beagle pr list`: attached PRs, with live state when `gh` works.
pub(crate) fn run_pr_list(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let meta = store.read_meta(id)?;
    if meta.prs.is_empty() {
        println!("no PRs attached to {id} (use `beagle pr add {id} <url>`)");
        return Ok(());
    }
    let gh = beagle::prs::gh_available();
    for url in &meta.prs {
        match gh.then(|| beagle::prs::state_of(url)).flatten() {
            Some(state) => println!("{} {:<8} {url}", state.glyph(), state.label()),
            None => println!("  {:<8} {url}", "-"),
        }
    }
    Ok(())
}

/// `beagle ticket add <id> <ref>`: attach a Linear/GitHub issue.
pub(crate) fn run_ticket_add(
    root: Option<PathBuf>,
    id: &RcaId,
    reference: &str,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    if store.add_ticket(id, reference)? {
        println!("attached {reference} to {id}");
    } else {
        println!("{reference} is already attached to {id}");
    }
    Ok(())
}

/// `beagle ticket list <id>`: the attached tickets, one per line.
pub(crate) fn run_ticket_list(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let tickets = store.read_meta(id)?.tickets;
    if tickets.is_empty() {
        println!("no tickets attached to {id} (use `beagle ticket add {id} <ref>`)");
    } else {
        for t in tickets {
            println!("{t}");
        }
    }
    Ok(())
}

/// `beagle ticket sync [<id>]`: for each attached ticket, discover its
/// linked PRs and attach any new ones to the RCA (idempotent). With no id,
/// syncs every workspace. Per-ticket failures are reported but don't abort
/// the run.
pub(crate) fn run_ticket_sync(root: Option<PathBuf>, id: Option<&RcaId>) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let tickets_cfg = effective_config()?.and_then(|c| c.tickets);
    let ids: Vec<RcaId> = match id {
        Some(id) => vec![id.clone()],
        None => store
            .list_all()?
            .summaries
            .into_iter()
            .map(|s| s.id)
            .collect(),
    };
    let mut attached = 0usize;
    for id in &ids {
        let meta = store.read_meta(id)?;
        for ticket in &meta.tickets {
            match beagle::tickets::linked_prs(ticket, tickets_cfg.as_ref()) {
                Ok(prs) => {
                    for pr in prs {
                        match store.add_pr(id, &pr) {
                            Ok(true) => {
                                println!("{id}: attached {pr} (from {ticket})");
                                attached += 1;
                            }
                            Ok(false) => {} // already attached — ignore
                            Err(e) => eprintln!("{id}: could not attach {pr}: {e}"),
                        }
                    }
                }
                Err(e) => eprintln!("{id}: {ticket}: {e}"),
            }
        }
    }
    println!(
        "done — {attached} PR(s) newly attached across {} workspace(s)",
        ids.len()
    );
    Ok(())
}
