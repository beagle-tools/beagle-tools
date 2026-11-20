//! The words every tool answers the same way, whatever its own parser
//! does: `beagle <tool> help|-h|--help` prints the tool's help, and
//! `beagle <tool> version|-V|--version` prints `beagle <tool> <version>`,
//! where the version is this binary's (every tool ships inside it).

use std::ffi::OsString;

/// What `beagle <tool> <args>` means before the tool sees it.
#[derive(Debug, PartialEq, Eq)]
pub enum Shared {
    /// Run the tool with `--help`.
    Help,
    /// Print `beagle <tool> <version>`.
    Version,
}

/// The shared meaning of a tool's arguments, when they are exactly one
/// help or version word; `None` hands them to the tool unchanged.
#[must_use]
pub fn shared(args: &[OsString]) -> Option<Shared> {
    let [only] = args else {
        return None;
    };
    match only.to_str()? {
        "help" | "-h" | "--help" => Some(Shared::Help),
        "version" | "-V" | "--version" => Some(Shared::Version),
        _ => None,
    }
}

/// The line `beagle <tool> version` prints.
#[must_use]
pub fn version_line(tool: &str) -> String {
    format!("beagle {tool} {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    use super::*;
    use crate::tools::TOOLS;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn help_and_version_words_are_shared_only_when_alone() {
        for word in ["help", "-h", "--help"] {
            assert_eq!(shared(&args(&[word])), Some(Shared::Help), "{word}");
        }
        for word in ["version", "-V", "--version"] {
            assert_eq!(shared(&args(&[word])), Some(Shared::Version), "{word}");
        }
        // Anything else, or more words, is the tool's own business.
        assert_eq!(shared(&args(&[])), None);
        assert_eq!(shared(&args(&["list"])), None);
        assert_eq!(shared(&args(&["version", "list"])), None);
        assert_eq!(shared(&args(&["help", "new"])), None);
    }

    #[test]
    fn every_tool_prints_beagle_tool_version() {
        for tool in TOOLS {
            assert_eq!(
                version_line(tool.name),
                format!("beagle {} {}", tool.name, env!("CARGO_PKG_VERSION"))
            );
        }
    }

    #[test]
    fn every_tool_answers_help() {
        for tool in TOOLS {
            assert_eq!(
                (tool.run)(args(&["--help"])),
                ExitCode::SUCCESS,
                "beagle {} --help",
                tool.name
            );
        }
    }
}
