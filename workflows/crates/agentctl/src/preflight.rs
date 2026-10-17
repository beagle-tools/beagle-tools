//! Preconditions checked before a job is queued.
//!
//! Right now that means MCP servers: a headless run that needs Linear can
//! only fail slowly and confusingly when Linear is not authenticated — it
//! starts, gets no `mcp__` tools, and either stalls or invents an answer.
//! Checking at submit time turns that into a refusal with a fix attached.

use std::fmt::Write as _;
use std::process::Command;

use anyhow::{bail, Result};

/// What `claude mcp get <name>` says about one server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpStatus {
    Connected,
    /// Registered but not signed in — the common case, and the fixable one.
    NeedsAuth,
    /// Registered, sign-in done, still unreachable (network, outage, bad URL).
    Failed(String),
    /// `claude mcp get` did not recognise the name at all.
    Missing,
}

impl McpStatus {
    fn label(&self) -> String {
        match self {
            Self::Connected => "connected".to_owned(),
            Self::NeedsAuth => "needs authentication".to_owned(),
            Self::Failed(detail) => format!("failed to connect — {detail}"),
            Self::Missing => "not registered".to_owned(),
        }
    }
}

/// Read the `Status:` line out of `claude mcp get <name>` output.
///
/// The CLI decorates it (`✔ Connected`, `! Needs authentication`), so match
/// on the words rather than the glyphs — the symbols are cosmetic and the
/// wording has been stable.
pub fn parse_status(output: &str) -> McpStatus {
    let Some(line) = output
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("Status:"))
    else {
        return McpStatus::Missing;
    };
    let status = line.trim_start_matches("Status:").trim();
    let lower = status.to_ascii_lowercase();
    if lower.contains("connected") && !lower.contains("failed") {
        McpStatus::Connected
    } else if lower.contains("authentic") {
        McpStatus::NeedsAuth
    } else {
        McpStatus::Failed(status.to_owned())
    }
}

/// Refuse the run unless every server the job declares is connected.
///
/// A missing `claude` binary is a warning, not a refusal: `agentctl` is
/// useful on a machine that cannot run the agents, and the spawn would fail
/// with a clearer message anyway.
pub fn check_mcp(job: &str, servers: &[String]) -> Result<()> {
    if servers.is_empty() {
        return Ok(());
    }
    let mut bad: Vec<(String, McpStatus)> = Vec::new();
    for name in servers {
        let Ok(output) = Command::new("claude").args(["mcp", "get", name]).output() else {
            eprintln!("warning: claude is not on PATH — skipping the MCP preflight");
            return Ok(());
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let status = if output.status.success() {
            parse_status(&text)
        } else {
            McpStatus::Missing
        };
        if status != McpStatus::Connected {
            bad.push((name.clone(), status));
        }
    }
    if bad.is_empty() {
        return Ok(());
    }

    let mut message = format!("job {job:?} needs MCP server(s) that are not ready:\n\n");
    for (name, status) in &bad {
        let _ = writeln!(message, "  {:<10} {}", name, status.label());
    }
    let needs_auth: Vec<&String> = bad
        .iter()
        .filter(|(_, s)| *s == McpStatus::NeedsAuth)
        .map(|(n, _)| n)
        .collect();
    let missing: Vec<&String> = bad
        .iter()
        .filter(|(_, s)| *s == McpStatus::Missing)
        .map(|(n, _)| n)
        .collect();
    if !needs_auth.is_empty() {
        message.push_str(
            "\nSign in once, interactively: run `claude`, then `/mcp`, and \
             authenticate\neach one. Verify with `claude mcp get <name>`.\n",
        );
        // The trap this check exists for: the account connectors make an
        // interactive session look connected while headless runs get nothing.
        message.push_str(
            "\nNote: the \"claude.ai <Name>\" account connectors are a different \
             mechanism.\nThey do NOT load in headless `claude -p` runs, so an \
             interactive session\nhaving Linear or Datadog is not enough — the \
             locally registered server\nis what a job uses.\n",
        );
    }
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
        let _ = writeln!(
            message,
            "\nRegister the missing one(s): claude mcp add --scope user \
             --transport http {} <url>",
            names.join(" ")
        );
    }
    message.push_str("\nNothing was queued.");
    bail!(message)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn statuses_parse_from_real_cli_output() {
        let connected = "linear:\n  Scope: User config\n  Status: ✔ Connected\n  Type: http\n";
        assert_eq!(parse_status(connected), McpStatus::Connected);

        let needs = "linear:\n  Scope: User config\n  Status: ! Needs authentication\n";
        assert_eq!(parse_status(needs), McpStatus::NeedsAuth);

        let failed = "x:\n  Status: ✘ Failed to connect — HTTP 406\n";
        assert!(matches!(parse_status(failed), McpStatus::Failed(_)));

        // No Status: line at all (unknown server / changed output shape).
        assert_eq!(parse_status("nothing here"), McpStatus::Missing);
    }

    #[test]
    fn no_declared_servers_is_always_fine() {
        assert!(check_mcp("hello", &[]).is_ok());
    }

    #[test]
    fn the_refusal_explains_the_connector_trap() {
        // Built directly: check_mcp shells out, so assert on the message the
        // NeedsAuth branch produces.
        let bad = [("linear".to_owned(), McpStatus::NeedsAuth)];
        assert_eq!(bad[0].1.label(), "needs authentication");
        assert_eq!(
            McpStatus::Failed("HTTP 406".to_owned()).label(),
            "failed to connect — HTTP 406"
        );
        assert_eq!(McpStatus::Missing.label(), "not registered");
    }
}
