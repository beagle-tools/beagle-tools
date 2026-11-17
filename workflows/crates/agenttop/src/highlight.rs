//! A small YAML highlighter for the CONFIG pane: keys, strings, numbers,
//! booleans/null, comments, list dashes, and block scalars (`|`, `>`)
//! whose indented body reads as one string. Not a parser — it colours
//! lines. Mirrors Agentdesk's `desktop/src/lib/yaml.ts`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const KEY: Style = Style::new().fg(Color::Cyan);
const STRING: Style = Style::new().fg(Color::Green);
const NUMBER: Style = Style::new().fg(Color::Yellow);
const LITERAL: Style = Style::new().fg(Color::Magenta);
const PUNCT: Style = Style::new().fg(Color::Blue);
const COMMENT: Style = Style::new()
    .fg(Color::DarkGray)
    .add_modifier(Modifier::ITALIC);

const LITERALS: [&str; 8] = ["true", "false", "null", "~", "yes", "no", "on", "off"];

/// Every line of a manifest, coloured.
pub fn yaml(text: &str) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    // Inside a block scalar: lines indented past this are its body.
    let mut block_indent: Option<usize> = None;
    for raw in text.lines() {
        let indent = raw.len() - raw.trim_start().len();
        if let Some(at) = block_indent {
            if raw.trim().is_empty() || indent > at {
                out.push(Line::from(Span::styled(raw.to_owned(), STRING)));
                continue;
            }
            block_indent = None;
        }
        out.push(line(raw));
        let code = comment_at(raw).map_or(raw, |i| &raw[..i]).trim_end();
        if opens_block(code) {
            block_indent = Some(indent);
        }
    }
    out
}

/// One line, outside a block scalar.
fn line(raw: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let (code, comment) = match comment_at(raw) {
        Some(i) => (&raw[..i], &raw[i..]),
        None => (raw, ""),
    };
    let indent = code.len() - code.trim_start().len();
    if indent > 0 {
        spans.push(Span::raw(code[..indent].to_owned()));
    }
    let mut rest = &code[indent..];
    // List dashes, possibly nested on one line: "- - x".
    while let Some(after) = rest.strip_prefix('-') {
        if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
            break;
        }
        let gap = after.len() - after.trim_start().len();
        spans.push(Span::styled(rest[..=gap].to_owned(), PUNCT));
        rest = &after[gap..];
    }
    if let Some((key, gap, after)) = split_key(rest) {
        spans.push(Span::styled(key.to_owned(), KEY));
        if !gap.is_empty() {
            spans.push(Span::raw(gap.to_owned()));
        }
        spans.push(Span::styled(":", PUNCT));
        rest = after;
    }
    spans.extend(value(rest));
    if !comment.is_empty() {
        spans.push(Span::styled(comment.to_owned(), COMMENT));
    }
    Line::from(spans)
}

/// `key: value` → (key, space before the colon, value). A key is quoted,
/// or plain text without `:` or `#`, followed by `:` and a space or the
/// end of the line — so `https://x` is never read as a key.
fn split_key(text: &str) -> Option<(&str, &str, &str)> {
    let end = if text.starts_with('"') || text.starts_with('\'') {
        let quote = text.chars().next()?;
        text[1..].find(quote)? + 2
    } else {
        text.find([':', '#'])?
    };
    let key = &text[..end];
    let tail = &text[end..];
    let gap_len = tail.len() - tail.trim_start().len();
    let after_gap = &tail[gap_len..];
    let after = after_gap.strip_prefix(':')?;
    if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
        return None;
    }
    let key = key.trim_end();
    if key.is_empty() {
        return None;
    }
    Some((key, &tail[..gap_len], after))
}

fn value(text: &str) -> Vec<Span<'static>> {
    let core = text.trim();
    if core.is_empty() {
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![Span::raw(text.to_owned())]
        };
    }
    let lead = &text[..text.len() - text.trim_start().len()];
    // Unquoted scalars are strings too: in a manifest they are names,
    // paths and commands, and reading them as values is the point.
    let style = if LITERALS.contains(&core.to_lowercase().as_str()) {
        LITERAL
    } else if core.parse::<f64>().is_ok() {
        NUMBER
    } else if is_block_marker(core) || core.chars().all(|c| "[]{}".contains(c)) {
        PUNCT
    } else {
        STRING
    };
    let mut out = Vec::new();
    if !lead.is_empty() {
        out.push(Span::raw(lead.to_owned()));
    }
    out.push(Span::styled(core.to_owned(), style));
    out
}

/// `|`, `>`, `|-`, `>+2` — the start of a block scalar.
fn is_block_marker(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('|' | '>'))
        && chars.all(|c| c == '-' || c == '+' || c.is_ascii_digit())
}

fn opens_block(code: &str) -> bool {
    code.rsplit_once(':')
        .is_some_and(|(_, after)| is_block_marker(after.trim()))
}

/// Where an unquoted comment starts: `#` at the start or after whitespace.
fn comment_at(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && prev_space => return Some(i),
            None => {}
        }
        prev_space = c.is_whitespace();
    }
    None
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn styled(line: &Line<'_>) -> Vec<(String, Style)> {
        line.spans
            .iter()
            .map(|s| (s.content.to_string(), s.style))
            .collect()
    }

    #[test]
    fn keys_values_and_comments_get_their_colours() {
        let lines = yaml(
            "kind: Agent # the kind\ntimeout_minutes: 25\nenabled: true\nschedule: \"0 12 * * *\"\n",
        );
        assert_eq!(
            styled(&lines[0]),
            vec![
                ("kind".to_owned(), KEY),
                (":".to_owned(), PUNCT),
                (" ".to_owned(), Style::new()),
                ("Agent".to_owned(), STRING),
                ("# the kind".to_owned(), COMMENT),
            ]
        );
        assert!(styled(&lines[1]).contains(&("25".to_owned(), NUMBER)));
        assert!(styled(&lines[2]).contains(&("true".to_owned(), LITERAL)));
        assert!(styled(&lines[3]).contains(&("\"0 12 * * *\"".to_owned(), STRING)));
    }

    #[test]
    fn list_items_urls_and_block_scalars() {
        let lines = yaml(
            "triggers:\n  - review\nurl: https://x.y/z\nbody: |\n  line one: not a key\nnext: 1\n",
        );
        assert!(styled(&lines[1]).contains(&("- ".to_owned(), PUNCT)));
        assert!(styled(&lines[1]).contains(&("review".to_owned(), STRING)));
        // The URL's own colon does not start a key.
        assert!(styled(&lines[2]).contains(&("https://x.y/z".to_owned(), STRING)));
        // A block scalar's body is one string, colons and all.
        assert_eq!(
            styled(&lines[4]),
            vec![("  line one: not a key".to_owned(), STRING)]
        );
        assert!(styled(&lines[5]).contains(&("next".to_owned(), KEY)));
    }

    #[test]
    fn a_hash_inside_quotes_is_not_a_comment() {
        let lines = yaml("command: \"echo #1\"\n");
        assert!(!styled(&lines[0]).iter().any(|(_, s)| *s == COMMENT));
    }
}
