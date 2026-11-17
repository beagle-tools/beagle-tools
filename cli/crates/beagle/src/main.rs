//! `beagle`: one command for every beagle-tools tool.
//!
//! This binary only dispatches. `beagle <tool> …` hands everything after
//! the tool's name to that tool's own `run`, so each tool keeps its own
//! commands, flags and TUI exactly as it would standalone.

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
        Some("version" | "-V" | "--version") => {
            println!("beagle {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(name) => match tools::find(name) {
            Some(tool) => (tool.run)(rest),
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
    if tools::TOOLS.is_empty() {
        println!("  (none yet)");
    }
    println!("\n  help [tool]  this list, or a tool's own help");
    println!("  version      print the version");
}
