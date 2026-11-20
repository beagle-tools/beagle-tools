//! A small, panic-free markdown renderer targeting ratatui lines.
//!
//! Lifted from beagle. Deliberately minimal: headings, bullets, checklists,
//! code fences, blockquotes, rules, `**bold**` and `` `code` `` inline;
//! everything else is plain text. Each output line is tagged prose or
//! preformatted so the layout step ([`crate::wrap`]) word-wraps prose but
//! never breaks a code fence — ASCII diagrams inside fences keep their
//! alignment.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::wrap::DocLine;

/// Opt-in renderer behaviour. The default renders every tool the same way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Dim a line that is only an unfilled `<placeholder>` (scout's task
    /// templates), so it reads as "still to do".
    pub dim_placeholders: bool,
}

/// Renders markdown `source` into styled, wrap-tagged lines.
///
/// Never fails and never panics: unrecognized syntax falls through as plain
/// text, and an unclosed code fence simply styles the rest of the document as
/// code. Single-line HTML comments (`<!-- … -->`) are hidden.
#[must_use]
pub fn to_lines(source: &str) -> Vec<DocLine> {
    render(source, true, Options::default())
}

/// Renders markdown `source` into styled [`Text`] with exactly one output
/// line per source line, so a rendered line index is its source line index
/// (search and jump-to-line rely on this). Unlike [`to_lines`], HTML
/// comments are kept, as plain text.
#[must_use]
pub fn to_text(source: &str) -> Text<'static> {
    to_text_with(source, Options::default())
}

/// [`to_text`] with [`Options`].
#[must_use]
pub fn to_text_with(source: &str, options: Options) -> Text<'static> {
    Text::from(
        render(source, false, options)
            .into_iter()
            .map(|doc| doc.line)
            .collect::<Vec<_>>(),
    )
}

fn render(source: &str, hide_comments: bool, options: Options) -> Vec<DocLine> {
    let mut lines: Vec<DocLine> = Vec::with_capacity(source.lines().count());
    let mut in_code_fence = false;
    let mut lang = String::new();

    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            if in_code_fence {
                // Closing fence: a dim rule so the block's end is unmistakable.
                lines.push(DocLine::pre(fence_rule("╰", "")));
                in_code_fence = false;
                lang.clear();
            } else {
                // Opening fence: a dim rule labelled with the language, so
                // where the code starts (and in what language) is obvious.
                trimmed.trim_start_matches('`').trim().clone_into(&mut lang);
                lines.push(DocLine::pre(fence_rule("╭", &lang)));
                in_code_fence = true;
            }
            continue;
        }
        if in_code_fence {
            // A slim gutter groups the block; the code itself is syntax-
            // highlighted for the language on the opening fence.
            let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
            spans.extend(highlight::code(&lang, raw));
            lines.push(DocLine::pre(Line::from(spans)));
            continue;
        }
        if hide_comments && trimmed.starts_with("<!--") && trimmed.trim_end().ends_with("-->") {
            continue;
        }
        lines.push(DocLine::prose(render_line(raw, options)));
    }
    lines
}

/// A dim horizontal rule delimiting a code block: `╭─ python ────` at the
/// top, `╰──────` at the bottom.
fn fence_rule(corner: &str, lang: &str) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let label = if lang.is_empty() {
        format!("{corner}{}", "─".repeat(20))
    } else {
        format!("{corner}─ {lang} {}", "─".repeat(16))
    };
    Line::styled(label, dim)
}

fn render_line(raw: &str, options: Options) -> Line<'static> {
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
        if let Some((done, text)) = checkbox(rest) {
            // Checked items go green — on a verification checklist a tick
            // means "this held". Unchecked stay normal (not red/yellow):
            // pending is the default state, not a warning.
            let (glyph, glyph_style, text_style) = if done {
                (
                    "☑ ",
                    Style::default().fg(Color::Green),
                    Style::default().fg(Color::Green),
                )
            } else {
                ("☐ ", Style::default().fg(Color::Yellow), Style::default())
            };
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(glyph, glyph_style),
            ];
            spans.extend(inline_spans(text, text_style));
            return Line::from(spans);
        }
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled("• ", Style::default().fg(Color::Yellow)),
        ];
        spans.extend(inline_spans(rest, Style::default()));
        return Line::from(spans);
    }
    if options.dim_placeholders && trimmed.starts_with('<') && trimmed.trim_end().ends_with('>') {
        return Line::styled(
            raw.to_owned(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        );
    }
    Line::from(inline_spans(raw, Style::default()))
}

/// Counts markdown checkboxes in `source`: `(checked, total)`. Follows the
/// renderer's rules — code fences are skipped, `- ` and `* ` bullets at any
/// indent count, `[x]`/`[X]` are checked.
#[must_use]
pub fn checklist_stats(source: &str) -> (usize, usize) {
    let mut checked = 0;
    let mut total = 0;
    let mut in_code_fence = false;
    for raw in source.lines() {
        if raw.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        let trimmed = raw.trim_start();
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

/// The checkbox at the start of a bullet's content, if any: `[ ]`, `[x]`,
/// or `[X]`, either alone or followed by a space and the item text.
/// Anything else (`[x]done`, `[y] ...`) is an ordinary bullet.
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

fn heading(text: &str, color: Color, top_level: bool) -> Line<'static> {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    if top_level {
        Line::styled(text.to_owned(), style.add_modifier(Modifier::UNDERLINED))
    } else {
        Line::styled(text.to_owned(), style)
    }
}

/// Splits a line into spans, honoring `` `code` `` and `**bold**` markers.
/// Unbalanced markers are emitted literally — an author's stray `**` must
/// never eat the rest of the line.
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
        // Advance one char; `chars().next()` is Some because rest is non-empty.
        let ch = rest.chars().next().unwrap_or('\u{0}');
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

/// Syntax highlighting for fenced code (`markdown/highlight.rs`).
mod highlight;

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/markdown_text.rs"]
mod text_tests;
