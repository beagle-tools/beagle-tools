//! Building blocks for the documents apps put in a [`crate::DocView`]: dim
//! text, blank lines, section rules, link rows and diagram blocks.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ansi;
use crate::wrap::DocLine;

/// The dim style for secondary text.
#[must_use]
pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// An empty prose line.
#[must_use]
pub fn blank() -> DocLine {
    DocLine::prose(Line::default())
}

/// `── label ────…`: a dim section rule.
#[must_use]
pub fn rule(label: &str) -> DocLine {
    DocLine::pre(Line::styled(
        format!("── {label} {}", "─".repeat(40)),
        dim(),
    ))
}

/// A link row: the label padded to `label_width`, the URL underlined, then
/// the key that opens it: `pr     https://…  [o]`.
#[must_use]
pub fn link(label: &str, label_width: usize, key: &str, url: &str) -> DocLine {
    DocLine::pre(Line::from(vec![
        Span::styled(format!("{label:<label_width$}"), dim()),
        Span::styled(
            url.to_owned(),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
        ),
        Span::styled(format!("  [{key}]"), dim()),
    ]))
}

/// A blank line, a `diagram · <name>` rule, then the diagram's ANSI-styled
/// body as preformatted lines (never wrapped).
pub fn push_diagram(out: &mut Vec<DocLine>, name: &str, body: &str) {
    out.push(blank());
    out.push(rule(&format!("diagram · {name}")));
    out.extend(ansi::to_text(body).lines.into_iter().map(DocLine::pre));
}

/// A blank line, then each warning as a red `! …` line.
pub fn push_warnings(out: &mut Vec<DocLine>, heading: Option<&str>, warnings: &[String]) {
    if warnings.is_empty() {
        return;
    }
    out.push(blank());
    if let Some(heading) = heading {
        out.push(rule(heading));
    }
    for w in warnings {
        out.push(DocLine::prose(Line::styled(
            format!("! {w}"),
            Style::default().fg(Color::Red),
        )));
    }
}
