//! `extensions install`, `extensions update` and `beagle <ext> update`.

use std::process::Command;
use std::time::Duration;

use super::{spec, target};
use crate::doctor::LEGACY_BINS;
use crate::error::{Error, Result};
use crate::install::{self, swap, Installed, Request};
use crate::picker;
use crate::registry::ExtensionSpec;
use crate::releases::{self, Release};
use crate::state::{self, State};
use crate::version::Version;
use crate::{Context, Out, Outcome};

/// Exit code when the user aborts the picker (like a shell's SIGINT).
const ABORTED: u8 = 130;
/// Non-current versions kept after an install, for offline rollback.
const KEEP: usize = 1;
/// Bound on an extension's post-update hook (e.g. a daemon restart).
const HOOK_TIMEOUT: Duration = Duration::from_secs(120);

/// `beagle extensions install <name>`.
pub fn install(
    ctx: &Context<'_>,
    out: Out<'_>,
    name: &str,
    requested: Option<&str>,
    yes: bool,
    pre: bool,
) -> Result<Outcome> {
    let spec = spec(ctx, name)?;
    let target = supported_target(ctx, spec)?;
    let releases = ctx.source.fetch(spec, pre)?;
    record_check(ctx, spec, &releases, target);
    let Some(release) = pick(ctx, spec, &releases, target, requested, yes)? else {
        eprintln!("aborted; nothing changed");
        return Ok(Outcome::Exit(ABORTED));
    };
    let done = install_release(ctx, spec, release, target)?;
    if done.was_present {
        writeln_out(
            out,
            &format!(
                "{} {} was already installed; it is now current",
                spec.name, done.version
            ),
        )?;
    } else {
        writeln_out(out, &format!("installed {} {}", spec.name, done.version))?;
    }
    note_legacy(ctx);
    Ok(Outcome::Exit(0))
}

/// `beagle extensions update <names>` / `beagle <ext> update`.
pub fn update(
    ctx: &Context<'_>,
    out: Out<'_>,
    names: &[String],
    requested: Option<&str>,
    yes: bool,
) -> Result<Outcome> {
    if names.is_empty() {
        writeln_out(out, "no extensions installed")?;
    }
    for name in names {
        let spec = spec(ctx, name)?;
        let current = ctx
            .layout
            .current_version(spec.name)
            .ok_or_else(|| Error::NotInstalled {
                name: spec.name.to_owned(),
            })?;
        let target = supported_target(ctx, spec)?;
        let releases = ctx.source.fetch(spec, false)?;
        record_check(ctx, spec, &releases, target);
        let Some(release) = pick(ctx, spec, &releases, target, requested, yes)? else {
            eprintln!("aborted; nothing changed");
            return Ok(Outcome::Exit(ABORTED));
        };
        if release.version == current {
            writeln_out(out, &format!("{} is up to date ({current})", spec.name))?;
            continue;
        }
        let done = install_release(ctx, spec, release, target)?;
        writeln_out(out, &format!("{}: {current} → {}", spec.name, done.version))?;
        run_post_update(ctx, spec, &done);
    }
    Ok(Outcome::Exit(0))
}

fn supported_target<'c>(ctx: &'c Context<'_>, spec: &ExtensionSpec) -> Result<&'c str> {
    let target = target(ctx, spec.name)?;
    if spec.supports(target) {
        Ok(target)
    } else {
        Err(Error::UnsupportedTarget {
            name: spec.name.to_owned(),
            target: target.to_owned(),
        })
    }
}

/// The release to install: `requested`, else the picker on a terminal
/// (unless `yes`), else the latest. `None` when the picker is aborted.
fn pick<'r>(
    ctx: &Context<'_>,
    spec: &ExtensionSpec,
    releases: &'r [Release],
    target: &str,
    requested: Option<&str>,
    yes: bool,
) -> Result<Option<&'r Release>> {
    if requested.is_some() || yes || !ctx.interactive {
        let release = releases::choose(releases, spec, target, requested)?;
        if requested.is_none() {
            eprintln!(
                "{}: using the latest release, {}",
                spec.name, release.version
            );
        }
        return Ok(Some(release));
    }
    let candidates = releases::installable(releases, spec, target);
    if candidates.is_empty() {
        return releases::choose(releases, spec, target, None).map(Some);
    }
    let versions: Vec<Version> = candidates.iter().map(|r| r.version).collect();
    let items = picker::items(
        &versions,
        &ctx.layout.installed_versions(spec.name),
        ctx.layout.current_version(spec.name),
    );
    let Some(chosen) = picker::pick_version(spec.name, items)? else {
        return Ok(None);
    };
    Ok(candidates.into_iter().find(|r| r.version == chosen))
}

fn install_release(
    ctx: &Context<'_>,
    spec: &ExtensionSpec,
    release: &Release,
    target: &str,
) -> Result<Installed> {
    let done = install::install(&Request {
        spec,
        release,
        source: &ctx.source,
        target,
        layout: &ctx.layout,
        base_version: Version::current(),
        lock_wait: ctx.lock_wait,
    })?;
    if let Err(e) = swap::prune(&ctx.layout, spec.name, KEEP) {
        eprintln!("warning: could not prune old {} versions: {e}", spec.name);
    }
    Ok(done)
}

/// Refreshes the update-notice cache from a listing we fetched anyway.
fn record_check(ctx: &Context<'_>, spec: &ExtensionSpec, releases: &[Release], target: &str) {
    let path = ctx.layout.state_file();
    let mut cache = State::load(&path);
    let latest = releases::installable(releases, spec, target)
        .first()
        .map(|r| r.version);
    cache.record_check(spec.name, latest, state::now());
    let _ = cache.save(&path); // only a cache
}

/// Runs the extension's declared `post_update` hook (e.g. restarting its
/// daemon onto the new binary). A failing hook is reported, not fatal: the
/// update itself already succeeded.
fn run_post_update(ctx: &Context<'_>, spec: &ExtensionSpec, done: &Installed) {
    if done.manifest.post_update.is_empty() {
        return;
    }
    let bin = ctx.layout.active_bin(spec.name, &done.manifest.bin);
    let mut cmd = Command::new(&bin);
    cmd.args(&done.manifest.post_update);
    if let Err(e) = crate::tool::run_command("post-update hook", cmd, HOOK_TIMEOUT) {
        eprintln!(
            "warning: {} {} post-update hook failed: {e} — run it yourself: beagle {} {}",
            spec.name,
            done.version,
            spec.name,
            done.manifest.post_update.join(" ")
        );
    }
}

/// Mentions binaries the extensions replace, when they are still on `PATH`.
fn note_legacy(ctx: &Context<'_>) {
    for bin in LEGACY_BINS {
        if let Some(found) = beagle_utils::process::which_in(bin, &ctx.path_var) {
            if !found.starts_with(ctx.layout.root()) {
                eprintln!(
                    "note: {} is still on PATH; `beagle doctor` lists old binaries",
                    found.display()
                );
                return;
            }
        }
    }
}

fn writeln_out(out: Out<'_>, line: &str) -> Result<()> {
    writeln!(out, "{line}").map_err(|e| Error::io("<stdout>", e))
}
