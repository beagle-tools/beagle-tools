//! Just enough markdown for summaries and comments: headings, bold,
//! inline code, fenced blocks (ASCII diagrams keep their shape), lists.
//! Lines are not wrapped here; the Paragraph wraps prose, and code
//! blocks are short enough in practice.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// `source` as styled lines.
pub fn render(source: &str) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut in_code = false;
    for raw in source.lines() {
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(Line::styled(
                format!("  {}", raw.replace('\t', "    ")),
                Style::default().fg(Color::Yellow),
            ));
            continue;
        }
        let trimmed = raw.trim_start();
        if trimmed.starts_with('#') {
            let text = trimmed.trim_start_matches('#').trim().to_owned();
            out.push(Line::styled(
                text,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            continue;
        }
        out.push(Line::from(inline(raw)));
    }
    out
}

/// `**bold**` and `` `code` `` within a line.
fn inline(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let bold = rest.find("**");
        let code = rest.find('`');
        let (at, marker) = match (bold, code) {
            (Some(b), Some(c)) if c < b => (c, "`"),
            (Some(b), _) => (b, "**"),
            (None, Some(c)) => (c, "`"),
            (None, None) => break,
        };
        let after = &rest[at + marker.len()..];
        let Some(end) = after.find(marker) else { break };
        if at > 0 {
            spans.push(Span::raw(rest[..at].to_owned()));
        }
        let style = if marker == "`" {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(after[..end].to_owned(), style));
        rest = &after[end + marker.len()..];
    }
    if !rest.is_empty() {
        spans.push(Span::raw(rest.to_owned()));
    }
    spans
}

#[cfg(test)]
// Tests panic on a broken fixture; that is the failure mode we want.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn styles_headings_bold_code_and_fences() {
        let lines = render("# Title\n**Mergeable** — `x` ok\n```\na -> b\n```\nplain **open");
        assert_eq!(text(&lines[0]), "Title");
        assert_eq!(text(&lines[1]), "Mergeable — x ok");
        assert!(lines[1].spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD));
        assert_eq!(text(&lines[2]), "  a -> b");
        assert_eq!(text(&lines[3]), "plain **open");
    }
}
