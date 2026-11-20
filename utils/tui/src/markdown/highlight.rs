//! Lightweight, dependency-free syntax highlighting for fenced code. A
//! single-pass tokenizer colours comments, strings, numbers, and a
//! language's keywords; everything else is left plain. Deliberately
//! approximate — good enough to read a command or a snippet, never a full
//! language grammar. Unknown languages fall through to the generic rules
//! (strings + numbers + `#` comments), so nothing is ever worse than plain.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// Highlights one line of code in `lang` into styled spans.
pub(super) fn code(lang: &str, line: &str) -> Vec<Span<'static>> {
    let kws = keywords(lang);
    let comment = Style::default().fg(Color::DarkGray);
    let string = Style::default().fg(Color::Green);
    let number = Style::default().fg(Color::Yellow);
    let keyword = Style::default().fg(Color::Magenta);
    let plain = Style::default().fg(Color::Gray);

    let chars: Vec<char> = line.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // `#` (outside a string) runs a comment to end of line — the
        // comment style for python, bash, and the generic fallback.
        if c == '#' {
            spans.push(Span::styled(chars[i..].iter().collect::<String>(), comment));
            break;
        }
        if c == '"' || c == '\'' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != c {
                i += 1;
            }
            i = (i + 1).min(chars.len()); // include the closing quote if present
            spans.push(Span::styled(
                chars[start..i].iter().collect::<String>(),
                string,
            ));
            continue;
        }
        if is_word(c) {
            let start = i;
            while i < chars.len() && is_word(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let style = if word.chars().all(|c| c.is_ascii_digit() || c == '.') {
                number
            } else if kws.contains(&word.as_str()) {
                keyword
            } else {
                plain
            };
            spans.push(Span::styled(word, style));
            continue;
        }
        // Operators/punctuation/whitespace: coalesce a run as plain.
        let start = i;
        while i < chars.len() && !is_word(chars[i]) && !matches!(chars[i], '#' | '"' | '\'') {
            i += 1;
        }
        spans.push(Span::styled(
            chars[start..i].iter().collect::<String>(),
            plain,
        ));
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), plain));
    }
    spans
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.'
}

/// Keywords per language. Approximate sets — the common ones that make a
/// snippet scannable, not the full grammar.
fn keywords(lang: &str) -> &'static [&'static str] {
    match lang.to_ascii_lowercase().as_str() {
        "python" | "py" => &[
            "def", "class", "import", "from", "as", "return", "if", "elif", "else", "for", "while",
            "in", "is", "not", "and", "or", "try", "except", "finally", "with", "lambda", "yield",
            "raise", "pass", "break", "continue", "global", "nonlocal", "assert", "del", "None",
            "True", "False", "self", "async", "await",
        ],
        "bash" | "sh" | "shell" | "zsh" | "console" => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "in", "return", "exit", "export", "local", "set", "echo", "cd", "source",
        ],
        "rust" | "rs" => &[
            "fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "use", "mod", "match",
            "if", "else", "for", "while", "loop", "return", "self", "Self", "async", "await",
            "move", "ref", "const", "static", "where", "as", "dyn", "true", "false",
        ],
        _ => &[],
    }
}
