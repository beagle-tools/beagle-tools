//! `beagle update`: replace this binary with another release of the base.
//!
//! Same staged, verified process as an extension install; the final step
//! renames the new binary over this one from a sibling file, so the swap is
//! atomic and an interrupted update leaves the old binary working.

use std::fs;
use std::path::{Component, Path};

use super::target;
use crate::error::{Error, Result};
use crate::install::{download, extract, is_executable_file, lock};
use crate::registry::BASE;
use crate::releases;
use crate::version::Version;
use crate::{Context, Out};

/// Updates the base to `requested`, or the latest release.
pub fn run(ctx: &Context<'_>, out: Out<'_>, requested: Option<&str>) -> Result<()> {
    let exe = fs::canonicalize(&ctx.exe).unwrap_or_else(|_| ctx.exe.clone());
    if exe
        .components()
        .any(|c| c == Component::Normal("Cellar".as_ref()))
    {
        return Err(Error::ManagedByHomebrew);
    }
    let target = target(ctx, BASE.name)?;
    let releases = ctx.source.fetch(&BASE, false)?;
    let release = releases::choose(&releases, &BASE, target, requested)?;
    let current = Version::current();
    if release.version == current {
        return writeln!(out, "beagle is up to date ({current})")
            .map_err(|e| Error::io("<stdout>", e));
    }

    let _lock = lock::acquire(&ctx.layout.lock_file(BASE.name), BASE.name, ctx.lock_wait)?;
    let staging = ctx
        .layout
        .root()
        .join(format!(".staging-self-{}", std::process::id()));
    fs::create_dir_all(&staging).map_err(|e| Error::io(&staging, e))?;
    let result = stage_and_swap(ctx, release, target, &staging, &exe);
    let _ = fs::remove_dir_all(&staging); // best effort
    result?;
    writeln!(out, "beagle: {current} → {}", release.version).map_err(|e| Error::io("<stdout>", e))
}

fn stage_and_swap(
    ctx: &Context<'_>,
    release: &releases::Release,
    target: &str,
    staging: &Path,
    exe: &Path,
) -> Result<()> {
    let asset = BASE.asset_name(target);
    let url = ctx.source.asset_url(&release.tag, &asset);
    let tarball = staging.join(&asset);
    let sha_file = staging.join(format!("{asset}.sha256"));
    download::download(&url, &tarball)?;
    download::download(&format!("{url}.sha256"), &sha_file)?;
    download::verify(&tarball, &sha_file, &asset)?;
    let unpacked = staging.join("unpacked");
    fs::create_dir_all(&unpacked).map_err(|e| Error::io(&unpacked, e))?;
    extract::extract(&tarball, &unpacked)?;
    let new_bin = unpacked.join("beagle");
    if !is_executable_file(&new_bin) {
        return Err(Error::Tool {
            tool: "tar",
            message: format!("{asset} has no executable `beagle` at its root"),
        });
    }
    // Stage beside the destination so the rename stays on one filesystem;
    // fs::copy carries the executable bit over.
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let side = dir.join(format!(".beagle.update-{}", std::process::id()));
    fs::copy(&new_bin, &side).map_err(|e| Error::io(&side, e))?;
    fs::rename(&side, exe).map_err(|e| {
        let _ = fs::remove_file(&side);
        Error::io(exe, e)
    })
}
