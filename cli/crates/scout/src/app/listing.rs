//! Read-only views: the TUI, `list`, `show` and `path`.

use std::fs;
use std::io::IsTerminal as _;

use crate::model::TaskStatus;
use crate::store::{Project, Store};
use crate::{config, ui, Error, Result};

use super::home;

/// Opens the TUI, or prints the listing when stdout is not a terminal.
pub(super) fn tui(store: Store) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        // Piped: print the listing instead of drawing to a non-terminal.
        return list(&store, false);
    }
    fs::create_dir_all(store.root()).map_err(Error::io(store.root()))?;
    let app = ui::App::new(store)?;
    let mut terminal = ratatui::try_init().map_err(Error::Terminal)?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

/// Applies `[auto] pause_after` before showing anything, so a stalled agent
/// never shows as working. Quiet: changes are logged and notified.
fn sweep_quiet(store: &Store) {
    let (config, _) = config::load();
    crate::autopause::sweep(store, &config, home().ok().as_deref());
}

/// `beagle scout list [--json]`.
pub(super) fn list(store: &Store, json: bool) -> Result<()> {
    sweep_quiet(store);
    let listing = store.list()?;
    for warning in &listing.warnings {
        eprintln!("warning: {warning}");
    }
    if json {
        let rows: Vec<_> = listing.projects.iter().map(Project::list_json).collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if listing.projects.is_empty() {
        println!(
            "no projects in {} — start one with `beagle scout new`",
            store.root().display()
        );
        return Ok(());
    }
    let width = listing
        .projects
        .iter()
        .map(|p| p.id.as_str().len())
        .max()
        .unwrap_or(0);
    for p in &listing.projects {
        let now = p
            .working()
            .map(|t| t.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "{:<width$}  {:<8}  {:>2}/{:<2}  {}{}",
            p.id.as_str(),
            p.meta.status.as_str(),
            p.closed_count(),
            p.tasks.len(),
            p.meta.title,
            if now.is_empty() {
                String::new()
            } else {
                format!("  [working: {now}]")
            },
        );
    }
    Ok(())
}

/// The "where were we?" view an agent reads at the start of a session.
pub(super) fn show(store: &Store, id: &str) -> Result<()> {
    sweep_quiet(store);
    let (p, warnings) = store.load_project(id)?;
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    print_project(&p);
    Ok(())
}

fn print_project(p: &Project) {
    println!("{} ({})", p.meta.title, p.id);
    println!(
        "status: {} · {}/{} tasks closed",
        p.meta.status.as_str(),
        p.closed_count(),
        p.tasks.len()
    );
    if let Some(goal) = p.goal() {
        println!("goal:   {goal}");
    }
    if !p.meta.links.is_empty() {
        println!("links:  {}", p.meta.links.join(", "));
    }
    println!("dir:    {}", p.dir.display());
    println!();
    if p.tasks.is_empty() {
        println!(
            "no tasks yet — add them with `beagle scout task add {} <slug> --title \"...\"`",
            p.id
        );
        return;
    }
    let width = p.tasks.iter().map(|t| t.id.len()).max().unwrap_or(0);
    for t in &p.tasks {
        let mark = match t.meta.status {
            TaskStatus::Todo => "[ ]",
            TaskStatus::Working => "[>]",
            TaskStatus::Paused => "[~]",
            TaskStatus::Blocked => "[!]",
            TaskStatus::Done => "[x]",
            TaskStatus::Skipped => "[-]",
        };
        let (checked, total) = t.checklist;
        let checklist = if total > 0 {
            format!("  ({checked}/{total})")
        } else {
            String::new()
        };
        println!(
            "{mark} {:<width$}  {:<7}  {}{checklist}",
            t.id,
            t.meta.status.as_str(),
            t.meta.title
        );
    }
    // The handoff: what the last task (or the paused one) said to do next.
    if let Some((from, steps)) = p.handoff() {
        println!("\nnext steps (from {}):", from.id);
        for line in steps.lines() {
            println!("  {line}");
        }
    }
}

/// `beagle scout path <project> [<task>]`.
pub(super) fn print_path(store: &Store, project: String, task: Option<String>) -> Result<()> {
    let p = store.project(&project)?;
    let path = match task {
        Some(reference) => p
            .resolve_task(&reference)
            .ok_or(Error::TaskNotFound {
                project,
                task: reference,
            })?
            .path
            .clone(),
        None => p.file(),
    };
    println!("{}", path.display());
    Ok(())
}
