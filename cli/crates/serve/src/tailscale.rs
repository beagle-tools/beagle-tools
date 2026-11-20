//! What Tailscale knows about this machine: its tailnet IPv4 address and
//! its `MagicDNS` name. Each question is one bounded `tailscale` call.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::error::{Error, Result};

/// How long one `tailscale` call may take.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Where the Mac app keeps its CLI when `tailscale` is not on `PATH`.
const MAC_APP_CLI: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// The `tailscale` CLI: on `PATH` (augmented), else the Mac app's.
fn program() -> PathBuf {
    beagle_utils::process::which("tailscale").unwrap_or_else(|| PathBuf::from(MAC_APP_CLI))
}

/// Runs `tailscale <args>` and returns its stdout.
fn run(args: &[&str]) -> Result<String> {
    let program = program();
    let mut cmd = Command::new(&program);
    cmd.args(args);
    let out = beagle_utils::process::output_with_timeout(&mut cmd, TIMEOUT)
        .map_err(|e| Error::Tailscale(format!("`tailscale {}`: {e}", args.join(" "))))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(Error::Tailscale(format!(
            "`tailscale {}` failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// This machine's tailnet IPv4 address (`tailscale ip -4`).
///
/// # Errors
/// [`Error::Tailscale`] when the CLI is missing, fails, or prints no address.
pub fn ipv4() -> Result<Ipv4Addr> {
    parse_ipv4(&run(&["ip", "-4"])?)
        .ok_or_else(|| Error::Tailscale("`tailscale ip -4` printed no IPv4 address".into()))
}

/// This machine's `MagicDNS` name, e.g. `mac.example-tailnet.ts.net`
/// (`Self.DNSName` from `tailscale status --json`).
///
/// # Errors
/// [`Error::Tailscale`] when the CLI is missing, fails, or has no name.
pub fn dns_name() -> Result<String> {
    parse_dns_name(&run(&["status", "--json"])?).ok_or_else(|| {
        Error::Tailscale("`tailscale status --json` has no Self.DNSName (is MagicDNS on?)".into())
    })
}

/// The first IPv4 address in `tailscale ip -4` output.
#[must_use]
pub fn parse_ipv4(stdout: &str) -> Option<Ipv4Addr> {
    stdout.lines().find_map(|line| line.trim().parse().ok())
}

/// `Self.DNSName` without its trailing dot.
#[must_use]
pub fn parse_dns_name(status_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(status_json).ok()?;
    let name = value.get("Self")?.get("DNSName")?.as_str()?;
    let name = name.trim_end_matches('.');
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_is_the_first_parseable_line() {
        assert_eq!(
            parse_ipv4("100.64.0.7\n"),
            Some(Ipv4Addr::new(100, 64, 0, 7))
        );
        assert_eq!(
            parse_ipv4("warning: x\n100.64.0.8\n"),
            Some(Ipv4Addr::new(100, 64, 0, 8))
        );
        assert_eq!(parse_ipv4(""), None);
    }

    #[test]
    fn dns_name_loses_its_trailing_dot() {
        let json = r#"{"Self": {"DNSName": "mac.example-tailnet.ts.net.", "HostName": "mac"}}"#;
        assert_eq!(
            parse_dns_name(json).as_deref(),
            Some("mac.example-tailnet.ts.net")
        );
        assert_eq!(parse_dns_name(r#"{"Self": {"DNSName": ""}}"#), None);
        assert_eq!(parse_dns_name("{}"), None);
        assert_eq!(parse_dns_name("not json"), None);
    }
}
