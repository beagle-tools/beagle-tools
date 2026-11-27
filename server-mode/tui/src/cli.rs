//! Arguments → [`Cmd`]. Pure: nothing here touches the system.

use std::path::PathBuf;

/// What to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    /// Stay awake (the default, and what bare flags mean).
    On(OnOpts),
    /// Let the Mac sleep again and stop the startup file.
    Off,
    /// On/off, time left, startup file.
    Status,
    /// The startup file's last 50 log lines.
    Log,
    /// Lock the screen now.
    Lock,
    /// Print the help.
    Help,
}

/// The flags of `on`. Each is `None` when not given, so
/// `server-mode.toml` (then the built-in default) decides.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct OnOpts {
    /// `-t` as typed, for the message (`awake for 8h`).
    pub hours: Option<String>,
    /// `-t` in whole seconds (at least 1).
    pub secs: Option<u64>,
    /// `-f`: the startup file, as given.
    pub file: Option<PathBuf>,
    /// `--on-stop`: a file run when server mode ends.
    pub on_stop: Option<PathBuf>,
    /// `--no-lock` → `Some(false)`.
    pub lock: Option<bool>,
    /// `--serve` / `--no-serve`: start and stop `beagle serve`.
    pub serve: Option<bool>,
    /// `--new-pairing`: rotate the pairing token first.
    pub new_pairing: Option<bool>,
}

const SEE_HELP: &str = "(see beagle server-mode --help)";

/// Parses the arguments after `server-mode`. The error is a usage error.
pub(crate) fn parse(args: &[String]) -> Result<Cmd, String> {
    let Some(first) = args.first() else {
        return Ok(Cmd::On(OnOpts::default()));
    };
    let simple = match first.as_str() {
        "on" => return parse_on(&args[1..]),
        flag if flag.starts_with('-') => return parse_on(args),
        "off" => Cmd::Off,
        "status" => Cmd::Status,
        "log" => Cmd::Log,
        "lock" => Cmd::Lock,
        "help" => Cmd::Help,
        other => return Err(format!("unknown command: {other} {SEE_HELP}")),
    };
    match args.get(1) {
        Some(extra) => Err(format!("unexpected argument: {extra} {SEE_HELP}")),
        None => Ok(simple),
    }
}

fn parse_on(args: &[String]) -> Result<Cmd, String> {
    let mut opts = OnOpts::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
            _ => (arg.as_str(), None),
        };
        match flag {
            "-t" | "--hours" => {
                let value = value(inline, &mut it, flag, "a number of hours")?;
                opts.secs = Some(hours_to_secs(&value)?);
                opts.hours = Some(value);
            }
            "-f" | "--file" => {
                opts.file = Some(PathBuf::from(value(inline, &mut it, flag, "a file")?));
            }
            // Reads as a label for -f.
            "--start-up" | "--startup" if inline.is_none() => {}
            "--on-stop" => {
                opts.on_stop = Some(PathBuf::from(value(inline, &mut it, flag, "a file")?));
            }
            "--no-lock" if inline.is_none() => opts.lock = Some(false),
            "--serve" if inline.is_none() => opts.serve = Some(true),
            "--no-serve" if inline.is_none() => opts.serve = Some(false),
            "--new-pairing" if inline.is_none() => opts.new_pairing = Some(true),
            "-h" | "--help" if inline.is_none() => return Ok(Cmd::Help),
            _ => return Err(format!("unknown option: {arg} {SEE_HELP}")),
        }
    }
    Ok(Cmd::On(opts))
}

/// The value of `flag`: after `=`, else the next argument.
fn value<'a>(
    inline: Option<String>,
    rest: &mut impl Iterator<Item = &'a String>,
    flag: &str,
    what: &str,
) -> Result<String, String> {
    inline
        .or_else(|| rest.next().cloned())
        .ok_or_else(|| format!("{flag} needs {what}"))
}

/// `-t` hours (decimals ok) in whole seconds, never less than one.
pub(crate) fn hours_to_secs(hours: &str) -> Result<u64, String> {
    let bad = || format!("--hours must be a positive number, got '{hours}'");
    let h: f64 = hours.trim().parse().map_err(|_| bad())?;
    if !h.is_finite() || h <= 0.0 || h > 1.0e6 {
        return Err(bad());
    }
    // Bounded to (0, 3.6e9] above, so the truncating cast is exact enough
    // (the script's `printf %d` truncates too).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let secs = (h * 3600.0) as u64;
    Ok(secs.max(1))
}
