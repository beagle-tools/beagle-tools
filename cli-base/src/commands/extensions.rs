//! The read-only and offline `extensions` subcommands: list, available,
//! use, uninstall, prune, path.

use std::fmt::Write as _;

use super::{installed, spec};
use crate::dispatch;
use crate::error::{Error, Result};
use crate::install::swap;
use crate::releases;
use crate::state::{self, State};
use crate::version::Version;
use crate::{Context, Out};

fn line(out: Out<'_>, text: &str) -> Result<()> {
    writeln!(out, "{text}").map_err(|e| Error::io("<stdout>", e))
}

/// `extensions list`: installed extensions, their versions, and whether an
/// update is available (checked live; the cached result when offline).
pub fn list(ctx: &Context<'_>, out: Out<'_>) -> Result<()> {
    let active = installed(ctx);
    if active.is_empty() {
        return line(
            out,
            "no extensions installed — see: beagle extensions available",
        );
    }
    let path = ctx.layout.state_file();
    let mut cache = State::load(&path);
    for (spec, current) in active {
        let others: Vec<String> = ctx
            .layout
            .installed_versions(spec.name)
            .into_iter()
            .filter(|v| *v != current)
            .map(|v| v.to_string())
            .collect();
        let mut row = format!("{:<10} {current} (current)", spec.name);
        if !others.is_empty() {
            let _ = write!(row, "  also installed: {}", others.join(", "));
        }
        let latest = match (ctx.target.as_deref(), ctx.source.fetch(spec, false)) {
            (Some(target), Ok(found)) => {
                let latest = releases::installable(&found, spec, target)
                    .first()
                    .map(|r| r.version);
                cache.record_check(spec.name, latest, state::now());
                latest
            }
            _ => cache.ext.get(spec.name).and_then(|e| e.latest),
        };
        if let Some(latest) = latest.filter(|l| *l > current) {
            let _ = write!(
                row,
                "  update available: {latest} (beagle {} update)",
                spec.name
            );
        }
        line(out, &row)?;
    }
    let _ = cache.save(&path); // only a cache
    Ok(())
}

/// `extensions available`: every registry entry and its latest release
/// for this platform.
pub fn available(ctx: &Context<'_>, out: Out<'_>, pre: bool) -> Result<()> {
    let path = ctx.layout.state_file();
    let mut cache = State::load(&path);
    let target = ctx.target.as_deref();
    let mut rows = Vec::new();
    for spec in ctx.registry {
        let status = match target {
            None => "not available for this platform".to_owned(),
            Some(t) if !spec.supports(t) => format!("not available for {t}"),
            Some(t) => match ctx.source.fetch(spec, pre) {
                Ok(found) => {
                    let latest = releases::installable(&found, spec, t)
                        .first()
                        .map(|r| r.version);
                    cache.record_check(spec.name, latest, state::now());
                    latest.map_or_else(|| format!("not released for {t} yet"), |v| v.to_string())
                }
                Err(e) => format!("could not check ({e})"),
            },
        };
        let marker = if ctx.layout.current_version(spec.name).is_some() {
            '●'
        } else {
            ' '
        };
        rows.push((marker, spec, status));
    }
    let width = rows
        .iter()
        .map(|(_, _, s)| s.chars().count())
        .max()
        .unwrap_or(0);
    for (marker, spec, status) in rows {
        let row = format!(
            "{marker} {:<10} {status:<width$}  {}",
            spec.name, spec.description
        );
        line(out, &row)?;
    }
    let _ = cache.save(&path); // only a cache
    Ok(())
}

/// `extensions use <name> <version>`: switch to an installed version.
pub fn use_version(ctx: &Context<'_>, out: Out<'_>, name: &str, version: &str) -> Result<()> {
    let spec = spec(ctx, name)?;
    let version = Version::parse_for(version, spec.tag_prefix)?;
    swap::use_version(&ctx.layout, spec.name, version)?;
    line(out, &format!("{} {version} is now current", spec.name))
}

/// `extensions uninstall <name> [--all-versions]`.
pub fn uninstall(ctx: &Context<'_>, out: Out<'_>, name: &str, all: bool) -> Result<()> {
    let spec = spec(ctx, name)?;
    let left = swap::uninstall(&ctx.layout, spec.name, all)?;
    if left.is_empty() {
        return line(out, &format!("uninstalled {}", spec.name));
    }
    let list: Vec<String> = left.iter().map(ToString::to_string).collect();
    line(
        out,
        &format!(
            "uninstalled the current {name}; still on disk: {} — `beagle extensions use {name} <version>` or `--all-versions`",
            list.join(", "),
            name = spec.name
        ),
    )
}

/// `extensions prune [<name>] [--keep <n>]`.
pub fn prune(ctx: &Context<'_>, out: Out<'_>, name: Option<&str>, keep: usize) -> Result<()> {
    let specs = match name {
        Some(name) => vec![spec(ctx, name)?],
        None => ctx.registry.iter().collect(),
    };
    let mut removed_any = false;
    for spec in specs {
        let removed = swap::prune(&ctx.layout, spec.name, keep)?;
        for version in removed {
            removed_any = true;
            line(out, &format!("removed {} {version}", spec.name))?;
        }
    }
    if !removed_any {
        line(out, "nothing to prune")?;
    }
    Ok(())
}

/// `extensions path <name>`: the active binary.
pub fn path(ctx: &Context<'_>, out: Out<'_>, name: &str) -> Result<()> {
    let spec = spec(ctx, name)?;
    let resolved = dispatch::installed(spec, &ctx.layout)?;
    line(out, &resolved.bin().display().to_string())
}
