//! Wiring `beagle scout hook` into Claude Code: merge and remove scout's entries in
//! `~/.claude/settings.json` without disturbing anything else in it.
//!
//! Four entries, all running the same command (`<beagle binary> scout hook`):
//!
//! - `PreToolUse` (`Bash`, synchronous, milliseconds): records each command
//!   as it starts, for `beagle scout watch`.
//! - `PostToolUse` / `PostToolUseFailure` (every tool, `async`): record a
//!   command's output for `beagle scout watch`, and run the session pipeline for
//!   sessions attached to a task (so Edits and Writes show up too).
//! - `Stop` (`async`): the end of each Claude turn, so the final reply
//!   reaches the Session tab.
//!
//! `async` hooks run in the background and never slow Claude down. JSON key
//! order is preserved, and a backup is written before every change.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::error::{Error, Result};

/// The hook events scout listens to, in the order they fire.
pub const EVENTS: [&str; 4] = ["PreToolUse", "PostToolUse", "PostToolUseFailure", "Stop"];

/// How each event is registered: its matcher (`None` for events without
/// one) and whether it runs in the background.
fn spec(event: &str) -> (Option<&'static str>, bool) {
    match event {
        "PreToolUse" => (Some("Bash"), false),
        "Stop" => (None, true),
        _ => (Some("*"), true),
    }
}

/// The hook group scout wants for `event`.
fn desired_group(event: &str, command: &str) -> Value {
    let (matcher, background) = spec(event);
    let mut hook = json!({"type": "command", "command": command, "timeout": TIMEOUT_SECS});
    if background {
        hook["async"] = json!(true);
    }
    let mut group = Map::new();
    if let Some(matcher) = matcher {
        group.insert("matcher".to_owned(), json!(matcher));
    }
    group.insert("hooks".to_owned(), json!([hook]));
    Value::Object(group)
}

/// Seconds Claude Code waits for `beagle scout hook` (it finishes in milliseconds).
const TIMEOUT_SECS: u64 = 10;

/// `~/.claude/settings.json` under `home`.
#[must_use]
pub fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

/// The command the hooks run: `exe` (the `beagle` binary, quoted if needed)
/// followed by ` scout hook`.
#[must_use]
pub fn hook_command(exe: &Path) -> String {
    let exe = exe.to_string_lossy();
    if exe.contains(|c: char| c.is_whitespace() || "'\"$`\\".contains(c)) {
        format!("{} scout hook", beagle_utils::shell::quote(&exe))
    } else {
        format!("{exe} scout hook")
    }
}

/// Whether a hook command is scout's: `<…/beagle> scout hook`, or the
/// standalone `<…/scout> hook` an older install wrote (so installing
/// upgrades it in place and uninstalling removes it).
fn is_ours(command: &str) -> bool {
    let Some(rest) = command.trim_end().strip_suffix(" hook") else {
        return false;
    };
    match rest.strip_suffix(" scout") {
        Some(program) => is_program(program, "beagle"),
        None => is_program(rest, "scout"),
    }
}

/// Whether `program` (possibly single-quoted) is the binary `name`, bare or
/// at any path.
fn is_program(program: &str, name: &str) -> bool {
    let program = program.trim_matches('\'');
    program == name
        || program
            .strip_suffix(name)
            .is_some_and(|dir| dir.ends_with('/'))
}

/// Reads the settings file; a missing file is an empty object.
///
/// # Errors
/// [`Error::Io`] if it cannot be read, [`Error::Json`] if it is not JSON.
pub fn load(path: &Path) -> Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(json!({})),
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(Error::io(path)(e)),
    }
}

/// Writes the settings back (pretty-printed, atomically), first copying the
/// current file to `settings.json.scout-backup`. Returns the backup path
/// when there was a file to back up.
///
/// # Errors
/// [`Error::Io`] or [`Error::Json`].
pub fn save(path: &Path, settings: &Value) -> Result<Option<PathBuf>> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir).map_err(Error::io(dir))?;
    let backup = path.with_extension("json.scout-backup");
    let backed_up = if path.exists() {
        fs::copy(path, &backup).map_err(Error::io(&backup))?;
        Some(backup)
    } else {
        None
    };
    let mut text = serde_json::to_string_pretty(settings)?;
    text.push('\n');
    beagle_utils::fs::write_atomic(path, text.as_bytes()).map_err(Error::io(path))?;
    Ok(backed_up)
}

/// Adds scout's hook to each of [`EVENTS`], or updates its command if the
/// binary moved. Returns whether anything changed. Leaves non-object
/// settings alone (returns false).
pub fn install(settings: &mut Value, command: &str) -> bool {
    let Some(root) = settings.as_object_mut() else {
        return false;
    };
    let Some(hooks) = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
    else {
        return false;
    };
    let mut changed = false;
    for event in EVENTS {
        let Some(groups) = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
        else {
            continue;
        };
        let desired = desired_group(event, command);
        let ours = groups.iter().position(|g| {
            g["hooks"].as_array().is_some_and(|hs| {
                hs.iter()
                    .any(|h| h["command"].as_str().is_some_and(is_ours))
            })
        });
        match ours {
            // A group holding only scout's hook is scout's: make it exactly
            // what we want (command, matcher, async), upgrading old installs.
            Some(i)
                if groups[i]["hooks"]
                    .as_array()
                    .is_some_and(|hs| hs.len() == 1) =>
            {
                if groups[i] != desired {
                    groups[i] = desired;
                    changed = true;
                }
            }
            // Shared with someone else's hooks: only touch our own entry.
            Some(i) => {
                if let Some(hook) = groups[i]["hooks"].as_array_mut().and_then(|hs| {
                    hs.iter_mut()
                        .find(|h| h["command"].as_str().is_some_and(is_ours))
                }) {
                    if hook["command"] != command {
                        hook["command"] = json!(command);
                        changed = true;
                    }
                }
            }
            None => {
                groups.push(desired);
                changed = true;
            }
        }
    }
    changed
}

/// Removes scout's hooks, and any group, event, or `hooks` key left empty
/// by that removal. Returns whether anything changed.
pub fn uninstall(settings: &mut Value) -> bool {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    let mut changed = false;
    for event in EVENTS {
        let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
            continue;
        };
        groups.retain_mut(|group| {
            let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = list.len();
            list.retain(|h| {
                !h.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(is_ours)
            });
            if list.len() == before {
                return true;
            }
            changed = true;
            !list.is_empty()
        });
        if groups.is_empty() {
            hooks.remove(event);
        }
    }
    if hooks.is_empty() {
        if let Some(root) = settings.as_object_mut() {
            root.remove("hooks");
        }
    }
    changed
}

/// For each of [`EVENTS`], scout's installed hook command, if any.
#[must_use]
pub fn status(settings: &Value) -> Vec<(&'static str, Option<String>)> {
    EVENTS
        .iter()
        .map(|event| {
            let command = settings["hooks"][event]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|g| g["hooks"].as_array())
                .flatten()
                .filter_map(|h| h["command"].as_str())
                .find(|c| is_ours(c))
                .map(str::to_owned);
            (*event, command)
        })
        .collect()
}

#[cfg(test)]
mod tests;
