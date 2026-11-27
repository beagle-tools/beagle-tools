//! Whether sshd lets a password in, read from its config files the way
//! sshd reads them (no root needed, unlike `sshd -T`).
//!
//! sshd keeps the **first** value it reads for a keyword, and `Include`
//! pulls files in at the line it sits on (globs sorted by name), so the
//! files are walked in that order and the first value wins. Lines after a
//! `Match` only apply to some connections; they are not the global setting
//! and are ignored.

use std::path::{Path, PathBuf};

/// How deep `Include`s may nest (sshd's own limit).
const MAX_DEPTH: usize = 16;

/// A setting's value and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The value, lowercased (`yes`, `no`, …).
    pub value: String,
    /// `file:line`, or `the default` when no file sets it.
    pub from: String,
}

/// The settings that decide whether a password gets in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordAuth {
    /// `PasswordAuthentication`.
    pub password: Setting,
    /// `KbdInteractiveAuthentication` (or its old name
    /// `ChallengeResponseAuthentication`): with PAM, also a password prompt.
    pub keyboard_interactive: Setting,
    /// `AuthenticationMethods`, when set: only these methods are offered.
    pub methods: Option<Setting>,
}

impl PasswordAuth {
    /// The methods that accept a password, by their sshd names.
    #[must_use]
    pub fn password_methods(&self) -> Vec<&'static str> {
        let allowed = |name: &str| {
            self.methods.as_ref().map_or(true, |m| {
                m.value == "any" || m.value.split([',', ' ']).any(|method| method == name)
            })
        };
        let mut on = Vec::new();
        if self.password.value == "yes" && allowed("password") {
            on.push("password");
        }
        if self.keyboard_interactive.value == "yes" && allowed("keyboard-interactive") {
            on.push("keyboard-interactive");
        }
        on
    }
}

/// Reads `<etc_ssh>/sshd_config` and what it includes. `etc_ssh` stands in
/// for `/etc/ssh`: absolute includes under `/etc/ssh` are read below it,
/// so tests can use a fake directory. Unreadable files count as empty.
#[must_use]
pub fn password_auth(etc_ssh: &Path) -> PasswordAuth {
    let mut found = Found::default();
    walk(etc_ssh, &etc_ssh.join("sshd_config"), 0, &mut found);
    let default = |s: Option<Setting>| {
        s.unwrap_or_else(|| Setting {
            value: "yes".to_owned(),
            from: "the default".to_owned(),
        })
    };
    PasswordAuth {
        password: default(found.password),
        keyboard_interactive: default(found.keyboard_interactive),
        methods: found.methods,
    }
}

#[derive(Default)]
struct Found {
    password: Option<Setting>,
    keyboard_interactive: Option<Setting>,
    methods: Option<Setting>,
}

fn walk(etc_ssh: &Path, file: &Path, depth: usize, found: &mut Found) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, rest) = line
            .split_once(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or((line, ""));
        let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=');
        let setting = || Setting {
            value: rest.trim().to_ascii_lowercase(),
            from: format!("{}:{}", file.display(), n + 1),
        };
        match key.to_ascii_lowercase().as_str() {
            "match" => return,
            "include" => {
                for pattern in rest.split_whitespace() {
                    for path in expand(etc_ssh, pattern) {
                        walk(etc_ssh, &path, depth + 1, found);
                    }
                }
            }
            "passwordauthentication" => {
                found.password.get_or_insert_with(setting);
            }
            "kbdinteractiveauthentication" | "challengeresponseauthentication" => {
                found.keyboard_interactive.get_or_insert_with(setting);
            }
            "authenticationmethods" => {
                found.methods.get_or_insert_with(setting);
            }
            _ => {}
        }
    }
}

/// An `Include` pattern's files, sorted. Relative patterns are under
/// `/etc/ssh`; a `*` is allowed in the last component only.
fn expand(etc_ssh: &Path, pattern: &str) -> Vec<PathBuf> {
    let path = match pattern.strip_prefix("/etc/ssh") {
        Some(rest) => etc_ssh.join(rest.trim_start_matches('/')),
        None => etc_ssh.join(pattern),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some((prefix, suffix)) = name.split_once('*') else {
        return vec![path];
    };
    let Some(Ok(entries)) = path.parent().map(std::fs::read_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n.len() >= prefix.len() + suffix.len()
                    && n.starts_with(prefix)
                    && n.ends_with(suffix)
            })
        })
        .collect();
    files.sort();
    files
}
