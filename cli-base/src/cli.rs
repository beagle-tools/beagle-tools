//! Argument parsing for the base commands, plus the `--help` text.
//!
//! Pure: no filesystem, no network. Arguments for an extension are never
//! parsed — they are carried as raw `OsString`s and forwarded untouched.

use std::ffi::OsString;

use crate::registry::{self, ExtensionSpec};

/// The base commands' help text; installed extensions are appended at run
/// time.
pub const USAGE: &str = "\
beagle — one command for every Beagle tool

USAGE:
    beagle <extension> [args...]          run an installed extension
    beagle <extension> update [--version <v>]
                                          update (or pin) one extension
    beagle extensions list                installed extensions and versions
    beagle extensions available [--pre]   every extension and its latest release
    beagle extensions install <ext> [--version <v>] [--yes] [--pre]
    beagle extensions update [<ext>... | --all] [--version <v>] [--yes]
    beagle extensions use <ext> <version> switch to an installed version (offline)
    beagle extensions uninstall <ext> [--all-versions]
    beagle extensions prune [<ext>] [--keep <n>]
                                          delete old versions (keeps 1 previous)
    beagle extensions path <ext>          print the active binary's path
    beagle update [--version <v>] [--all] update beagle itself (--all: and every
                                            extension)
    beagle doctor                         check the install for problems
    beagle version | --version            versions of beagle and its extensions
    beagle help [<extension>] | --help

`beagle ext` is short for `beagle extensions`. Without --version, install and
update show a version picker on a terminal and take the latest otherwise.
<v> is 0.29.0, v0.29.0 or a full tag such as beagle-rca-v0.29.0.";

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print help: the base's, or (with a name) the extension's own.
    Help {
        /// `beagle help <ext>`.
        ext: Option<String>,
    },
    /// Print base and extension versions.
    Version,
    /// Check the install.
    Doctor,
    /// `beagle update`: update the base itself.
    SelfUpdate {
        /// Pin to this version.
        version: Option<String>,
        /// Also update every installed extension.
        all: bool,
    },
    /// `beagle extensions …`.
    Ext(ExtCommand),
    /// `beagle <name> args…`: dispatch to an extension.
    Run {
        /// The extension (or `PATH` binary) name.
        name: String,
        /// Its arguments, verbatim.
        args: Vec<OsString>,
    },
}

/// `beagle extensions <sub>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtCommand {
    /// Installed extensions.
    List,
    /// Every registry entry and its latest release.
    Available {
        /// Include pre-releases.
        pre: bool,
    },
    /// Install (or switch to) a version.
    Install {
        /// The extension.
        name: String,
        /// Pin to this version.
        version: Option<String>,
        /// Never prompt.
        yes: bool,
        /// Include pre-releases.
        pre: bool,
    },
    /// Update extensions (one, several, or `--all` installed ones).
    Update {
        /// The extensions; empty with `all`.
        names: Vec<String>,
        /// Every installed extension.
        all: bool,
        /// Pin to this version (one extension only).
        version: Option<String>,
        /// Never prompt.
        yes: bool,
    },
    /// Switch `current` to an installed version.
    Use {
        /// The extension.
        name: String,
        /// The version.
        version: String,
    },
    /// Remove the active version (or every version).
    Uninstall {
        /// The extension.
        name: String,
        /// Remove every version.
        all_versions: bool,
    },
    /// Delete old versions.
    Prune {
        /// One extension, or every installed one.
        name: Option<String>,
        /// Non-current versions to keep.
        keep: usize,
    },
    /// Print the active binary path.
    Path {
        /// The extension.
        name: String,
    },
}

/// Parses `args` (without the program name). `registry` decides which
/// names get `<ext> update` intercepted.
pub fn parse(args: Vec<OsString>, registry: &[ExtensionSpec]) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Help { ext: None });
    };
    let first = first
        .into_string()
        .map_err(|raw| format!("command `{}` is not valid UTF-8", raw.display()))?;
    match first.as_str() {
        "-h" | "--help" | "help" => {
            let rest = strings(args)?;
            match rest.as_slice() {
                [] => Ok(Command::Help { ext: None }),
                [ext] => Ok(Command::Help {
                    ext: Some(ext.clone()),
                }),
                _ => Err("usage: beagle help [<extension>]".to_owned()),
            }
        }
        "-V" | "--version" | "version" => no_args(args, Command::Version),
        "doctor" => no_args(args, Command::Doctor),
        "update" => {
            let mut flags = Flags::parse(strings(args)?, &["--version"])?;
            if flags.help {
                return Ok(Command::Help { ext: None });
            }
            flags.no_positionals("update")?;
            let command = Command::SelfUpdate {
                version: flags.value("--version"),
                all: flags.has("--all"),
            };
            flags.finish()?;
            Ok(command)
        }
        "extensions" | "ext" => parse_ext(&strings(args)?),
        "completions" | "self" => Err(format!(
            "`beagle {first}` is reserved but not available yet"
        )),
        flag if flag.starts_with('-') => Err(format!("unknown flag `{flag}`")),
        _ => {
            let rest: Vec<OsString> = args.collect();
            let intercepted = registry::find(registry, &first).is_some()
                && rest.first().is_some_and(|a| a == "update");
            if !intercepted {
                return Ok(Command::Run {
                    name: first,
                    args: rest,
                });
            }
            let mut flags = Flags::parse(strings(rest.into_iter().skip(1))?, &["--version"])?;
            flags.no_positionals(&format!("{first} update"))?;
            let command = ExtCommand::Update {
                names: vec![first],
                all: false,
                version: flags.value("--version"),
                yes: flags.has("--yes"),
            };
            flags.finish()?;
            Ok(Command::Ext(command))
        }
    }
}

fn parse_ext(args: &[String]) -> Result<Command, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err(
            "usage: beagle extensions <list|available|install|update|use|uninstall|prune|path>"
                .to_owned(),
        );
    };
    let mut flags = Flags::parse(rest.to_vec(), &["--version", "--keep"])?;
    if flags.help {
        return Ok(Command::Help { ext: None });
    }
    let one = |flags: &mut Flags, usage: &str| -> Result<String, String> {
        match flags.positionals.as_slice() {
            [name] => Ok(name.clone()),
            _ => Err(format!("usage: beagle extensions {usage}")),
        }
    };
    let command = match sub.as_str() {
        "list" => {
            flags.no_positionals("extensions list")?;
            ExtCommand::List
        }
        "available" => {
            flags.no_positionals("extensions available")?;
            ExtCommand::Available {
                pre: flags.has("--pre"),
            }
        }
        "install" => ExtCommand::Install {
            name: one(&mut flags, "install <ext> [--version <v>] [--yes]")?,
            version: flags.value("--version"),
            yes: flags.has("--yes"),
            pre: flags.has("--pre"),
        },
        "update" => {
            let all = flags.has("--all");
            let names = std::mem::take(&mut flags.positionals);
            if all != names.is_empty() {
                return Err("usage: beagle extensions update (<ext>... | --all)".to_owned());
            }
            let version = flags.value("--version");
            if version.is_some() && names.len() != 1 {
                return Err("--version needs exactly one extension".to_owned());
            }
            ExtCommand::Update {
                names,
                all,
                version,
                yes: flags.has("--yes"),
            }
        }
        "use" => match flags.positionals.as_slice() {
            [name, version] => ExtCommand::Use {
                name: name.clone(),
                version: version.clone(),
            },
            _ => return Err("usage: beagle extensions use <ext> <version>".to_owned()),
        },
        "uninstall" => ExtCommand::Uninstall {
            name: one(&mut flags, "uninstall <ext> [--all-versions]")?,
            all_versions: flags.has("--all-versions"),
        },
        "prune" => {
            let keep = match flags.value("--keep") {
                Some(n) => n
                    .parse()
                    .map_err(|_| format!("--keep wants a number, got `{n}`"))?,
                None => 1,
            };
            let name = match flags.positionals.as_slice() {
                [] => None,
                [name] => Some(name.clone()),
                _ => return Err("usage: beagle extensions prune [<ext>] [--keep <n>]".to_owned()),
            };
            ExtCommand::Prune { name, keep }
        }
        "path" => ExtCommand::Path {
            name: one(&mut flags, "path <ext>")?,
        },
        other => return Err(format!("unknown extensions command `{other}`")),
    };
    flags.finish()?;
    Ok(Command::Ext(command))
}

/// Base-command arguments must be UTF-8 (unlike forwarded ones).
fn strings(args: impl Iterator<Item = OsString>) -> Result<Vec<String>, String> {
    args.map(|a| {
        a.into_string()
            .map_err(|raw| format!("argument `{}` is not valid UTF-8", raw.display()))
    })
    .collect()
}

fn no_args(mut args: impl Iterator<Item = OsString>, command: Command) -> Result<Command, String> {
    match args.next() {
        None => Ok(command),
        Some(a) if a == "--help" || a == "-h" => Ok(Command::Help { ext: None }),
        Some(a) => Err(format!("unexpected argument `{}`", a.display())),
    }
}

/// Flags and positionals of one base command.
#[derive(Debug, Default)]
struct Flags {
    switches: Vec<String>,
    values: Vec<(String, String)>,
    positionals: Vec<String>,
    help: bool,
}

/// Switches any base command accepts; each command reads the ones it uses
/// and [`Flags::finish`] rejects the rest.
const SWITCHES: &[&str] = &["--all", "--yes", "--pre", "--all-versions"];

impl Flags {
    fn parse(args: Vec<String>, valued: &[&str]) -> Result<Self, String> {
        let mut flags = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                flags.help = true;
            } else if arg == "-y" {
                flags.switches.push("--yes".to_owned());
            } else if let Some((key, value)) =
                arg.split_once('=').filter(|(k, _)| valued.contains(k))
            {
                flags.values.push((key.to_owned(), value.to_owned()));
            } else if valued.contains(&arg.as_str()) {
                let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
                flags.values.push((arg, value));
            } else if SWITCHES.contains(&arg.as_str()) {
                flags.switches.push(arg);
            } else if arg.starts_with('-') && arg != "-" {
                return Err(format!("unknown flag `{arg}`"));
            } else {
                flags.positionals.push(arg);
            }
        }
        Ok(flags)
    }

    fn has(&mut self, switch: &str) -> bool {
        let before = self.switches.len();
        self.switches.retain(|s| s != switch);
        self.switches.len() != before
    }

    fn value(&mut self, key: &str) -> Option<String> {
        let at = self.values.iter().rposition(|(k, _)| k == key)?;
        let (_, value) = self.values.remove(at);
        self.values.retain(|(k, _)| k != key);
        Some(value)
    }

    fn no_positionals(&mut self, command: &str) -> Result<(), String> {
        match self.positionals.first() {
            None => Ok(()),
            Some(extra) => Err(format!("`beagle {command}` takes no argument `{extra}`")),
        }
    }

    /// Fails on any flag the command did not read.
    fn finish(self) -> Result<(), String> {
        if let Some(flag) = self
            .switches
            .first()
            .or(self.values.first().map(|(k, _)| k))
        {
            return Err(format!("flag `{flag}` does not apply here"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
