//! Markdown for scout's panes: rendered by the shared
//! `beagle_tui::markdown` (the same look as every beagle TUI) with unfilled
//! `<placeholder>` lines dimmed, plus the section splitter behind the Task /
//! Outcome tabs. One output line per source line, so line counts match the
//! file.

use beagle_tui::markdown::{self, Options};
use ratatui::text::Text;

/// Renders Markdown `source` with the shared renderer, dimming unfilled
/// `<placeholder>` lines so they read as "still to do".
#[must_use]
pub fn to_text(source: &str) -> Text<'static> {
    markdown::to_text_with(
        source,
        Options {
            dim_placeholders: true,
        },
    )
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

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use ratatui::text::Line;

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
    fn placeholders_stay_dimmed() {
        let text = to_text("<Filled in when the task closes.>\ndone");
        assert_eq!(text.lines[0].style.fg, Some(Color::DarkGray));
        assert_ne!(text.lines[1].style.fg, Some(Color::DarkGray));
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
        assert_eq!(beagle_tui::markdown::checklist_stats(src), (2, 3));
    }
}
