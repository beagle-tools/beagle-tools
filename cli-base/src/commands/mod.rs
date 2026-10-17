//! Running a parsed command against a [`Context`].
//!
//! Submodules: `extensions` (list/available/use/uninstall/prune/path),
//! `install` (install + update, with the picker), `self_update`.

mod extensions;
mod install;
mod self_update;

use std::ffi::OsString;

use crate::cli::{self, Command, ExtCommand, USAGE};
use crate::dispatch::{self, Resolved};
use crate::doctor;
use crate::error::{Error, Result};
use crate::install::MANIFEST;
use crate::manifest::ExtensionManifest;
use crate::registry::{self, ExtensionSpec};
use crate::state::{self, State};
use crate::version::Version;
use crate::{Context, Dispatch, Out, Outcome};

/// Parses `args` (argv without the program name) and runs the command.
/// Output goes to `out`; progress and warnings go to stderr.
pub fn run(args: Vec<OsString>, ctx: &Context<'_>, out: Out<'_>) -> Result<Outcome> {
    let command = cli::parse(args, ctx.registry).map_err(Error::Usage)?;
    let wrote = |r: std::io::Result<()>| r.map_err(|e| Error::io("<stdout>", e));
    match command {
        Command::Help { ext: None } => {
            wrote(writeln!(out, "{USAGE}\n\n{}", installed_summary(ctx)))?;
            Ok(Outcome::Exit(0))
        }
        Command::Help { ext: Some(name) } => dispatch_to(ctx, &name, &["--help".into()]),
        Command::Version => {
            let target = ctx.target.as_deref().unwrap_or("unsupported platform");
            wrote(writeln!(out, "beagle {} ({target})", Version::current()))?;
            for (spec, version) in installed(ctx) {
                wrote(writeln!(out, "  {} {version}", spec.name))?;
            }
            Ok(Outcome::Exit(0))
        }
        Command::Doctor => {
            let checks = doctor::run(&doctor::Inputs {
                layout: &ctx.layout,
                registry: ctx.registry,
                path_var: &ctx.path_var,
                exe: &ctx.exe,
                home: ctx.home.as_deref(),
            });
            for check in &checks {
                wrote(writeln!(out, "{}  {}", check.status, check.message))?;
            }
            let failed = checks.iter().any(|c| c.status == doctor::Status::Fail);
            Ok(Outcome::Exit(u8::from(failed)))
        }
        Command::SelfUpdate { version, all } => {
            self_update::run(ctx, out, version.as_deref())?;
            if all {
                let names: Vec<String> = installed(ctx)
                    .into_iter()
                    .map(|(s, _)| s.name.to_owned())
                    .collect();
                install::update(ctx, out, &names, None, true)?;
            }
            Ok(Outcome::Exit(0))
        }
        Command::Ext(sub) => run_ext(ctx, out, sub),
        Command::Run { name, args } => dispatch_to(ctx, &name, &args),
    }
}

fn run_ext(ctx: &Context<'_>, out: Out<'_>, sub: ExtCommand) -> Result<Outcome> {
    match sub {
        ExtCommand::List => extensions::list(ctx, out),
        ExtCommand::Available { pre } => extensions::available(ctx, out, pre),
        ExtCommand::Install {
            name,
            version,
            yes,
            pre,
        } => {
            return install::install(ctx, out, &name, version.as_deref(), yes, pre);
        }
        ExtCommand::Update {
            names,
            all,
            version,
            yes,
        } => {
            let names = if all {
                installed(ctx)
                    .into_iter()
                    .map(|(s, _)| s.name.to_owned())
                    .collect()
            } else {
                names
            };
            return install::update(ctx, out, &names, version.as_deref(), yes || all);
        }
        ExtCommand::Use { name, version } => extensions::use_version(ctx, out, &name, &version),
        ExtCommand::Uninstall { name, all_versions } => {
            extensions::uninstall(ctx, out, &name, all_versions)
        }
        ExtCommand::Prune { name, keep } => extensions::prune(ctx, out, name.as_deref(), keep),
        ExtCommand::Path { name } => extensions::path(ctx, out, &name),
    }?;
    Ok(Outcome::Exit(0))
}

/// Resolves `name` and prepares the exec, printing a cached "update
/// available" notice first when one is due.
fn dispatch_to(ctx: &Context<'_>, name: &str, args: &[OsString]) -> Result<Outcome> {
    let resolved = dispatch::resolve(name, ctx.registry, &ctx.layout, &ctx.path_var)?;
    if let Resolved::Installed {
        name: canonical,
        version,
        ..
    } = &resolved
    {
        if ctx.notices {
            notice(ctx, canonical, *version);
        }
    }
    let command = dispatch::command(&resolved, args, &ctx.exe, name);
    Ok(Outcome::Exec(Box::new(Dispatch { command, resolved })))
}

fn notice(ctx: &Context<'_>, name: &str, active: Version) {
    let path = ctx.layout.state_file();
    let mut cache = State::load(&path);
    let now = state::now();
    if let Some(latest) = cache.due_notice(name, active, now) {
        eprintln!(
            "beagle: {name} {latest} is available (you have {active}) — run: beagle {name} update"
        );
        cache.mark_notified(name, now);
        let _ = cache.save(&path); // a cache; failing to save only repeats the notice
    }
}

/// Registry extensions with an active version, in registry order.
fn installed<'r>(ctx: &Context<'r>) -> Vec<(&'r ExtensionSpec, Version)> {
    ctx.registry
        .iter()
        .filter_map(|spec| Some((spec, ctx.layout.current_version(spec.name)?)))
        .collect()
}

/// The "installed extensions" block of `beagle --help`.
fn installed_summary(ctx: &Context<'_>) -> String {
    let rows: Vec<String> = installed(ctx)
        .into_iter()
        .map(|(spec, version)| {
            let manifest = ctx.layout.version_dir(spec.name, version).join(MANIFEST);
            let description = ExtensionManifest::load(&manifest)
                .map_or_else(|_| spec.description.to_owned(), |m| m.description);
            format!(
                "    {:<10} {:<10} {description}",
                spec.name,
                version.to_string()
            )
        })
        .collect();
    if rows.is_empty() {
        "INSTALLED EXTENSIONS: none — see: beagle extensions available".to_owned()
    } else {
        format!("INSTALLED EXTENSIONS:\n{}", rows.join("\n"))
    }
}

/// Looks up `name` for an extensions command: reserved names and unknown
/// names are distinct errors.
fn spec<'r>(ctx: &Context<'r>, name: &str) -> Result<&'r ExtensionSpec> {
    if registry::is_reserved(name) {
        return Err(Error::Reserved {
            name: name.to_owned(),
        });
    }
    registry::find(ctx.registry, name).ok_or_else(|| Error::UnknownExtension {
        name: name.to_owned(),
    })
}

/// This platform's target, or an "unsupported" error naming `name`.
fn target<'c>(ctx: &'c Context<'_>, name: &str) -> Result<&'c str> {
    ctx.target
        .as_deref()
        .ok_or_else(|| Error::UnsupportedTarget {
            name: name.to_owned(),
            target: std::env::consts::OS.to_owned() + "/" + std::env::consts::ARCH,
        })
}
