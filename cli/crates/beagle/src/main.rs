//! `beagle`: one command for every beagle-tools tool.
//!
//! This binary dispatches. `beagle <tool> …` hands everything after the
//! tool's name to that tool's own `run`, so each tool keeps its own
//! commands, flags and TUI exactly as it would standalone. The few
//! commands about the binary itself — `update`, `version`, `doctor`,
//! `help` — are answered here, before any tool.

mod doctor;
mod health;
mod selfupdate;
mod setup;
mod shared;
mod tools;

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        print_help();
        return ExitCode::SUCCESS;
    };
    let rest: Vec<OsString> = args.collect();
    match first.to_str() {
        Some("help" | "-h" | "--help") => {
            if let Some(name) = rest.first().and_then(|a| a.to_str()) {
                return match tools::find(name) {
                    Some(tool) => (tool.run)(vec![OsString::from("--help")]),
                    None => unknown(name),
                };
            }
            print_help();
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            selfupdate::print_version();
            ExitCode::SUCCESS
        }
        Some("version") => selfupdate::run_version(&rest),
        Some("update") => selfupdate::run_update(&rest),
        Some("doctor") if rest.is_empty() => doctor::run(),
        Some("doctor") => {
            eprintln!("beagle: `doctor` takes no arguments");
            ExitCode::from(2)
        }
        Some("setup") => setup::run(&rest),
        Some(name) => match tools::find(name) {
            // `help` and `version` mean the same in every tool.
            Some(tool) => match shared::shared(&rest) {
                Some(shared::Shared::Help) => (tool.run)(vec![OsString::from("--help")]),
                Some(shared::Shared::Version) => {
                    println!("{}", shared::version_line(tool.name));
                    ExitCode::SUCCESS
                }
                None => (tool.run)(rest),
            },
            None => unknown(name),
        },
        None => unknown(&first.to_string_lossy()),
    }
}

fn unknown(name: &str) -> ExitCode {
    eprintln!("beagle: unknown command `{name}` — run `beagle help` for the list");
    ExitCode::from(2)
}

fn print_help() {
    println!(
        "beagle {} — developer tools for the terminal\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: beagle <tool> [args…]\n");
    println!("Tools:");
    let width = tools::TOOLS.iter().map(|t| t.name.len()).max().unwrap_or(0);
    for t in tools::TOOLS {
        println!("  {:width$}  {}", t.name, t.about);
    }
    println!("\nThis binary:");
    println!("  help [tool]          this list, or a tool's own help");
    println!("  version [list]       print the version, or browse releases to install one");
    println!("  update [--check]     update to the latest release (--version X.Y.Z for any)");
    println!(
        "  setup [--yes]        run every tool's one-time install steps (skills, hooks, store)"
    );
    println!("  doctor               check the setup, and find old binaries this one replaces");
}
