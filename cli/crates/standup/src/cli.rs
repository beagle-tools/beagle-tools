//! Argument parsing — pure: no filesystem, no environment.

/// Usage text for `beagle standup --help` and parse errors.
pub const USAGE: &str = "\
beagle standup — standup reports: one overview, then a short write-up per ticket

USAGE
  beagle standup [--date YYYY-MM-DD]   open the TUI (newest report by default)
  beagle standup new [YYYY-MM-DD] [--since YYYY-MM-DD] [--title TEXT]
                                       scaffold a report (default: today; --since
                                       defaults to the previous report's date)
  beagle standup ticket <ID> --title TEXT [--date D] [--url URL] [--state STATE]
                 [--system NAME]... [--pr URL]... [--update]
                                       scaffold a ticket in a report (default:
                                       newest); --update rewrites ticket.toml only
  beagle standup list [--json]         list reports, newest first
  beagle standup show [YYYY-MM-DD]     print a whole report as markdown (agents)
  beagle standup last                  print the newest report's date
  beagle standup path [YYYY-MM-DD]     print a report's directory (or standups/)
  beagle standup guide                 print the agent guide (how everything works)
  beagle standup version

ID is a tracker ticket id like ABC-123; --url is its page in your tracker.

GLOBAL
  --root DIR    store root, the dir holding standups/ (else $STANDUP_ROOT,
                config `root`, the nearest parent dir with standups/, or .)
";

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// `--root`, if given.
    pub root: Option<String>,
    /// What to do.
    pub command: Command,
}

/// The subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Open the TUI.
    Tui {
        /// Report to open.
        date: Option<String>,
    },
    /// Scaffold a report.
    New {
        /// Report date (default today).
        date: Option<String>,
        /// Previous standup date.
        since: Option<String>,
        /// Headline.
        title: Option<String>,
    },
    /// Scaffold or update a ticket.
    Ticket {
        /// Ticket id.
        id: String,
        /// Ticket title.
        title: Option<String>,
        /// Report date (default newest).
        date: Option<String>,
        /// The ticket's URL in the tracker.
        url: Option<String>,
        /// Workflow state.
        state: Option<String>,
        /// Systems touched.
        systems: Vec<String>,
        /// PR URLs.
        prs: Vec<String>,
        /// Rewrite an existing ticket's manifest.
        update: bool,
    },
    /// List reports.
    List {
        /// Emit JSON.
        json: bool,
    },
    /// Print a report as markdown.
    Show {
        /// Report date (default newest).
        date: Option<String>,
    },
    /// Print the newest report date.
    Last,
    /// Print a report directory.
    Path {
        /// Report date (default: the root).
        date: Option<String>,
    },
    /// Print the agent guide.
    Guide,
    /// Print help.
    Help,
    /// Print the version.
    Version,
}

/// Parses OS arguments (without the program or tool name).
///
/// # Errors
/// A message for a non-UTF-8 argument, else as [`parse`].
pub fn parse_os(args: Vec<std::ffi::OsString>) -> Result<Args, String> {
    let args = args
        .into_iter()
        .map(|a| {
            a.into_string()
                .map_err(|a| format!("argument `{}` is not valid UTF-8", a.to_string_lossy()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    parse(args)
}

/// Parses `args` (without the program name).
///
/// # Errors
/// A human-readable message for unknown flags, missing values, or
/// unexpected positionals.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut root = None;
    let mut flags: Vec<(String, Option<String>)> = Vec::new();
    let mut positional: Vec<String> = Vec::new();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => positional.insert(0, "help".to_owned()),
            "-V" | "--version" => positional.insert(0, "version".to_owned()),
            "--json" | "--update" => flags.push((arg, None)),
            "--root" => root = Some(it.next().ok_or("--root needs a value")?),
            flag if flag.starts_with("--") => {
                let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
                flags.push((arg, Some(value)));
            }
            _ => positional.push(arg),
        }
    }

    let mut pos = positional.into_iter();
    let sub = pos.next();
    let mut take = |name: &str| -> Option<String> {
        let i = flags.iter().position(|(f, _)| f == name)?;
        flags.remove(i).1
    };
    let command = match sub.as_deref() {
        None => Command::Tui {
            date: take("--date"),
        },
        Some("help") => Command::Help,
        Some("guide") => Command::Guide,
        Some("version") => Command::Version,
        Some("new") => Command::New {
            date: pos.next(),
            since: take("--since"),
            title: take("--title"),
        },
        Some("ticket") => {
            let id = pos
                .next()
                .ok_or("ticket needs an id, e.g. `beagle standup ticket ABC-123 --title ...`")?;
            let (title, date, url, state) = (
                take("--title"),
                take("--date"),
                take("--url"),
                take("--state"),
            );
            let mut systems = Vec::new();
            while let Some(s) = take("--system") {
                systems.push(s);
            }
            let mut prs = Vec::new();
            while let Some(p) = take("--pr") {
                prs.push(p);
            }
            let update = remove_switch(&mut flags, "--update");
            Command::Ticket {
                id,
                title,
                date,
                url,
                state,
                systems,
                prs,
                update,
            }
        }
        Some("list") => Command::List {
            json: remove_switch(&mut flags, "--json"),
        },
        Some("show") => Command::Show { date: pos.next() },
        Some("last") => Command::Last,
        Some("path") => Command::Path { date: pos.next() },
        Some(other) => return Err(format!("unknown command `{other}`")),
    };
    if let Some(extra) = pos.next() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    if let Some((flag, _)) = flags.first() {
        return Err(format!("unknown or misplaced flag `{flag}`"));
    }
    Ok(Args { root, command })
}

fn remove_switch(flags: &mut Vec<(String, Option<String>)>, name: &str) -> bool {
    let before = flags.len();
    flags.retain(|(f, v)| !(f == name && v.is_none()));
    flags.len() != before
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
