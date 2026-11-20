//! Argument parsing — pure: no filesystem, no environment, no network.

/// Usage text for `beagle recap --help` and parse errors.
pub const USAGE: &str = "\
beagle recap — PR recaps: the diff plus a plain-language write-up of what it fixes and why

USAGE
  beagle recap [ID|PR]                     open the TUI (newest PR by default)
  beagle recap new <PR> [--ticket ID] [--ticket-url URL]
                                           pull the PR's metadata + diff from GitHub (gh)
                                           and scaffold its write-up
  beagle recap refresh <ID|PR> [--ticket-url URL]
                                           re-pull metadata + diff; keeps the write-up
  beagle recap list [--json] [--archived]  list recaps: drafts, open, merged, closed
                                           (--archived: the archive instead)
  beagle recap sync                        ask GitHub for every PR's state now and record
                                           changes (the TUI also polls every 3 minutes)
  beagle recap show <ID|PR> [--diff]       print a recap as markdown (agents)
  beagle recap comments <ID|PR>            print the PR's reviews, inline review comments
                                           and conversation (read-only)
  beagle recap comments sync               fetch every open/draft PR's comment threads (local)
  beagle recap comments pending [--json]   threads waiting on an agent review
  beagle recap comments review <ID> <THREAD> <FILE>
                                           install an agent's local review of a thread
  beagle recap path [ID|PR]                print a recap's directory (or recaps/)
  beagle recap archive <ID|PR>             move a recap to the archive (local only)
  beagle recap unarchive <ID|PR>           bring a recap back from the archive
  beagle recap merge-check <ID|PR>         can this PR merge right now, and how?
                                           (read-only: never merges)
  beagle recap guide                       print the agent guide (how everything works)
  beagle recap version

PR is a URL (https://github.com/<owner>/<repo>/pull/<n>) or <owner>/<repo>#<n>.
ID is a recap id like widgets-42 (see `beagle recap list`).

GLOBAL
  --root DIR    store root, the dir holding recaps/ (else $RECAP_ROOT,
                config `root`, the nearest parent dir with recaps/, or .)
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
        /// Recap to open (id or PR).
        target: Option<String>,
    },
    /// Pull a PR and scaffold its recap.
    New {
        /// The PR.
        pr: String,
        /// Ticket id override.
        ticket: Option<String>,
        /// Ticket URL.
        ticket_url: Option<String>,
    },
    /// Re-pull a PR's metadata and diff.
    Refresh {
        /// Recap id or PR.
        target: String,
        /// Ticket URL.
        ticket_url: Option<String>,
    },
    /// Poll every PR's state once and record changes.
    Sync,
    /// List recaps.
    List {
        /// Emit JSON.
        json: bool,
        /// List the archive instead.
        archived: bool,
    },
    /// Move a recap to the archive.
    Archive {
        /// Recap id or PR.
        target: String,
    },
    /// Bring a recap back from the archive.
    Unarchive {
        /// Recap id or PR.
        target: String,
    },
    /// Read-only: would a merge go through, and how.
    MergeCheck {
        /// Recap id or PR.
        target: String,
    },
    /// Print a recap as markdown.
    Show {
        /// Recap id or PR.
        target: String,
        /// Include the full diff.
        diff: bool,
    },
    /// Print a PR's reviews and comments.
    Comments {
        /// Recap id or PR.
        target: String,
    },
    /// Fetch every open/draft recap's comment threads into its local
    /// `comments/threads.json`.
    CommentsSync,
    /// Threads waiting on an agent review.
    CommentsPending {
        /// Print JSON.
        json: bool,
    },
    /// Install an agent's local review of one thread.
    CommentsReview {
        /// Recap id or PR.
        target: String,
        /// Thread id (`t-…`, `c-…`, `r-…`).
        thread: String,
        /// Markdown file holding the review.
        file: String,
    },
    /// Print a recap's directory.
    Path {
        /// Recap id or PR (default: the recaps dir).
        target: Option<String>,
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
            "--json" | "--diff" | "--archived" => flags.push((arg, None)),
            "--root" => root = Some(it.next().ok_or("--root needs a value")?),
            flag if flag.starts_with("--root=") => {
                root = Some(flag["--root=".len()..].to_owned());
            }
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
        let i = flags.iter().position(|(f, v)| f == name && v.is_some())?;
        flags.remove(i).1
    };
    let command = match sub.as_deref() {
        Some("help") => Command::Help,
        Some("version") => Command::Version,
        Some("guide") => Command::Guide,
        Some("new") => Command::New {
            pr: pos
                .next()
                .ok_or("new needs a PR, e.g. `beagle recap new https://github.com/o/r/pull/1`")?,
            ticket: take("--ticket"),
            ticket_url: take("--ticket-url"),
        },
        Some("refresh") => Command::Refresh {
            target: pos.next().ok_or("refresh needs a recap id or PR")?,
            ticket_url: take("--ticket-url"),
        },
        Some("list") => Command::List {
            json: remove_switch(&mut flags, "--json"),
            archived: remove_switch(&mut flags, "--archived"),
        },
        Some("archive") => Command::Archive {
            target: pos.next().ok_or("archive needs a recap id or PR")?,
        },
        Some("unarchive") => Command::Unarchive {
            target: pos.next().ok_or("unarchive needs a recap id or PR")?,
        },
        Some("merge-check") => Command::MergeCheck {
            target: pos.next().ok_or("merge-check needs a recap id or PR")?,
        },
        Some("show") => Command::Show {
            target: pos.next().ok_or("show needs a recap id or PR")?,
            diff: remove_switch(&mut flags, "--diff"),
        },
        Some("path") => Command::Path { target: pos.next() },
        Some("sync") => Command::Sync,
        Some("comments") => match pos.next().as_deref() {
            Some("sync") => Command::CommentsSync,
            Some("pending") => Command::CommentsPending {
                json: remove_switch(&mut flags, "--json"),
            },
            Some("review") => Command::CommentsReview {
                target: pos.next().ok_or("comments review needs a recap id or PR")?,
                thread: pos.next().ok_or("comments review needs a thread id")?,
                file: pos.next().ok_or("comments review needs a markdown file")?,
            },
            Some(target) => Command::Comments {
                target: target.to_owned(),
            },
            None => {
                return Err(
                    "comments needs a recap id or PR, or sync / pending / review".to_owned(),
                )
            }
        },
        None => Command::Tui { target: None },
        // A PR (URL or owner/repo#n) or a recap id (`<repo>-<number>`)
        // opens the TUI there; any other word is a typo, not a target.
        Some(other) if other.contains('#') || other.contains('/') || looks_like_id(other) => {
            Command::Tui {
                target: Some(other.to_owned()),
            }
        }
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

/// Whether `word` has a recap id's shape: `<repo>-<number>`.
fn looks_like_id(word: &str) -> bool {
    word.rsplit_once('-').is_some_and(|(repo, n)| {
        !repo.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())
    })
}

fn remove_switch(flags: &mut Vec<(String, Option<String>)>, name: &str) -> bool {
    let before = flags.len();
    flags.retain(|(f, v)| !(f == name && v.is_none()));
    flags.len() != before
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
