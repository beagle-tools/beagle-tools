//! `beagle update` and `beagle version [list]`: keeping the one `beagle`
//! binary current.
//!
//! Releases are the `beagle-cli-vX.Y.Z` tags of the monorepo, each with a
//! `beagle-cli-<target>.tar.gz` (+ `.sha256`) holding `beagle`. The
//! download → verify → atomic swap and the picker are shared with rca
//! (`beagle_utils::update`, `beagle_tui::version_picker`). What "update"
//! means depends on how this copy was installed ([`InstallKind`]): Homebrew
//! owns a Cellar binary, a source build is rebuilt, a release tarball is
//! swapped in place.

mod install_kind;

use std::ffi::OsString;
use std::io::IsTerminal as _;
use std::path::Path;
use std::process::ExitCode;

use beagle_utils::update::{Channel, Version};

pub use install_kind::InstallKind;
use install_kind::{source_update_hint, BREW_FORMULA};

/// The repository URL from `Cargo.toml`; a fork updates from the fork.
const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

const UPDATE_HELP: &str = "\
Usage: beagle update [--version <X.Y.Z>] [--check]

Updates this beagle (every tool in it) to the latest release, or to
--version, older or newer.

  --check            show the installed and latest versions, change nothing
  --version <ver>    install exactly this release (X.Y.Z, vX.Y.Z or the tag)

Homebrew installs run `brew upgrade beagle-tools/tap/beagle`. Source builds
print the cargo command that rebuilds them (unless --version is given,
which installs that release binary over this one).";

/// The unified CLI's release channel.
fn channel() -> Channel {
    Channel {
        repo: REPO_URL
            .strip_prefix("https://github.com/")
            .unwrap_or(REPO_URL),
        tag_prefix: "beagle-cli-v",
        asset_prefix: "beagle-cli",
        binary: "beagle",
        // Swapped too when installed beside `beagle` (brew, tarball).
        companions: &["beagle-agentd"],
    }
}

/// The version this binary was built as.
fn current() -> Version {
    Version::from_pkg(env!("CARGO_PKG_VERSION"))
}

/// What `beagle update` was asked to do.
#[derive(Debug, PartialEq, Eq)]
enum UpdateRequest {
    /// Print [`UPDATE_HELP`].
    Help,
    /// Report versions only.
    Check,
    /// Install `Some(version)`, or the latest release.
    Install(Option<Version>),
}

fn parse_update(args: &[OsString]) -> Result<UpdateRequest, String> {
    let mut check = false;
    let mut version = None;
    let mut args = args.iter().map(|a| a.to_string_lossy());
    while let Some(arg) = args.next() {
        match arg.as_ref() {
            "-h" | "--help" => return Ok(UpdateRequest::Help),
            "--check" => check = true,
            "--version" => {
                let value = args.next().ok_or("--version requires a value")?;
                version = Some(channel().parse_version(&value)?);
            }
            other => return Err(format!("unknown argument `{other}` for `update`")),
        }
    }
    match (check, version) {
        (true, Some(_)) => Err("--check and --version don't go together".to_owned()),
        (true, None) => Ok(UpdateRequest::Check),
        (false, version) => Ok(UpdateRequest::Install(version)),
    }
}

/// `beagle update …` (the arguments after `update`).
pub fn run_update(args: &[OsString]) -> ExitCode {
    let request = match parse_update(args) {
        Ok(UpdateRequest::Help) => {
            println!("{UPDATE_HELP}");
            return ExitCode::SUCCESS;
        }
        Ok(request) => request,
        Err(message) => {
            eprintln!("beagle: {message}\n\n{UPDATE_HELP}");
            return ExitCode::from(2);
        }
    };
    report(current_exe().and_then(|exe| update(&request, &exe)))
}

/// `beagle version` / `beagle version list` (the arguments after `version`).
pub fn run_version(args: &[OsString]) -> ExitCode {
    match args.first().map(|a| a.to_string_lossy()).as_deref() {
        None => {
            print_version();
            ExitCode::SUCCESS
        }
        Some("list") if args.len() == 1 => report(current_exe().and_then(|exe| list(&exe))),
        Some(other) => {
            eprintln!("beagle: unknown `version` argument `{other}` (expected `list`)");
            ExitCode::from(2)
        }
    }
}

/// Prints `beagle X.Y.Z (<target> | source build)`.
pub fn print_version() {
    let kind = current_exe().map_or(InstallKind::Source, |exe| InstallKind::detect(&exe));
    println!("beagle {} ({})", current(), kind.describe());
}

fn report(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("beagle: {message}");
            ExitCode::FAILURE
        }
    }
}

fn current_exe() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("can't locate the running beagle binary: {e}"))
}

fn update(request: &UpdateRequest, exe: &Path) -> Result<(), String> {
    let kind = InstallKind::detect(exe);
    match (request, kind) {
        (UpdateRequest::Help, _) => {
            println!("{UPDATE_HELP}");
            Ok(())
        }
        (UpdateRequest::Check, _) => check(kind),
        (UpdateRequest::Install(None), InstallKind::Homebrew) => brew_upgrade(),
        (UpdateRequest::Install(Some(_)), InstallKind::Homebrew) => Err(format!(
            "this beagle is managed by Homebrew, which installs the formula's version: \
             run `brew upgrade {BREW_FORMULA}` (or install a release tarball to pin one)"
        )),
        (UpdateRequest::Install(None), InstallKind::Source) => {
            println!("{}", source_update_hint(REPO_URL));
            Ok(())
        }
        (UpdateRequest::Install(version), InstallKind::Release | InstallKind::Source) => {
            install(*version, exe)
        }
    }
}

/// Every published `beagle-cli` release, newest first.
fn releases() -> Result<Vec<Version>, String> {
    channel()
        .fetch_releases()
        .map_err(|e| format!("could not list releases: {e}"))
}

fn no_release_yet() -> String {
    format!(
        "no beagle release has been published yet (looking for `beagle-cli-v*` tags at {}); \
         you're on {}",
        channel().releases_page(),
        current()
    )
}

fn check(kind: InstallKind) -> Result<(), String> {
    let releases = releases()?;
    let current = current();
    println!("installed: beagle {current} ({})", kind.describe());
    match releases.first() {
        None => println!(
            "latest:    none — no `beagle-cli-v*` release is published yet ({})",
            channel().releases_page()
        ),
        Some(latest) if *latest > current => {
            println!("latest:    {latest} — run `beagle update` to install it");
        }
        Some(latest) => println!("latest:    {latest} — up to date"),
    }
    Ok(())
}

fn install(version: Option<Version>, exe: &Path) -> Result<(), String> {
    let version = if let Some(version) = version {
        version
    } else if let Some(latest) = releases()?.first() {
        *latest
    } else {
        println!("{}", no_release_yet());
        return Ok(());
    };
    let current = current();
    if version == current {
        println!("already on beagle {current}; nothing to do");
        return Ok(());
    }
    let verb = if version > current {
        "updating"
    } else {
        "downgrading"
    };
    println!("{verb} beagle {current} → {version} …");
    channel()
        .update_to(version, exe)
        .map_err(|e| format!("update failed: {e}"))?;
    println!("✓ beagle {version} installed at {}", exe.display());
    offer_skill_refresh(exe);
    Ok(())
}

/// After an update, offers to refresh every tool's agent skills: runs the
/// NEW binary's `beagle setup --only rca-skills,scout-skills` (this process
/// still holds the old skills), which asks per step on a terminal. Piped
/// output only prints the command. Best effort: never fails the update.
fn offer_skill_refresh(exe: &Path) {
    let command = "beagle setup --only rca-skills,scout-skills";
    if !std::io::stdin().is_terminal() {
        println!("  `{command}` refreshes the agent skills it bundles");
        return;
    }
    println!("  refreshing the agent skills it bundles (`{command}`) …");
    let ran = beagle_utils::process::status_with_timeout(
        std::process::Command::new(exe).args(["setup", "--only", "rca-skills,scout-skills"]),
        std::time::Duration::from_secs(300),
    );
    if !ran.is_ok_and(|s| s.success()) {
        println!("  skills not refreshed — run `{command}`");
    }
}

fn brew_upgrade() -> Result<(), String> {
    println!("beagle is managed by Homebrew; running `brew upgrade {BREW_FORMULA}` …");
    // Stdio stays inherited so the user sees brew's progress; a brew that
    // hangs (a stuck download or lock) is killed after 10 minutes.
    let status = beagle_utils::process::status_with_timeout(
        std::process::Command::new("brew").args(["upgrade", BREW_FORMULA]),
        std::time::Duration::from_secs(600),
    )
    .map_err(|e| format!("couldn't run brew: {e}"))?;
    if status.success() {
        // brew replaced the Cellar binary; the `beagle` on PATH is the new one.
        if let Some(beagle) = beagle_utils::process::which("beagle") {
            offer_skill_refresh(&beagle);
        }
        Ok(())
    } else {
        Err(format!(
            "`brew upgrade {BREW_FORMULA}` exited with {status}"
        ))
    }
}

/// `beagle version list`: the picker on a terminal (enter installs), a
/// plain listing when piped or when Homebrew owns the binary.
fn list(exe: &Path) -> Result<(), String> {
    let releases = releases()?;
    if releases.is_empty() {
        println!("{}", no_release_yet());
        return Ok(());
    }
    let current = current();
    let channel = channel();
    let kind = InstallKind::detect(exe);
    if kind == InstallKind::Homebrew || !std::io::stdout().is_terminal() {
        print!(
            "{}",
            beagle_tui::version_picker::plain_listing(&channel, &releases, current)
        );
        if kind == InstallKind::Homebrew {
            println!("(managed by Homebrew: `brew upgrade {BREW_FORMULA}` updates it)");
        }
        return Ok(());
    }
    let picked = beagle_tui::version_picker::pick_version("beagle", &channel, &releases, current)
        .map_err(|e| format!("terminal error: {e}"))?;
    match picked {
        Some(version) => install(Some(version), exe),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
