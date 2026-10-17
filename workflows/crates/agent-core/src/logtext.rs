//! Text-level helpers for log views: making a line safe to print, and
//! reading structure back out of a deterministic stage's output.
//!
//! A script stage prints plain lines, so there is no event type to switch
//! on. What there is, is a convention — and two of them:
//!
//! * the standard logger (`$AGENTD_LOG_SH`): `LEVEL source: message`,
//!   e.g. `WARN pr-watch: gh could not read …`. The level is explicit.
//! * the house style every script already uses: `source: message`, e.g.
//!   `gather: wrote 6 files`. The level is inferred from the words.
//!
//! Anything else is plain output. Nothing is ever dropped or rewritten;
//! the parse only decides colour and where the source name sits.

use serde::Serialize;

/// How a line should be shown. Mirrors the standard logger's levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// `DEBUG` — diagnostic detail.
    Debug,
    /// `INFO` — ordinary progress.
    Info,
    /// `STEP` — the start of a named phase.
    Step,
    /// `OK` — something finished successfully.
    Ok,
    /// `WARN` — degraded but continuing.
    Warn,
    /// `ERROR` — something failed.
    Error,
    /// Plain output with no claim about itself — a command's stdout.
    Out,
}

impl Level {
    /// Lowercase name, as serialised and as CSS classes use it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Step => "step",
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Out => "out",
        }
    }
}

/// A script line, read for structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLine {
    /// Explicit from a logger prefix, otherwise inferred from the wording.
    pub level: Level,
    /// The `source:` prefix, when the line has one.
    pub source: Option<String>,
    /// The rest of the line, level and source removed.
    pub message: String,
}

/// Read one line of script output: the standard logger's `LEVEL source: msg`
/// format exactly, anything else by inference. Never fails — an
/// unrecognised line is [`Level::Out`].
pub fn parse_script_line(line: &str) -> ScriptLine {
    let line = line.trim_end();

    // `LEVEL source: message` / `LEVEL message` — the standard logger.
    if let Some((first, rest)) = line.split_once(' ') {
        if let Some(level) = explicit_level(first) {
            let (source, message) = split_source(rest.trim_start());
            return ScriptLine {
                level,
                source,
                message,
            };
        }
    }

    // `source: message` — the convention gather.sh and pr-watch.sh use.
    let (source, message) = split_source(line);
    // ...unless the "source" is itself a level word: `Error: no such
    // file` is an error, not a message from something called Error.
    if let Some(level) = source
        .as_deref()
        .and_then(|s| explicit_level(&s.to_ascii_uppercase()))
    {
        return ScriptLine {
            level,
            source: None,
            message,
        };
    }
    let level = inferred_level(&message);
    let level = if source.is_none() && level == Level::Info {
        Level::Out
    } else {
        level
    };
    ScriptLine {
        level,
        source,
        message,
    }
}

fn explicit_level(word: &str) -> Option<Level> {
    match word.trim_matches(|c| c == '[' || c == ']' || c == ':') {
        "DEBUG" => Some(Level::Debug),
        "INFO" => Some(Level::Info),
        "STEP" => Some(Level::Step),
        "OK" => Some(Level::Ok),
        "WARN" | "WARNING" => Some(Level::Warn),
        "ERROR" | "FATAL" => Some(Level::Error),
        _ => None,
    }
}

/// `name: rest`, where name is one short token. `https://x` has no space
/// after its colon, and `watch with: x` has a space inside its name, so
/// neither is mistaken for a source.
fn split_source(line: &str) -> (Option<String>, String) {
    if let Some((name, rest)) = line.split_once(": ") {
        let token = !name.is_empty()
            && name.len() <= 32
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
        if token {
            return (Some(name.to_owned()), rest.to_owned());
        }
    }
    (None, line.to_owned())
}

fn inferred_level(message: &str) -> Level {
    let lower = message.to_ascii_lowercase();
    let starts = |w: &str| lower.starts_with(w);
    if starts("error") || starts("fatal") || lower.contains(" failed") || starts("failed") {
        Level::Error
    } else if starts("warn") || starts("warning") {
        Level::Warn
    } else if starts("done") || starts("ok ") || lower == "ok" {
        Level::Ok
    } else {
        Level::Info
    }
}

/// Strip anything the terminal would act on rather than print.
///
/// Run logs carry raw command output, so bodies routinely contain ANSI colour
/// sequences and carriage returns from progress-style printing. A TUI writes
/// span text into its cell buffer as-is, so those reach the terminal intact: a
/// `\r` snaps the cursor to column 0 and a `\x1b[` sequence repositions or
/// recolours it, and the frame gets painted over — borders included. Newlines
/// survive because the log views split on them to build their lines.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // CSI / OSC / two-char escapes: drop ESC and everything through
            // the sequence's final byte.
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for f in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&f) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(f) = chars.next() {
                        if f == '\u{7}' {
                            break;
                        }
                        if f == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(line: &str) -> (Level, Option<String>, String) {
        let p = parse_script_line(line);
        (p.level, p.source, p.message)
    }

    #[test]
    fn the_standard_logger_format_is_read_exactly() {
        assert_eq!(
            parsed("WARN pr-watch: gh could not read the PR"),
            (
                Level::Warn,
                Some("pr-watch".to_owned()),
                "gh could not read the PR".to_owned()
            )
        );
        assert_eq!(parsed("ERROR gather: no ticket").0, Level::Error);
        assert_eq!(parsed("OK pr-watch: done").0, Level::Ok);
        assert_eq!(
            parsed("STEP gather: triggering linear-context").0,
            Level::Step
        );
    }

    #[test]
    fn the_house_style_gets_its_source_and_an_inferred_level() {
        assert_eq!(
            parsed("gather: wrote 6 files"),
            (
                Level::Info,
                Some("gather".to_owned()),
                "wrote 6 files".to_owned()
            )
        );
        assert_eq!(
            parsed("pr-watch: WARNING gh could not read x").0,
            Level::Warn
        );
        assert_eq!(parsed("deploy: error: exit 2").0, Level::Error);
    }

    #[test]
    fn urls_and_phrases_are_not_mistaken_for_a_source() {
        let url = parse_script_line("https://github.com/a/b/pull/1");
        assert_eq!(url.source, None);
        let phrase = parse_script_line("watch with: agentctl logs x -f");
        assert_eq!(phrase.source, None);
        assert_eq!(phrase.level, Level::Out, "unclaimed plain output");
    }

    #[test]
    fn a_bare_line_is_plain_output_unless_it_says_otherwise() {
        assert_eq!(parse_script_line("linear-context-2026").level, Level::Out);
        assert_eq!(parse_script_line("Error: no such file").level, Level::Error);
    }

    #[test]
    fn sanitize_strips_terminal_control_but_keeps_newlines() {
        assert_eq!(sanitize("a\u{1b}[31mred\u{1b}[0m\r\nb"), "ared\nb");
    }
}
