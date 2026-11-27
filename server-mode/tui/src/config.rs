//! `~/.config/beagle/server-mode.toml`: defaults for `on`, and the steps
//! run before and after a session. Every key is optional; flags win.
//!
//! ```toml
//! serve = true          # start beagle serve on `on`, stop it on `off`
//! new_pairing = false   # rotate the pairing token on every `on`
//! lock = true           # lock the screen once on
//! hours = 8             # default -t
//! start_up = "~/bin/run.sh"   # default -f (runs for the whole session)
//! on_stop = "~/bin/stop.sh"   # default --on-stop
//!
//! [[before]]            # run in order by `on`, before beagle serve starts
//! name = "network"
//! run = "my-vpn up"     # must exit 0 (sh -c, bounded by `timeout`)
//! wait = "my-vpn ip"    # optional: retried until it exits 0
//! timeout = 60          # seconds, for `run` and again for `wait`
//! optional = false      # true: a failure is a warning, not a stop
//!
//! [[after]]             # run by `off` / the timer, after beagle serve stops
//! name = "cleanup"
//! run = "…"
//! ```

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::{hours_to_secs, OnOpts};

/// Seconds a step's `run` (and, separately, its `wait`) may take.
pub(crate) const DEFAULT_STEP_TIMEOUT: u64 = 60;

/// The file's contents.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub serve: Option<bool>,
    pub new_pairing: Option<bool>,
    pub lock: Option<bool>,
    pub hours: Option<f64>,
    pub start_up: Option<PathBuf>,
    pub on_stop: Option<PathBuf>,
    #[serde(default)]
    pub before: Vec<Step>,
    #[serde(default)]
    pub after: Vec<Step>,
}

/// One command run by `on` (`[[before]]`) or `off` (`[[after]]`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Step {
    pub name: String,
    pub run: String,
    pub wait: Option<String>,
    pub timeout: Option<u64>,
    #[serde(default)]
    pub optional: bool,
}

impl Step {
    pub fn timeout_secs(&self) -> u64 {
        self.timeout.unwrap_or(DEFAULT_STEP_TIMEOUT).max(1)
    }

    /// The `--on-stop` file as an `[[after]]` step: executable files run
    /// as-is, anything else with bash (like the startup file).
    pub fn for_file(name: &str, file: &Path) -> Self {
        let q = shell_quote(&file.display().to_string());
        Self {
            name: name.to_owned(),
            run: format!("if [ -x {q} ]; then exec {q}; else exec bash {q}; fi"),
            wait: None,
            timeout: None,
            optional: true,
        }
    }
}

/// `$XDG_CONFIG_HOME/beagle/server-mode.toml` (default
/// `~/.config/beagle/server-mode.toml`).
pub(crate) fn default_path() -> Option<PathBuf> {
    beagle_utils::paths::xdg_config_home().map(|d| d.join("beagle").join("server-mode.toml"))
}

/// Loads `path`; a missing file is the empty config.
pub(crate) fn load(path: &Path) -> Result<Config, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parses the file's text (unknown keys fail, so typos are loud).
pub(crate) fn parse(text: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    for step in config.before.iter().chain(&config.after) {
        if step.name.trim().is_empty() || step.run.trim().is_empty() {
            return Err("every step needs a `name` and a `run`".into());
        }
    }
    Ok(config)
}

/// What `on` does: the flags over the file over the defaults.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plan {
    pub hours: Option<String>,
    pub secs: Option<u64>,
    pub file: Option<PathBuf>,
    pub on_stop: Option<PathBuf>,
    pub lock: bool,
    pub serve: bool,
    pub new_pairing: bool,
    pub before: Vec<Step>,
}

/// Merges `opts` over `config`. Paths from the file may start with `~/`.
pub(crate) fn plan(opts: &OnOpts, config: &Config) -> Result<Plan, String> {
    let (hours, secs) = match (&opts.hours, config.hours) {
        (Some(h), _) => (Some(h.clone()), opts.secs),
        (None, Some(h)) => {
            let text = h.to_string();
            let secs = hours_to_secs(&text).map_err(|e| format!("server-mode.toml: {e}"))?;
            (Some(text), Some(secs))
        }
        (None, None) => (None, None),
    };
    let tilde = |p: &PathBuf| beagle_utils::paths::expand_tilde(p);
    Ok(Plan {
        hours,
        secs,
        file: opts
            .file
            .clone()
            .or_else(|| config.start_up.as_ref().map(tilde)),
        on_stop: opts
            .on_stop
            .clone()
            .or_else(|| config.on_stop.as_ref().map(tilde)),
        lock: opts.lock.or(config.lock).unwrap_or(true),
        serve: opts.serve.or(config.serve).unwrap_or(true),
        new_pairing: opts.new_pairing.or(config.new_pairing).unwrap_or(false),
        before: config.before.clone(),
    })
}

/// `'…'` for `sh`, with embedded quotes escaped.
pub(crate) fn shell_quote(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', r"'\''"))
}
