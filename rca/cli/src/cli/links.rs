//! Parsers for the link-attaching subcommands: `pr` and `ticket`.

use std::path::PathBuf;

use beagle::model::RcaId;

use super::{take_value, Command};

/// `ticket add <id> <ref>` / `ticket list <id>` / `ticket sync [<id>]`.
pub(super) fn parse_ticket(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let action = args
        .next()
        .ok_or("`ticket` requires a subcommand: `add`, `list`, or `sync`")?;
    if !matches!(action.as_str(), "add" | "list" | "sync") {
        return Err(format!(
            "unknown `ticket` subcommand `{action}` (expected add|list|sync)"
        ));
    }
    // `sync` takes an *optional* id; add/list require one.
    let id_raw = args.next().filter(|a| !a.starts_with('-'));
    if action != "sync" && id_raw.is_none() {
        return Err(format!("`ticket {action}` requires an <id> slug"));
    }
    let id = id_raw
        .map(RcaId::new)
        .transpose()
        .map_err(|e| e.to_string())?;
    let reference = if action == "add" {
        Some(
            args.next()
                .filter(|a| !a.starts_with('-'))
                .ok_or("`ticket add` requires a <ref> (URL or id) after the <id>")?,
        )
    } else {
        None
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `ticket {action}`")),
        }
    }
    Ok(match action.as_str() {
        "add" => Command::TicketAdd {
            root,
            // add always has an id and a reference (checked above).
            id: id.ok_or("`ticket add` requires an <id>")?,
            reference: reference.ok_or("`ticket add` requires a <ref>")?,
        },
        "list" => Command::TicketList {
            root,
            id: id.ok_or("`ticket list` requires an <id>")?,
        },
        _ => Command::TicketSync { root, id },
    })
}

/// `pr add <id> <url>` / `pr list <id>`.
pub(super) fn parse_pr(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let action = args
        .next()
        .ok_or("`pr` requires a subcommand: `add` or `list`")?;
    if action != "add" && action != "list" {
        return Err(format!(
            "unknown `pr` subcommand `{action}` (expected add|list)"
        ));
    }
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| format!("`pr {action}` requires an <id> slug"))?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let url = if action == "add" {
        Some(
            args.next()
                .filter(|a| !a.starts_with('-'))
                .ok_or("`pr add` requires a <url> after the <id>")?,
        )
    } else {
        None
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `pr {action}`")),
        }
    }
    match url {
        // `url` is Some exactly when the action is `add`.
        Some(url) => Ok(Command::PrAdd { root, id, url }),
        None => Ok(Command::PrList { root, id }),
    }
}

#[cfg(test)]
#[path = "tests/parse_links.rs"]
mod tests;
