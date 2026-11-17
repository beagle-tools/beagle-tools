//! A small, panic-free Markdown renderer targeting ratatui [`Text`], adapted
//! from beagle's.
//!
//! It supports the subset project and task files use — headings, bullets,
//! `- [ ]` checklists, code fences, blockquotes, rules, `**bold**` and
//! `` `code` `` — and treats everything else as plain text. One output line
//! per source line, so line counts match the file.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Renders Markdown `source` into styled text. Never fails: unrecognized
/// syntax falls through as plain text, and an unclosed code fence styles the
/// rest of the document as code.
#[must_use]
pub fn to_text(source: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_fence = false;
    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            let corner = if in_code_fence { "╰" } else { "╭" };
            let lang = trimmed.trim_start_matches('`').trim();
            let label = if lang.is_empty() || in_code_fence {
                format!("{corner}{}", "─".repeat(20))
            } else {
                format!("{corner}─ {lang} {}", "─".repeat(16))
            };
            lines.push(Line::styled(label, Style::default().fg(Color::DarkGray)));
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled(raw.to_owned(), Style::default().fg(Color::Green)),
            ]));
            continue;
        }
        lines.push(render_line(raw));
    }
    Text::from(lines)
}

fn render_line(raw: &str) -> Line<'static> {
    let trimmed = raw.trim_start();
    let indent = raw.len() - trimmed.len();

    if let Some(rest) = trimmed.strip_prefix("### ") {
        return heading(rest, Color::Blue, false);
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return heading(rest, Color::Cyan, false);
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return heading(rest, Color::Cyan, true);
    }
    if trimmed == "---" || trimmed == "***" {
        return Line::styled("─".repeat(40), Style::default().fg(Color::DarkGray));
    }
    if let Some(rest) = trimmed
        .strip_prefix("> ")
        .or_else(|| (trimmed == ">").then_some(""))
    {
        let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
        spans.extend(inline_spans(
            rest,
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ));
        return Line::from(spans);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        if let Some((done, text)) = checkbox(rest) {
            let (glyph, glyph_style, text_style) = if done {
                (
                    "☑ ",
                    Style::default().fg(Color::Green),
                    Style::default().fg(Color::Green),
                )
            } else {
                ("☐ ", Style::default().fg(Color::Yellow), Style::default())
            };
            spans.push(Span::styled(glyph, glyph_style));
            spans.extend(inline_spans(text, text_style));
        } else {
            spans.push(Span::styled("• ", Style::default().fg(Color::Yellow)));
            spans.extend(inline_spans(rest, Style::default()));
        }
        return Line::from(spans);
    }
    if trimmed.starts_with('<') && trimmed.trim_end().ends_with('>') {
        // An unfilled template placeholder: dim, so it reads as "to do".
        return Line::styled(
            raw.to_owned(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        );
    }
    Line::from(inline_spans(raw, Style::default()))
}

/// The checkbox at the start of a bullet's content: `[ ]`, `[x]` or `[X]`,
/// alone or followed by a space and the item text.
fn checkbox(rest: &str) -> Option<(bool, &str)> {
    let (done, after) = if let Some(after) = rest.strip_prefix("[ ]") {
        (false, after)
    } else {
        let after = rest
            .strip_prefix("[x]")
            .or_else(|| rest.strip_prefix("[X]"))?;
        (true, after)
    };
    match after.strip_prefix(' ') {
        Some(text) => Some((done, text)),
        None if after.is_empty() => Some((done, "")),
        None => None,
    }
}

/// Splits `source` into `(everything else, the named sections)`. A section
/// runs from its `## Heading` to the next `## ` heading; names match
/// case-insensitively, and headings inside code fences don't count.
#[must_use]
pub fn split_sections(source: &str, names: &[&str]) -> (String, String) {
    let mut rest = String::new();
    let mut picked = String::new();
    let mut in_picked = false;
    let mut in_code_fence = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
        } else if !in_code_fence {
            if let Some(heading) = trimmed.strip_prefix("## ") {
                let heading = heading.trim();
                in_picked = names.iter().any(|n| n.eq_ignore_ascii_case(heading));
            }
        }
        let out = if in_picked { &mut picked } else { &mut rest };
        out.push_str(line);
        out.push('\n');
    }
    (rest, picked)
}

/// Counts checkboxes in `source` as `(checked, total)`, skipping code fences.
#[must_use]
pub fn checklist_stats(source: &str) -> (usize, usize) {
    let mut checked = 0;
    let mut total = 0;
    let mut in_code_fence = false;
    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if let Some((done, _)) = checkbox(rest) {
                total += 1;
                checked += usize::from(done);
            }
        }
    }
    (checked, total)
}

fn heading(text: &str, color: Color, top_level: bool) -> Line<'static> {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let style = if top_level {
        style.add_modifier(Modifier::UNDERLINED)
    } else {
        style
    };
    Line::styled(text.to_owned(), style)
}

/// Splits a line into spans, honoring `` `code` `` and `**bold**`. Unbalanced
/// markers are emitted literally, so a stray `**` never eats the line.
fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                flush(&mut spans, &mut plain, base);
                spans.push(Span::styled(
                    after[..end].to_owned(),
                    base.add_modifier(Modifier::BOLD),
                ));
                rest = &after[end + 2..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`') {
                flush(&mut spans, &mut plain, base);
                spans.push(Span::styled(after[..end].to_owned(), base.fg(Color::Cyan)));
                rest = &after[end + 1..];
                continue;
            }
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    flush(&mut spans, &mut plain, base);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

fn flush(spans: &mut Vec<Span<'static>>, plain: &mut String, base: Style) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), base));
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn plain(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn one_line_per_source_line() {
        let src = "# T\n\n- a\n- [x] b\n```sh\nls\n```\n> q\n---\nplain";
        assert_eq!(to_text(src).lines.len(), src.lines().count());
    }

    #[test]
    fn checkboxes_and_bullets() {
        let text = to_text("- [ ] todo\n- [x] done\n- item\n  - nested");
        assert_eq!(plain(&text.lines[0]), "☐ todo");
        assert_eq!(plain(&text.lines[1]), "☑ done");
        assert_eq!(plain(&text.lines[2]), "• item");
        assert_eq!(plain(&text.lines[3]), "  • nested");
    }

    #[test]
    fn unbalanced_markers_stay_literal() {
        let text = to_text("a **b and `c");
        assert_eq!(plain(&text.lines[0]), "a **b and `c");
        let text = to_text("x **bold** `code` y");
        assert_eq!(plain(&text.lines[0]), "x bold code y");
    }

    #[test]
    fn splits_named_sections_out() {
        let src = "## Context\nc\n## Outcome\no\n```\n## Not a heading\n```\n## Next Steps\nn\n## Notes\nx\n";
        let (rest, picked) = split_sections(src, &["Outcome", "Next steps"]);
        assert_eq!(rest, "## Context\nc\n## Notes\nx\n");
        assert_eq!(
            picked,
            "## Outcome\no\n```\n## Not a heading\n```\n## Next Steps\nn\n"
        );
    }

    #[test]
    fn stats_skip_code_fences() {
        let src = "- [x] a\n- [ ] b\n```\n- [ ] not counted\n```\n* [X] c";
        assert_eq!(checklist_stats(src), (2, 3));
    }
}
