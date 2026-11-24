//! `beagle setup`: every tool's one-time install steps, in one command.
//!
//! Each step is a tool command a user would otherwise have to remember
//! (`beagle rca skill install`, `beagle scout hook install`, …). Steps that
//! are already in place are reported and skipped; every other step is
//! asked about (`[Y/n]`), or run straight away with `--yes`. `--skip <id>`
//! and `--only <id,…>` pick steps by id. Nothing here writes a file
//! itself: each step runs the tool's own command, so `beagle setup` does
//! exactly what the documented commands do.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::Path;
use std::process::ExitCode;

use beagle_rca::skill as rca_skill;
use beagle_scout::skill as scout_skill;

const HELP: &str = "\
Usage: beagle setup [--yes] [--skip <id>]… [--only <id>[,<id>…]]

Runs every tool's one-time install steps. Steps already in place are
skipped; each other step asks first (or runs with --yes).

  -y, --yes          run every step without asking
  --skip <id>        leave this step out (repeatable)
  --only <ids>       run only these steps (comma-separated)

Steps:";

/// One install step.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    /// What `--skip` / `--only` take.
    pub id: &'static str,
    /// One line for the prompt and the help.
    pub what: &'static str,
    /// The tool whose command runs it.
    pub tool: &'static str,
    /// That command's arguments (after the tool name), for this home.
    args: fn(&Path) -> Vec<String>,
    /// Whether it is already in place for this home.
    done: fn(&Path) -> bool,
}

impl Step {
    /// The command, as a user would type it.
    #[must_use]
    pub fn command(&self, home: &Path) -> String {
        format!("beagle {} {}", self.tool, (self.args)(home).join(" "))
    }

    /// Whether the step is already in place for `home`.
    #[must_use]
    pub fn is_done(&self, home: &Path) -> bool {
        (self.done)(home)
    }
}

/// Every step, in the order they run.
pub const STEPS: &[Step] = &[
    Step {
        id: "rca-skills",
        what: "install the /beagle and /beagle-review agent skills",
        tool: "rca",
        args: |_| vec!["skill".into(), "install".into()],
        done: rca_skills_current,
    },
    Step {
        id: "scout-skills",
        what: "install the /scout and /scout-build agent skills (and, on macOS, Scout's notifier)",
        tool: "scout",
        args: |_| vec!["skill".into(), "install".into()],
        done: scout_skills_current,
    },
    Step {
        id: "scout-hooks",
        what: "record Claude Code sessions for `beagle scout watch` (adds hooks to ~/.claude/settings.json)",
        tool: "scout",
        args: |_| vec!["hook".into(), "install".into()],
        done: scout_hooks_installed,
    },
    Step {
        id: "scout-store",
        what: "create the default scout store at ~/.scout",
        tool: "scout",
        args: |home| vec!["init".into(), home.join(".scout").display().to_string()],
        done: scout_store_configured,
    },
];

/// Whether every rca skill is current (or managed by a symlink) for each
/// agent `beagle rca skill install` would write to.
#[must_use]
pub fn rca_skills_current(home: &Path) -> bool {
    let detected: Vec<rca_skill::Agent> = rca_skill::Agent::ALL
        .into_iter()
        .filter(|a| a.is_present(home))
        .collect();
    let agents = if detected.is_empty() {
        rca_skill::Agent::ALL.to_vec()
    } else {
        detected
    };
    agents.into_iter().all(|agent| {
        rca_skill::Skill::ALL.into_iter().all(|skill| {
            matches!(
                rca_skill::status(skill, agent, home),
                rca_skill::SkillStatus::Current | rca_skill::SkillStatus::Linked
            )
        })
    })
}

/// Whether every scout skill is current for every agent.
#[must_use]
pub fn scout_skills_current(home: &Path) -> bool {
    scout_skill::Agent::ALL.into_iter().all(|agent| {
        scout_skill::Skill::ALL
            .into_iter()
            .all(|skill| agent.status(skill, home) == scout_skill::SkillStatus::Current)
    })
}

/// Whether every scout hook is in Claude Code's settings.
#[must_use]
pub fn scout_hooks_installed(home: &Path) -> bool {
    beagle_scout::hooks::load(&beagle_scout::hooks::settings_path(home)).is_ok_and(|settings| {
        beagle_scout::hooks::status(&settings)
            .iter()
            .all(|(_, command)| command.is_some())
    })
}

/// Whether a scout store is already chosen: `$SCOUT_ROOT`, a `root` in
/// scout's config, or an initialised `~/.scout`.
fn scout_store_configured(home: &Path) -> bool {
    beagle_utils::paths::env_path("SCOUT_ROOT").is_some()
        || beagle_scout::config::path().is_some_and(|p| p.is_file())
        || home.join(".scout").join(".scout-store").is_file()
}

/// Parsed `beagle setup` arguments.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Run every step without asking.
    pub yes: bool,
    /// Step ids left out.
    pub skip: Vec<String>,
    /// When set, only these step ids run.
    pub only: Option<Vec<String>>,
    /// `--help` was asked for.
    pub help: bool,
}

impl Options {
    /// Parses the arguments after `setup`.
    ///
    /// # Errors
    /// A message for an unknown flag, a missing value or an unknown step id.
    pub fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut opts = Self::default();
        let mut args = args.iter().map(|a| a.to_string_lossy().into_owned());
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-y" | "--yes" => opts.yes = true,
                "-h" | "--help" | "help" => opts.help = true,
                "--skip" => {
                    let id = args.next().ok_or("--skip needs a step id")?;
                    opts.skip.push(known(&id)?);
                }
                "--only" => {
                    let ids = args.next().ok_or("--only needs step ids")?;
                    let ids: Result<Vec<String>, String> = ids.split(',').map(known).collect();
                    opts.only = Some(ids?);
                }
                other => return Err(format!("unknown argument `{other}`")),
            }
        }
        Ok(opts)
    }

    /// Whether `step` is picked by `--skip` / `--only`.
    #[must_use]
    pub fn wants(&self, step: &Step) -> bool {
        !self.skip.iter().any(|s| s == step.id)
            && self
                .only
                .as_ref()
                .is_none_or(|only| only.iter().any(|s| s == step.id))
    }
}

fn known(id: &str) -> Result<String, String> {
    let id = id.trim();
    if STEPS.iter().any(|s| s.id == id) {
        Ok(id.to_owned())
    } else {
        let ids: Vec<&str> = STEPS.iter().map(|s| s.id).collect();
        Err(format!("unknown step `{id}` (steps: {})", ids.join(", ")))
    }
}

/// What happened to each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Already in place; nothing ran.
    AlreadyDone,
    /// Left out by `--skip` / `--only`, or declined at the prompt.
    Skipped,
    /// Ran and succeeded.
    Ran,
    /// Ran and failed.
    Failed,
}

/// Runs the picked steps for `home`. `run` executes one tool command and
/// says whether it succeeded; `ask` asks a yes/no question (never called
/// with `--yes`).
pub fn run_steps(
    opts: &Options,
    home: &Path,
    mut run: impl FnMut(&str, Vec<String>) -> bool,
    mut ask: impl FnMut(&str) -> bool,
) -> Vec<(&'static str, Outcome)> {
    let mut outcomes = Vec::new();
    for step in STEPS {
        let outcome = if !opts.wants(step) {
            Outcome::Skipped
        } else if step.is_done(home) {
            println!("✓ {} — already in place", step.id);
            Outcome::AlreadyDone
        } else if !opts.yes
            && !ask(&format!(
                "{}: {}?\n  runs `{}`",
                step.id,
                step.what,
                step.command(home)
            ))
        {
            println!("· {} — skipped", step.id);
            Outcome::Skipped
        } else {
            println!("→ {}", step.command(home));
            if run(step.tool, (step.args)(home)) {
                Outcome::Ran
            } else {
                println!("✗ {} failed", step.id);
                Outcome::Failed
            }
        };
        outcomes.push((step.id, outcome));
    }
    outcomes
}

/// `beagle setup` (the arguments after `setup`).
pub fn run(args: &[OsString]) -> ExitCode {
    let opts = match Options::parse(args) {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("beagle setup: {message}\n\n{}", help());
            return ExitCode::from(2);
        }
    };
    if opts.help {
        println!("{}", help());
        return ExitCode::SUCCESS;
    }
    let Some(home) = beagle_utils::paths::home_dir() else {
        eprintln!("beagle setup: $HOME is not set");
        return ExitCode::FAILURE;
    };
    if !opts.yes && !std::io::stdin().is_terminal() {
        eprintln!("beagle setup: not a terminal to ask on — pass --yes to run every step");
        return ExitCode::from(2);
    }
    let outcomes = run_steps(&opts, &home, run_tool, ask_yes_no);
    if outcomes.iter().any(|(_, o)| *o == Outcome::Failed) {
        return ExitCode::FAILURE;
    }
    println!("\ndone — `beagle doctor` checks the rest (gh, agentd, tmux)");
    ExitCode::SUCCESS
}

fn help() -> String {
    let mut out = HELP.to_owned();
    for step in STEPS {
        let _ = write!(out, "\n  {:<14} {}", step.id, step.what);
    }
    out
}

fn run_tool(tool: &str, args: Vec<String>) -> bool {
    crate::tools::find(tool).is_some_and(|t| {
        (t.run)(args.into_iter().map(OsString::from).collect()) == ExitCode::SUCCESS
    })
}

/// Asks `question [Y/n]` on the terminal; Enter means yes.
pub fn ask_yes_no(question: &str) -> bool {
    print!("{question}\n  run it? [Y/n] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    )
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
