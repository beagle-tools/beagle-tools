//! Everything server mode asks of the Mac beyond its own processes: power,
//! the screen lock, `beagle serve`, agentd and the exposure checks. Tests
//! use a fake [`Host`], so they never depend on this Mac's battery, its
//! screen-lock settings or its running services.

use std::fmt::Debug;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use beagle_utils::process::output_with_timeout;

use crate::procs::{output, Programs};

/// A launchd agent's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Svc {
    NotInstalled,
    Stopped,
    Running,
}

/// What server mode needs from the machine.
pub(crate) trait Host: Debug {
    /// Running on battery power.
    fn on_battery(&self) -> bool;
    /// Locks the screen; false when it couldn't (no Accessibility access).
    fn lock_screen(&self) -> bool;
    /// The screen-lock delay (`5 seconds`, `immediate`), if known.
    fn lock_delay(&self) -> Option<String>;
    /// Sleeps the display (the fallback when locking fails).
    fn display_sleep(&self);
    /// `beagle serve`'s launchd agent.
    fn serve(&self) -> Result<Svc, String>;
    /// Starts it (installing the agent if needed); says what it did.
    fn serve_start(&self) -> Result<String, String>;
    /// Stops it and unloads the agent; false when it wasn't loaded.
    fn serve_stop(&self) -> Result<bool, String>;
    /// One `GET /v1/health`.
    fn serve_health(&self) -> Result<(), String>;
    /// Where the phone reaches the server, once paired.
    fn serve_url(&self) -> Option<String>;
    /// Rotates the pairing token and shows the QR code.
    fn new_pairing(&self) -> Result<(), String>;
    /// Days until the server's TLS certificate expires.
    fn cert_days_left(&self) -> Result<i64, String>;
    /// agentd's launchd agent.
    fn agentd(&self) -> Result<Svc, String>;
    /// Starts a loaded agentd that isn't running.
    fn agentd_start(&self) -> Result<(), String>;
    /// `beagle doctor`'s exposure warnings.
    fn exposure(&self) -> Vec<String>;
}

/// agentd's launchd label.
const AGENTD_LABEL: &str = "com.agentd.daemon";

/// Bound on one health request.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

/// Ctrl-Cmd-Q, the Apple menu → Lock Screen shortcut. Needs Accessibility
/// access for the terminal; without it `lock` falls back to display sleep.
const LOCK_SCRIPT: &str =
    r#"tell application "System Events" to keystroke "q" using {control down, command down}"#;

/// This Mac.
#[derive(Debug, Clone)]
pub(crate) struct System {
    pub progs: Programs,
    /// The `beagle` binary (for `beagle serve pair --rotate`).
    pub exe: PathBuf,
}

fn serve_config() -> Result<beagle_serve::config::Config, String> {
    let path = beagle_serve::config::path().map_err(|e| e.to_string())?;
    beagle_serve::config::load(&path).map_err(|e| e.to_string())
}

fn stdout_of(program: &str, args: &[&str]) -> Result<String, String> {
    let out = output_with_timeout(Command::new(program).args(args), Duration::from_secs(10))
        .map_err(|e| format!("`{program}`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{program} {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn agentd_target() -> Result<String, String> {
    let uid = stdout_of("id", &["-u"])?;
    Ok(format!("gui/{}/{AGENTD_LABEL}", uid.trim()))
}

impl Host for System {
    fn on_battery(&self) -> bool {
        output(&self.progs.pmset, &["-g", "batt"])
            .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("Battery Power"))
    }

    fn lock_screen(&self) -> bool {
        output(&self.progs.osascript, &["-e", LOCK_SCRIPT]).is_some_and(|o| o.status.success())
    }

    fn lock_delay(&self) -> Option<String> {
        let o = output(&self.progs.sysadminctl, &["-screenLock", "status"])?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        crate::report::lock_delay(&text)
    }

    fn display_sleep(&self) {
        output(&self.progs.pmset, &["displaysleepnow"]);
    }

    fn serve(&self) -> Result<Svc, String> {
        use beagle_serve::service::Status;
        Ok(
            match beagle_serve::service::status().map_err(|e| e.to_string())? {
                Status::NotInstalled => Svc::NotInstalled,
                Status::Loaded { pid: Some(_), .. } => Svc::Running,
                Status::NotLoaded(_) | Status::Loaded { .. } => Svc::Stopped,
            },
        )
    }

    fn serve_start(&self) -> Result<String, String> {
        beagle_serve::service::start().map_err(|e| e.to_string())
    }

    fn serve_stop(&self) -> Result<bool, String> {
        beagle_serve::service::stop().map_err(|e| e.to_string())
    }

    fn serve_health(&self) -> Result<(), String> {
        let url = self
            .serve_url()
            .ok_or("not paired yet — run `beagle serve pair`")?;
        let client = reqwest::blocking::Client::builder()
            .timeout(HEALTH_TIMEOUT)
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(format!("{url}/v1/health"))
            .send()
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("/v1/health answered {}", resp.status()))
        }
    }

    fn serve_url(&self) -> Option<String> {
        let config = serve_config().ok()?;
        let host = config.host.as_deref()?;
        Some(beagle_serve::pair::server_url(host, config.port()))
    }

    fn new_pairing(&self) -> Result<(), String> {
        let status = Command::new(&self.exe)
            .args(["serve", "pair", "--rotate"])
            .status()
            .map_err(|e| format!("`beagle serve pair --rotate`: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("`beagle serve pair --rotate` failed".into())
        }
    }

    fn cert_days_left(&self) -> Result<i64, String> {
        let config = serve_config()?;
        let host = config
            .host
            .clone()
            .ok_or("not paired yet — run `beagle serve pair`")?;
        let dir = beagle_serve::config::cert_dir().map_err(|e| e.to_string())?;
        let (cert, _) = config.cert_paths(&dir, &host);
        if !cert.is_file() {
            return Err(format!(
                "no certificate at {} — `beagle serve pair` shows how to make it",
                cert.display()
            ));
        }
        let text = stdout_of(
            "openssl",
            &["x509", "-enddate", "-noout", "-in", &cert.to_string_lossy()],
        )?;
        let end = crate::report::parse_enddate(&text)
            .ok_or_else(|| format!("can't read the certificate's end date: {}", text.trim()))?;
        Ok((end - crate::session::now()).div_euclid(86_400))
    }

    fn agentd(&self) -> Result<Svc, String> {
        let plist = beagle_utils::paths::home_dir()
            .ok_or("no home directory")?
            .join(format!("Library/LaunchAgents/{AGENTD_LABEL}.plist"));
        if !plist.is_file() {
            return Ok(Svc::NotInstalled);
        }
        let Ok(text) = stdout_of("launchctl", &["print", &agentd_target()?]) else {
            return Ok(Svc::Stopped);
        };
        Ok(match beagle_serve::service::parse_print(&text) {
            (_, Some(_)) => Svc::Running,
            _ => Svc::Stopped,
        })
    }

    fn agentd_start(&self) -> Result<(), String> {
        stdout_of("launchctl", &["kickstart", &agentd_target()?]).map(|_| ())
    }

    fn exposure(&self) -> Vec<String> {
        beagle_utils::exposure::gather(serve_config().ok().and_then(|c| c.token_created_at()))
    }
}
