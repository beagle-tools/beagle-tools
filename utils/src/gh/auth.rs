//! Which accounts `gh` is logged into, and a token for one of them.
//!
//! `gh` can hold several github.com accounts at once; one is "active" and
//! answers a plain `gh auth token`. A tool that writes to GitHub as a
//! person should name the account (`gh auth token -u <login>`) rather than
//! take whichever happens to be active.

use serde_json::Value;

/// A github.com account `gh` is logged into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// The GitHub login.
    pub login: String,
    /// Whether it is gh's active account (what a plain `gh auth token` uses).
    pub active: bool,
}

/// The github.com accounts in `gh auth status --json hosts` output, in
/// gh's order. `None` when it is not that JSON.
#[must_use]
pub fn parse_status_json(json: &str) -> Option<Vec<Account>> {
    let value: Value = serde_json::from_str(json).ok()?;
    let hosts = value.get("hosts")?.as_object()?;
    let Some(entries) = hosts.get("github.com") else {
        return Some(Vec::new());
    };
    let accounts = entries
        .as_array()?
        .iter()
        .filter_map(|e| {
            let login = e.get("login")?.as_str()?.trim();
            (!login.is_empty()).then(|| Account {
                login: login.to_owned(),
                active: e.get("active").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect();
    Some(accounts)
}

/// The github.com accounts in plain `gh auth status` text, for a `gh` too
/// old for `--json`:
///
/// ```text
/// github.com
///   ✓ Logged in to github.com account octo-personal (keyring)
///   - Active account: true
/// ```
#[must_use]
pub fn parse_status_text(text: &str) -> Vec<Account> {
    const LOGGED_IN: &str = "Logged in to github.com account ";
    let mut accounts: Vec<Account> = Vec::new();
    for line in text.lines() {
        if let Some((_, rest)) = line.split_once(LOGGED_IN) {
            if let Some(login) = rest.split_whitespace().next() {
                accounts.push(Account {
                    login: login.to_owned(),
                    active: false,
                });
            }
        } else if line.trim_start_matches([' ', '-']).trim() == "Active account: true" {
            if let Some(last) = accounts.last_mut() {
                last.active = true;
            }
        }
    }
    accounts
}
