//! Exposure checks for a Mac that serves (`beagle doctor`, and later
//! `beagle server-mode on`): what answers on networks beyond this Mac and
//! its tailnet. Each problem is a warning that names its fix, never an
//! error.
//!
//! - Remote Login (sshd) accepts passwords ([`sshd`]).
//! - A TCP listener on every interface (or on a LAN address): anything but
//!   loopback and Tailscale's `100.64.0.0/10` / `fd7a:115c:a1e0::/48`.
//!   `lsof` without root sees only this user's processes.
//! - Firewall stealth mode is off.
//! - Screen Sharing or Remote Management is on.
//! - `beagle serve`'s pairing token is older than [`TOKEN_MAX_DAYS`]
//!   (an unknown age is not a warning).
//!
//! On a Mac enrolled in MDM the firewall and Remote Management belong to
//! its administrators, so those warnings say to ask IT instead of how to
//! change them.
//!
//! [`warnings`] is pure over [`Inputs`]; [`gather`] collects them on this
//! machine (read-only commands, none needing root).

pub mod sshd;

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::process;

/// A pairing token older than this many days is worth rotating.
pub const TOKEN_MAX_DAYS: u64 = 90;

/// How long each probe command may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The macOS firewall's command-line tool.
const SOCKETFILTERFW: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";

/// Remote Management's agent; its listener is reported as Remote Management.
const ARD_AGENT: &str = "ARDAgent";

/// What the checks look at. `None` means "could not tell": no warning.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// sshd's config directory (`/etc/ssh`).
    pub etc_ssh: PathBuf,
    /// `launchctl print-disabled system`.
    pub launchd_disabled: Option<String>,
    /// `lsof -nP -iTCP -sTCP:LISTEN -F cn`.
    pub listeners: Option<String>,
    /// `socketfilterfw --getstealthmode`.
    pub stealth: Option<String>,
    /// `profiles status -type enrollment`: an MDM-managed Mac's firewall
    /// and Remote Management belong to its administrators.
    pub mdm: Option<String>,
    /// When the pairing token was made.
    pub token_created: Option<SystemTime>,
    /// Now, for the token's age.
    pub now: SystemTime,
}

/// The fix for a setting an MDM administrator owns.
const MANAGED_FIX: &str = "this Mac is managed by MDM, so that setting is your IT \
                           administrators': ask them rather than changing it";

/// Whether `profiles status -type enrollment` says this Mac is enrolled in MDM.
#[must_use]
pub fn mdm_enrolled(output: &str) -> bool {
    output.lines().any(|line| {
        line.split_once(':').is_some_and(|(key, value)| {
            key.trim().eq_ignore_ascii_case("MDM enrollment")
                && value.trim().to_ascii_lowercase().starts_with("yes")
        })
    })
}

/// Every exposure warning for `inputs`, each with its fix.
#[must_use]
pub fn warnings(inputs: &Inputs) -> Vec<String> {
    let mut out = Vec::new();
    let managed = inputs.mdm.as_deref().is_some_and(mdm_enrolled);
    let disabled = inputs.launchd_disabled.as_deref();
    if disabled.and_then(|d| service_enabled(d, "com.openssh.sshd")) == Some(true) {
        out.extend(ssh_warning(&inputs.etc_ssh));
    }
    if let Some(lsof) = &inputs.listeners {
        let listeners = exposed_listeners(lsof);
        if let Some((_, port)) = listeners.iter().find(|(cmd, _)| cmd == ARD_AGENT) {
            let fix = if managed {
                MANAGED_FIX
            } else {
                "turn Remote Management off in System Settings → General → Sharing"
            };
            out.push(format!(
                "Remote Management's agent ({ARD_AGENT}) listens on port {port} on every network \
                 — {fix}"
            ));
        }
        for (cmd, port) in listeners.iter().filter(|(cmd, _)| cmd != ARD_AGENT) {
            out.push(format!(
                "`{cmd}` listens on port {port} on every network — bind it to 127.0.0.1 or the \
                 Tailscale address (Docker: publish as 127.0.0.1:{port}:…), or stop it"
            ));
        }
    }
    if disabled.and_then(|d| service_enabled(d, "com.apple.screensharing")) == Some(true) {
        out.push(
            "Screen Sharing is on — turn it off in System Settings → General → Sharing → \
             Screen Sharing (or allow only the users who need it)"
                .to_owned(),
        );
    }
    if inputs.stealth.as_deref().and_then(stealth_on) == Some(false) {
        out.push(if managed {
            format!("firewall stealth mode is off — {MANAGED_FIX}")
        } else {
            format!(
                "firewall stealth mode is off — turn it on: \
                 `sudo {SOCKETFILTERFW} --setstealthmode on`"
            )
        });
    }
    if let Some(days) = token_age_days(inputs.token_created, inputs.now) {
        if days > TOKEN_MAX_DAYS {
            out.push(format!(
                "beagle serve's pairing token is {days} days old — rotate it with \
                 `beagle serve pair --rotate` and pair the phone again"
            ));
        }
    }
    out
}

/// The SSH warning, when a password can get in.
fn ssh_warning(etc_ssh: &Path) -> Option<String> {
    let auth = sshd::password_auth(etc_ssh);
    let methods = auth.password_methods();
    if methods.is_empty() {
        return None;
    }
    Some(format!(
        "Remote Login (SSH) accepts {} logins (PasswordAuthentication {} from {}; \
         KbdInteractiveAuthentication {} from {}) — put `PasswordAuthentication no` and \
         `KbdInteractiveAuthentication no` in a file that sorts first in \
         /etc/ssh/sshd_config.d/ (sshd keeps the first value it reads), check your key \
         logs in, then `sudo launchctl kickstart -k system/com.openssh.sshd`",
        methods.join(" and "),
        auth.password.value,
        auth.password.from,
        auth.keyboard_interactive.value,
        auth.keyboard_interactive.from,
    ))
}

/// Whether launchd's `print-disabled` list has `label` enabled: `None`
/// when it isn't listed (launchd then uses the job's own default).
#[must_use]
pub fn service_enabled(print_disabled: &str, label: &str) -> Option<bool> {
    let quoted = format!("\"{label}\"");
    print_disabled.lines().find_map(|line| {
        let (name, state) = line.split_once("=>")?;
        (name.trim() == quoted).then(|| matches!(state.trim(), "enabled" | "false"))
    })
}

/// `socketfilterfw --getstealthmode`'s answer: `None` if unrecognised.
#[must_use]
pub fn stealth_on(output: &str) -> Option<bool> {
    let text = output.to_ascii_lowercase();
    if text.contains("is off") || text.contains("disabled") {
        Some(false)
    } else if text.contains("is on") || text.contains("enabled") {
        Some(true)
    } else {
        None
    }
}

/// Listeners reachable beyond this Mac and the tailnet, from `lsof -F cn`
/// output, as `(command, port)`, deduplicated, in order.
#[must_use]
pub fn exposed_listeners(lsof: &str) -> Vec<(String, u16)> {
    let mut command = String::new();
    let mut out: Vec<(String, u16)> = Vec::new();
    for line in lsof.lines() {
        if let Some(c) = line.strip_prefix('c') {
            c.clone_into(&mut command);
        } else if let Some(addr) = line.strip_prefix('n') {
            let Some((host, port)) = addr.rsplit_once(':') else {
                continue;
            };
            let Ok(port) = port.parse::<u16>() else {
                continue;
            };
            if exposed_host(host) && !out.iter().any(|(c, p)| *c == command && *p == port) {
                out.push((command.clone(), port));
            }
        }
    }
    out
}

/// Whether a listener bound to `host` answers beyond loopback and Tailscale.
fn exposed_host(host: &str) -> bool {
    if host == "*" {
        return true;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let bare = bare.split('%').next().unwrap_or(bare);
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let [a, b, ..] = ip.octets();
            !(ip.is_loopback() || (a == 100 && (64..128).contains(&b)))
        }
        Ok(IpAddr::V6(ip)) => {
            let s = ip.segments();
            !(ip.is_loopback() || (s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0))
        }
        Err(_) => false,
    }
}

/// Whole days since `created`; `None` when unknown (or in the future).
#[must_use]
pub fn token_age_days(created: Option<SystemTime>, now: SystemTime) -> Option<u64> {
    let age = now.duration_since(created?).ok()?;
    Some(age.as_secs() / 86_400)
}

/// The warnings for this machine. Off macOS only the token's age is
/// checked. `token_created` comes from `beagle serve`'s config.
#[must_use]
pub fn gather(token_created: Option<SystemTime>) -> Vec<String> {
    let mac = cfg!(target_os = "macos");
    let run = |program: &str, args: &[&str]| {
        if !mac {
            return None;
        }
        let out =
            process::output_with_timeout(Command::new(program).args(args), PROBE_TIMEOUT).ok()?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    warnings(&Inputs {
        etc_ssh: PathBuf::from("/etc/ssh"),
        launchd_disabled: run("/bin/launchctl", &["print-disabled", "system"]),
        listeners: run(
            "/usr/sbin/lsof",
            &["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "cn"],
        ),
        stealth: run(SOCKETFILTERFW, &["--getstealthmode"]),
        mdm: run("/usr/bin/profiles", &["status", "-type", "enrollment"]),
        token_created,
        now: SystemTime::now(),
    })
}

#[cfg(test)]
#[path = "exposure/tests.rs"]
mod tests;
