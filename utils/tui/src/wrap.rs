//! Layout: turns tagged document lines into screen lines for a given width.
//!
//! Prose lines word-wrap (with a hanging indent under bullets and quotes so
//! wrapped text lines up with the first line's text). Preformatted lines —
//! code fences and diagrams — are never wrapped; the content pane pans
//! horizontally instead, so ASCII art keeps its alignment.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

/// One rendered document line, tagged with whether it may be wrapped.
#[derive(Debug, Clone, PartialEq)]
pub struct DocLine {
    /// The styled line.
    pub line: Line<'static>,
    /// `true` for code/diagram lines that must never be wrapped.
    pub pre: bool,
}

impl DocLine {
    /// A wrappable prose line.
    #[must_use]
    pub fn prose(line: Line<'static>) -> Self {
        Self { line, pre: false }
    }

    /// A preformatted (never wrapped) line.
    #[must_use]
    pub fn pre(line: Line<'static>) -> Self {
        Self { line, pre: true }
    }
}

/// Lays `lines` out for a pane `width` columns wide.
#[must_use]
pub fn layout(lines: &[DocLine], width: u16) -> Vec<Line<'static>> {
    layout_with_owners(lines, width).0
}

/// Like [`layout`], plus, for every screen row, the index of the document
/// line it came from (a wrapped prose line owns several rows).
#[must_use]
pub fn layout_with_owners(lines: &[DocLine], width: u16) -> (Vec<Line<'static>>, Vec<usize>) {
    let width = usize::from(width.max(8));
    let mut out = Vec::with_capacity(lines.len());
    let mut owners = Vec::with_capacity(lines.len());
    for (i, doc) in lines.iter().enumerate() {
        let before = out.len();
        if doc.pre || doc.line.width() <= width {
            out.push(doc.line.clone());
        } else {
            out.extend(wrap_line(&doc.line, width));
        }
        owners.resize(out.len().max(before), i);
    }
    (out, owners)
}

/// The text of a document line as copied by a yank: the rendered text,
/// minus the dim `│ ` gutter the markdown renderer puts on code lines.
#[must_use]
pub fn plain_text(line: &DocLine) -> String {
    let mut spans = line.line.spans.iter().peekable();
    if line.pre {
        if let Some(first) = spans.peek() {
            if first.content == "│ " {
                spans.next();
            }
        }
    }
    spans.map(|s| s.content.as_ref()).collect()
}

/// Markers after which a hanging indent starts.
const MARKERS: [&str; 5] = ["• ", "☐ ", "☑ ", "│ ", "- "];

fn wrap_line(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let cells: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let style = line.style.patch(span.style);
            span.content.chars().map(move |c| (c, style))
        })
        .collect();
    let indent = hanging_indent(&cells).min(width / 2);

    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut row: Vec<(char, Style)> = Vec::new();
    let mut row_width = 0;
    // Index in `row` just after the most recent space: the preferred cut.
    let mut last_break: Option<usize> = None;

    for (c, style) in cells {
        let w = c.width().unwrap_or(0);
        let fresh = !rows.is_empty() && row.len() == indent;
        if c == ' ' && (fresh || row_width + w > width) {
            // A space that would overflow is the break itself; spaces at the
            // start of a wrapped row are dropped.
            if !fresh {
                rows.push(trim_end(std::mem::replace(
                    &mut row,
                    vec![(' ', Style::default()); indent],
                )));
                row_width = indent;
                last_break = None;
            }
            continue;
        }
        if row_width + w > width && !row.is_empty() {
            let cut = last_break.filter(|&i| i > indent).unwrap_or(row.len());
            let carry: Vec<(char, Style)> = row.split_off(cut);
            rows.push(trim_end(row));
            row = vec![(' ', Style::default()); indent];
            row.extend(carry.into_iter().skip_while(|(c, _)| *c == ' '));
            row_width = row.iter().map(|(c, _)| c.width().unwrap_or(0)).sum();
            last_break = None;
        }
        row.push((c, style));
        row_width += w;
        if c == ' ' {
            last_break = Some(row.len());
        }
    }
    rows.push(row);
    rows.into_iter().map(to_line).collect()
}

fn hanging_indent(cells: &[(char, Style)]) -> usize {
    let lead = cells.iter().take_while(|(c, _)| *c == ' ').count();
    let rest: String = cells.iter().skip(lead).take(2).map(|(c, _)| *c).collect();
    if MARKERS.contains(&rest.as_str()) {
        return lead + 2;
    }
    // Numbered lists: "12. text" hangs under "text".
    let digits = cells
        .iter()
        .skip(lead)
        .take_while(|(c, _)| c.is_ascii_digit())
        .count();
    let after: String = cells
        .iter()
        .skip(lead + digits)
        .take(2)
        .map(|(c, _)| *c)
        .collect();
    if (1..=3).contains(&digits) && after == ". " {
        lead + digits + 2
    } else {
        lead
    }
}

fn trim_end(mut row: Vec<(char, Style)>) -> Vec<(char, Style)> {
    while row.last().is_some_and(|(c, _)| *c == ' ') {
        row.pop();
    }
    row
}

fn to_line(row: Vec<(char, Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut current: Option<Style> = None;
    for (c, style) in row {
        if current.is_some_and(|s| s != style) {
            spans.push(Span::styled(
                std::mem::take(&mut text),
                current.unwrap_or_default(),
            ));
        }
        current = Some(style);
        text.push(c);
    }
    if !text.is_empty() {
        spans.push(Span::styled(text, current.unwrap_or_default()));
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "tests/wrap.rs"]
mod tests;
