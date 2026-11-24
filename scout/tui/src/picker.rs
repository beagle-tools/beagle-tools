//! Choosing what `beagle scout watch` shows, like `claude --resume`: a fuzzy
//! finder over recorded sessions, titled the way Claude Code titles them.
//!
//! With `fzf` on the PATH it is an fzf picker styled like the `ffg` finder
//! (rounded border, 80% height, header, a preview of each session's recent
//! commands). Without it, a numbered prompt. The first row is always
//! **auto**: follow the newest Claude session in this Herdr tab (or
//! directory) and switch with it. Any other row pins that session.

use std::fmt::Write as _;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::SystemTime;

use beagle_utils::shell::quote_os;

use crate::error::{Error, Result};
use crate::session::{self, SessionInfo};
use crate::watch::{human_age, one_line, short_id};

/// The most sessions offered.
const LIMIT: usize = 50;

/// What the user picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Follow whichever session is newest in this tab/directory.
    Auto,
    /// Pin this session id.
    Session(String),
    /// Nothing; exit.
    Cancel,
}

/// One pickable line: the id fzf hands back, and what the user sees.
struct Row {
    id: String,
    label: String,
}

/// Shows the picker and returns the choice. `exe` is this `beagle` binary,
/// used for fzf's preview command.
///
/// # Errors
/// [`Error::Io`] if the prompt or fzf cannot be driven.
pub fn choose(root: &Path, tab: Option<&str>, exe: &Path) -> Result<Choice> {
    let rows = rows(root, tab);
    if which("fzf") {
        return fzf(&rows, root, exe);
    }
    let stdin = std::io::stdin();
    numbered(&rows, &mut stdin.lock(), &mut std::io::stdout())
}

fn rows(root: &Path, tab: Option<&str>) -> Vec<Row> {
    let auto = if tab.is_some() {
        "● auto — follow the newest Claude session in this Herdr tab"
    } else {
        "● auto — follow the newest Claude session in this directory"
    };
    let mut rows = vec![Row {
        id: "auto".to_owned(),
        label: auto.to_owned(),
    }];
    // No home: paths are shown in full instead of `~`-shortened.
    let home = beagle_utils::paths::home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();
    rows.extend(session::list(root).iter().take(LIMIT).map(|s| Row {
        id: s.id.clone(),
        label: label(s, tab, &home),
    }));
    rows
}

/// `12m  Fix flaky uploader test  ~/GitHub/x/repo  ← this tab  4f85a782  · last: …`
fn label(s: &SessionInfo, tab: Option<&str>, home: &str) -> String {
    let description = session::describe(s);
    let age = SystemTime::now()
        .duration_since(s.modified)
        .unwrap_or_default();
    let title = description
        .title
        .clone()
        .or_else(|| description.last_prompt.clone())
        .or_else(|| {
            s.last_command
                .as_deref()
                .map(|c| format!("$ {}", one_line(c, 60)))
        })
        .unwrap_or_else(|| "(untitled)".to_owned());
    let cwd = s.cwd.as_deref().unwrap_or("-");
    let cwd = match cwd.strip_prefix(home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => cwd.to_owned(),
    };
    let mut label = format!("{:>4}  {}", human_age(age), one_line(&title, 70));
    if tab.is_some() && s.tab.as_deref() == tab {
        label.push_str("  \x1b[33m← this tab\x1b[0m");
    }
    let _ = write!(label, "  \x1b[2m{cwd}  {}", short_id(&s.id));
    // The last prompt adds information only when the title is Claude's own.
    if let (Some(_), Some(prompt)) = (&description.title, &description.last_prompt) {
        let _ = write!(label, " · last: {}", one_line(prompt, 80));
    }
    label.push_str("\x1b[0m");
    label
}

/// fzf's preview command: `beagle scout watch <id> --once` on the same
/// store.
fn preview_command(exe: &Path, root: &Path) -> String {
    format!(
        "{} scout --root {} watch {{1}} --once --history 12 --color",
        quote_os(exe.as_os_str()),
        quote_os(root.as_os_str()),
    )
}

fn fzf(rows: &[Row], root: &Path, exe: &Path) -> Result<Choice> {
    let preview = preview_command(exe, root);
    let mut child = Command::new("fzf")
        .args([
            "--ansi",
            "--no-sort",
            "--layout=reverse",
            "--delimiter=\t",
            "--with-nth=2..",
            "--height=80%",
            "--border=rounded",
            "--prompt=watch › ",
            "--header=Scout sessions | Enter to watch · Esc to cancel",
            "--preview-window=right:50%:wrap",
        ])
        .arg(format!("--preview={preview}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(Error::io("fzf"))?;
    if let Some(mut input) = child.stdin.take() {
        let list = rows.iter().fold(String::new(), |mut list, r| {
            let _ = writeln!(list, "{}	{}", r.id, r.label);
            list
        });
        // fzf may exit early (Esc) and close its stdin; that is not an error.
        let _ = input.write_all(list.as_bytes());
    }
    let output = child.wait_with_output().map_err(Error::io("fzf"))?;
    if !output.status.success() {
        return Ok(Choice::Cancel); // 130 = Esc, 1 = no match
    }
    let picked = String::from_utf8_lossy(&output.stdout);
    Ok(to_choice(
        picked.split('\t').next().unwrap_or_default().trim(),
    ))
}

fn numbered(rows: &[Row], input: &mut impl BufRead, out: &mut impl Write) -> Result<Choice> {
    let strip = |s: &str| {
        let mut plain = String::new();
        let mut in_escape = false;
        for c in s.chars() {
            match (in_escape, c) {
                (false, '\x1b') => in_escape = true,
                (true, 'm') => in_escape = false,
                (false, c) => plain.push(c),
                (true, _) => {}
            }
        }
        plain
    };
    let mut menu = String::from("Watch which session? (install fzf for fuzzy search)\n\n");
    for (i, row) in rows.iter().enumerate() {
        let _ = writeln!(menu, "  {i:>2}) {}", strip(&row.label));
    }
    let _ = write!(
        menu,
        "\n[0-{}, enter = 0 (auto), q = cancel]: ",
        rows.len().saturating_sub(1)
    );
    out.write_all(menu.as_bytes())
        .and_then(|()| out.flush())
        .map_err(Error::io("<stdout>"))?;
    let mut answer = String::new();
    input.read_line(&mut answer).map_err(Error::io("<stdin>"))?;
    let answer = answer.trim();
    if answer.eq_ignore_ascii_case("q") {
        return Ok(Choice::Cancel);
    }
    let index = if answer.is_empty() {
        Some(0)
    } else {
        answer.parse::<usize>().ok()
    };
    Ok(index
        .and_then(|i| rows.get(i))
        .map_or(Choice::Cancel, |row| to_choice(&row.id)))
}

fn to_choice(id: &str) -> Choice {
    match id {
        "" => Choice::Cancel,
        "auto" => Choice::Auto,
        id => Choice::Session(id.to_owned()),
    }
}

/// Whether `program` is on this process's PATH (the one `Command::new`
/// searches).
fn which(program: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    beagle_utils::process::which_in(program, &path).is_some()
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::session::Event;
    use time::macros::datetime;

    fn record(root: &Path, id: &str, tab: &str, transcript: Option<&Path>) {
        let start = Event::Start {
            ts: datetime!(2026-11-12 15:00 UTC),
            id: "1".into(),
            command: format!("echo {id}"),
            description: None,
            cwd: Some("/repo".into()),
            agent: None,
            background: false,
            tab: Some(tab.into()),
            pane: None,
            transcript: transcript.map(|p| p.to_string_lossy().into_owned()),
        };
        session::append(root, id, &start).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    #[test]
    fn rows_start_with_auto_and_carry_titles() {
        let root = tempfile::tempdir().unwrap();
        let transcript = root.path().join("t.jsonl");
        std::fs::write(
            &transcript,
            r#"{"type":"ai-title","aiTitle":"Fix flaky uploader test"}"#,
        )
        .unwrap();
        record(root.path(), "aaaa1111-old", "t2", None);
        record(root.path(), "bbbb2222-new", "t1", Some(&transcript));
        let rows = rows(root.path(), Some("t1"));
        assert_eq!(rows[0].id, "auto");
        assert_eq!(rows[1].id, "bbbb2222-new");
        assert!(rows[1].label.contains("Fix flaky uploader test"));
        assert!(rows[1].label.contains("← this tab"));
        assert!(
            rows[2].label.contains("$ echo aaaa1111-old"),
            "{}",
            rows[2].label
        );
    }

    #[test]
    fn numbered_fallback() {
        let root = tempfile::tempdir().unwrap();
        record(root.path(), "aaaa1111", "t1", None);
        let rows = rows(root.path(), None);
        let ask = |answer: &str| {
            let mut out = Vec::new();
            let choice = numbered(&rows, &mut answer.as_bytes(), &mut out).unwrap();
            let menu = String::from_utf8(out).unwrap();
            assert!(!menu.contains('\x1b'), "colors are stripped: {menu}");
            choice
        };
        assert_eq!(ask("\n"), Choice::Auto);
        assert_eq!(ask("1\n"), Choice::Session("aaaa1111".into()));
        assert_eq!(ask("q\n"), Choice::Cancel);
        assert_eq!(ask("7\n"), Choice::Cancel);
    }

    #[test]
    fn choices_and_quoting() {
        assert_eq!(to_choice("auto"), Choice::Auto);
        assert_eq!(to_choice(""), Choice::Cancel);
        assert_eq!(
            preview_command(Path::new("/a b/beagle"), Path::new("/it's")),
            r"'/a b/beagle' scout --root '/it'\''s' watch {1} --once --history 12 --color"
        );
    }
}
