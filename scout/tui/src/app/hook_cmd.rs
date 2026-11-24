//! `beagle scout hook` (called by Claude Code) and `hook install |
//! uninstall | status`.

use std::env;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::session::{self, Location};
use crate::store::Store;
use crate::{hooks, transcript, Error, Result};

use super::{home, resolve_root};

/// `beagle scout hook`, called by Claude Code's hooks. It must never disturb
/// Claude: every problem is reported on stderr and the exit code is always 0.
pub(super) fn record(root: Option<PathBuf>) {
    if let Err(e) = try_record(root) {
        eprintln!("beagle scout hook: {e}");
    }
}

fn try_record(root: Option<PathBuf>) -> Result<()> {
    const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
    let mut input = String::new();
    std::io::stdin()
        .take(MAX_INPUT_BYTES)
        .read_to_string(&mut input)
        .map_err(Error::Terminal)?;
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&input) else {
        return Ok(()); // not a hook payload; nothing to record
    };
    let root = resolve_root(root)?;
    let now = time::OffsetDateTime::now_utc();
    if let Some((id, event)) = session::event_from_hook(&value, &Location::from_env(), now) {
        session::append(&root, &id, &event)?;
    }
    // The session pipeline: after any tool call or turn of a session that
    // is attached to a task, bring its rendered view up to date.
    let pipeline_event = matches!(
        value
            .get("hook_event_name")
            .and_then(serde_json::Value::as_str),
        Some("PostToolUse" | "PostToolUseFailure" | "Stop")
    );
    if pipeline_event {
        if let Some(session_id) = value.get("session_id").and_then(serde_json::Value::as_str) {
            run_session_pipeline(&root, session_id, &value);
        }
    }
    Ok(())
}

/// Runs the session pipeline for every project `session` is attached to.
fn run_session_pipeline(root: &Path, session: &str, hook: &serde_json::Value) {
    let store = Store::new(root);
    let projects = store.attached_projects(session);
    if projects.is_empty() {
        return;
    }
    let transcript_path = hook
        .get("transcript_path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            home()
                .ok()
                .and_then(|h| transcript::find_transcript(&h, session))
        });
    let Some(transcript_path) = transcript_path else {
        return;
    };
    for project in projects {
        if let Ok(p) = store.project(&project) {
            if let Err(e) = transcript::etl(&transcript_path, &p.dir, session, &store) {
                eprintln!("beagle scout hook: session pipeline: {e}");
            }
        }
    }
}

/// `beagle scout hook install`.
pub(super) fn install() -> Result<()> {
    let path = hooks::settings_path(&home()?);
    let exe = env::current_exe().map_err(Error::io("current executable"))?;
    let command = hooks::hook_command(&exe);
    let mut settings = hooks::load(&path)?;
    if hooks::install(&mut settings, &command) {
        let backup = hooks::save(&path, &settings)?;
        println!("installed scout's hooks in {}", path.display());
        if let Some(backup) = backup {
            println!("  previous file saved as {}", backup.display());
        }
        println!("  command: {command}");
        println!(
            "  new Claude Code sessions record their commands; watch them with `beagle scout watch`"
        );
    } else {
        println!(
            "already installed in {} (command: {command})",
            path.display()
        );
    }
    Ok(())
}

/// `beagle scout hook uninstall`.
pub(super) fn uninstall() -> Result<()> {
    let path = hooks::settings_path(&home()?);
    let mut settings = hooks::load(&path)?;
    if hooks::uninstall(&mut settings) {
        let backup = hooks::save(&path, &settings)?;
        println!("removed scout's hooks from {}", path.display());
        if let Some(backup) = backup {
            println!("  previous file saved as {}", backup.display());
        }
    } else {
        println!("scout's hooks are not installed in {}", path.display());
    }
    Ok(())
}

/// `beagle scout hook status`.
pub(super) fn status() -> Result<()> {
    let path = hooks::settings_path(&home()?);
    let settings = hooks::load(&path)?;
    for (event, command) in hooks::status(&settings) {
        if let Some(command) = command {
            println!("{event:<19} installed ({command})");
        } else {
            println!("{event:<19} not installed — run `beagle scout hook install`");
        }
    }
    Ok(())
}
