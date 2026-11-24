//! ANSI, both ways: styled text to escapes (the `.log` file) and back (the
//! TUI reading it).

use super::*;

/// Styled text as ANSI escapes (the `.log` file, `beagle scout session show`).
#[must_use]
pub fn to_ansi(text: &Text<'_>) -> String {
    let mut out = String::new();
    for line in &text.lines {
        for span in &line.spans {
            let codes = sgr(line.style.patch(span.style));
            if codes.is_empty() {
                out.push_str(&span.content);
            } else {
                let _ = write!(out, "\x1b[{codes}m{}\x1b[0m", span.content);
            }
        }
        out.push('\n');
    }
    out
}

/// Parses ANSI text back into styled lines: the TUI reads the pipeline's
/// `.log` with this, so displaying a session never re-renders it. Handles
/// the SGR codes [`to_ansi`] writes and ignores any others.
#[must_use]
pub fn from_ansi(source: &str) -> Text<'static> {
    let lines = source
        .lines()
        .map(|raw| {
            let mut spans = Vec::new();
            let mut style = Style::default();
            let mut rest = raw;
            while let Some(at) = rest.find("\x1b[") {
                if at > 0 {
                    spans.push(Span::styled(rest[..at].to_owned(), style));
                }
                let after = &rest[at + 2..];
                let Some(end) = after.find('m') else {
                    rest = after;
                    continue;
                };
                style = apply_sgr(style, &after[..end]);
                rest = &after[end + 1..];
            }
            if !rest.is_empty() {
                spans.push(Span::styled(rest.to_owned(), style));
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

pub(super) fn apply_sgr(mut style: Style, codes: &str) -> Style {
    let codes: Vec<u16> = codes.split(';').filter_map(|c| c.parse().ok()).collect();
    let mut i = 0;
    while i < codes.len() {
        match codes[i] {
            0 => style = Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            c @ 30..=37 => style = style.fg(basic(c - 30)),
            c @ 90..=97 => style = style.fg(bright(c - 90)),
            39 => style.fg = None,
            38 if codes.get(i + 1) == Some(&5) => {
                if let Some(n) = codes.get(i + 2).and_then(|n| u8::try_from(*n).ok()) {
                    style = style.fg(Color::Indexed(n));
                }
                i += 2;
            }
            38 if codes.get(i + 1) == Some(&2) => {
                let rgb: Vec<u8> = codes
                    .iter()
                    .skip(i + 2)
                    .take(3)
                    .filter_map(|c| u8::try_from(*c).ok())
                    .collect();
                if let [r, g, b] = rgb[..] {
                    style = style.fg(Color::Rgb(r, g, b));
                }
                i += 4;
            }
            _ => {}
        }
        i += 1;
    }
    style
}

pub(super) fn basic(n: u16) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        _ => Color::Gray,
    }
}

pub(super) fn bright(n: u16) -> Color {
    match n {
        0 => Color::DarkGray,
        1 => Color::LightRed,
        2 => Color::LightGreen,
        3 => Color::LightYellow,
        4 => Color::LightBlue,
        5 => Color::LightMagenta,
        6 => Color::LightCyan,
        _ => Color::White,
    }
}

pub(super) fn sgr(style: Style) -> String {
    let mut codes = Vec::new();
    for (flag, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::ITALIC, "3"),
        (Modifier::UNDERLINED, "4"),
    ] {
        if style.add_modifier.contains(flag) {
            codes.push(code.to_owned());
        }
    }
    if let Some(fg) = style.fg {
        let code = match fg {
            Color::Black => "30".to_owned(),
            Color::Red => "31".to_owned(),
            Color::Green => "32".to_owned(),
            Color::Yellow => "33".to_owned(),
            Color::Blue => "34".to_owned(),
            Color::Magenta => "35".to_owned(),
            Color::Cyan => "36".to_owned(),
            Color::Gray => "37".to_owned(),
            Color::DarkGray => "90".to_owned(),
            Color::LightRed => "91".to_owned(),
            Color::LightGreen => "92".to_owned(),
            Color::LightYellow => "93".to_owned(),
            Color::LightBlue => "94".to_owned(),
            Color::LightMagenta => "95".to_owned(),
            Color::LightCyan => "96".to_owned(),
            Color::White => "97".to_owned(),
            Color::Indexed(n) => format!("38;5;{n}"),
            Color::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
            Color::Reset => String::new(),
        };
        if !code.is_empty() {
            codes.push(code);
        }
    }
    codes.join(";")
}
