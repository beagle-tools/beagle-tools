//! macOS notifications for finished runs, so the pipelines announce
//! themselves. Prefers `terminal-notifier` (icons, grouping), falls back to
//! `osascript` (no icon). Sends are best-effort and never fail a tick.
//!
//! Two triggers:
//! - the notify policy (`all` / `failures` / `off`) against the final state;
//! - a run writing `$AGENTD_DATA_DIR/notify.txt` — "I did something worth
//!   announcing" — which notifies with that text even when the policy would
//!   stay quiet (e.g. a `failures` poller that just picked up a new RCA).
//!   `off` still silences everything.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agent_core::paths;
use agent_core::types::{AgentSpec, NotifyPolicy, Run, RunState};
use beagle_utils::notify::applescript_escape as escape;
use chrono::Utc;
use tracing::debug;

const TITLE: &str = "agentd";
const CUSTOM_MESSAGE_MAX: usize = 240;

/// Daemon-wide notification defaults (`--notify`, `--notify-icon`,
/// `--notify-icon-failure`, `--notify-sound[-failure]`); per-agent manifest
/// fields override the policy and icons.
#[derive(Debug, Clone)]
pub struct NotifyConfig {
    pub policy: NotifyPolicy,
    pub icon: Option<PathBuf>,
    pub icon_failure: Option<PathBuf>,
    pub sound: String,
    pub sound_failure: String,
}

/// Announce a run reaching `state`.
pub fn run_finished(spec: Option<&AgentSpec>, run: &Run, state: RunState, cfg: &NotifyConfig) {
    let policy = spec.and_then(|s| s.notify).unwrap_or(cfg.policy);
    if policy == NotifyPolicy::Off {
        return;
    }
    let custom = custom_message(run);
    if custom.is_none() && !should_notify(policy, state) {
        return;
    }
    // A high-frequency poller (opted into noop pruning) that timed out
    // without saying anything is almost always a slept-through poll — the
    // next tick retries in seconds. Not worth a ❌ on every laptop wake.
    if state == RunState::TimedOut
        && custom.is_none()
        && spec.is_some_and(|s| s.noop_retention_minutes.is_some())
    {
        return;
    }
    let icon = pick_icon(spec, state, cfg);
    let subtitle = format!("{}/{}", run.project, run.agent);
    let started = run.started_at.unwrap_or(run.created_at);
    let secs = (Utc::now() - started).num_seconds().max(0);
    let mut message = match custom {
        Some(text) => text,
        None => format!("{state} · {}", human_secs(secs)),
    };
    let sound = if state == RunState::Succeeded {
        &cfg.sound
    } else {
        message = format!("\u{274c} {message}");
        &cfg.sound_failure
    };
    send(&subtitle, &message, sound, icon.as_deref());
}

/// The run's self-authored announcement, if it wrote one.
fn custom_message(run: &Run) -> Option<String> {
    let dir = paths::data_dir().ok()?;
    let text = std::fs::read_to_string(dir.join(&run.id).join("notify.txt")).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out: String = trimmed.chars().take(CUSTOM_MESSAGE_MAX).collect();
    if trimmed.chars().count() > CUSTOM_MESSAGE_MAX {
        out.push('…');
    }
    Some(out)
}

fn should_notify(policy: NotifyPolicy, state: RunState) -> bool {
    match policy {
        NotifyPolicy::Off => false,
        NotifyPolicy::All => true,
        NotifyPolicy::Failures => state != RunState::Succeeded,
    }
}

/// Failed runs get the failure image (agent override, then daemon default),
/// falling back to the regular icon chain.
fn pick_icon(spec: Option<&AgentSpec>, state: RunState, cfg: &NotifyConfig) -> Option<PathBuf> {
    let regular = || {
        spec.and_then(|s| s.notify_icon.clone())
            .or_else(|| cfg.icon.clone())
    };
    if state == RunState::Succeeded {
        return regular();
    }
    spec.and_then(|s| s.notify_icon_failure.clone())
        .or_else(|| cfg.icon_failure.clone())
        .or_else(regular)
}

fn human_secs(secs: i64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let (h, m) = (secs / 3_600, (secs % 3_600) / 60);
    if h > 0 && m > 0 {
        format!("{h}h{m}m")
    } else if h > 0 {
        format!("{h}h")
    } else {
        format!("{m}m")
    }
}

/// Fire-and-forget delivery; failures are logged at debug and ignored.
/// The icon IS the sending app: `scripts/build-notifier.sh` builds one
/// branded notifier app per image in `assets/notify/`, and the run's icon
/// path picks the matching app (its bundle icon is the only icon macOS will
/// show — `-appIcon`/`-contentImage` overrides are dead in 3.x).
fn send(subtitle: &str, message: &str, sound: &str, icon: Option<&Path>) {
    let Some(notifier) = branded_notifier(icon) else {
        osascript_fallback(subtitle, message, sound);
        return;
    };
    let spawned = Command::new(notifier)
        .args(["-title", TITLE, "-subtitle", subtitle, "-message", message])
        .args(["-sound", sound, "-group", &format!("{TITLE}-{subtitle}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if spawned.is_err() {
        osascript_fallback(subtitle, message, sound);
    }
}

/// The branded app for this icon: `<state>/notifier/<icon stem>.app`,
/// falling back to the stock `agentd` app.
fn branded_notifier(icon: Option<&Path>) -> Option<PathBuf> {
    let dir = paths::state_dir().ok()?.join("notifier");
    let stem = icon
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("agentd");
    for candidate in [stem, "agentd"] {
        let bin = dir.join(format!("{candidate}.app/Contents/MacOS/terminal-notifier"));
        if bin.is_file() {
            return Some(bin);
        }
    }
    None
}

fn osascript_fallback(subtitle: &str, message: &str, sound: &str) {
    // osascript can't show a custom icon, but the text still lands.
    let script = format!(
        "display notification \"{}\" with title \"{TITLE}\" subtitle \"{}\" sound name \"{sound}\"",
        escape(message),
        escape(subtitle)
    );
    if let Err(e) = Command::new("osascript")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        debug!(error = %e, "notification delivery failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_gates_by_state() {
        assert!(should_notify(NotifyPolicy::All, RunState::Succeeded));
        assert!(should_notify(NotifyPolicy::All, RunState::Failed));
        assert!(!should_notify(NotifyPolicy::Failures, RunState::Succeeded));
        assert!(should_notify(NotifyPolicy::Failures, RunState::Failed));
        assert!(should_notify(NotifyPolicy::Failures, RunState::TimedOut));
        assert!(!should_notify(NotifyPolicy::Off, RunState::Failed));
    }

    #[test]
    fn durations_and_escaping() {
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(192), "3m");
        assert_eq!(human_secs(3_720), "1h2m");
        assert_eq!(human_secs(3_600), "1h");
        assert_eq!(escape(r#"say "hi" \now"#), r#"say \"hi\" \\now"#);
    }

    #[test]
    fn failure_states_prefer_the_failure_icon() {
        let cfg = NotifyConfig {
            policy: NotifyPolicy::All,
            icon: Some("/i/ok.png".into()),
            icon_failure: Some("/i/fail.png".into()),
            sound: "Tink".to_owned(),
            sound_failure: "Funk".to_owned(),
        };
        assert_eq!(
            pick_icon(None, RunState::Succeeded, &cfg),
            Some("/i/ok.png".into())
        );
        assert_eq!(
            pick_icon(None, RunState::Failed, &cfg),
            Some("/i/fail.png".into())
        );
        let no_fail = NotifyConfig {
            icon_failure: None,
            ..cfg
        };
        assert_eq!(
            pick_icon(None, RunState::TimedOut, &no_fail),
            Some("/i/ok.png".into())
        );
    }
}
