//! A secret manager's output: a JSON object of strings, or dotenv
//! (`KEY=VALUE` lines, with quoted and multi-line values).
//!
//! Errors never quote a value: they name the line or the key only.

use std::collections::BTreeMap;

/// The names and values in `text` (JSON when it starts with `{`, else
/// dotenv).
///
/// # Errors
/// What is malformed, by line or key; never a value.
pub fn parse(text: &str) -> Result<BTreeMap<String, String>, String> {
    if text.trim_start().starts_with('{') {
        json(text)
    } else {
        dotenv(text)
    }
}

fn json(text: &str) -> Result<BTreeMap<String, String>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "not a JSON object (line {}): {}",
            e.line(),
            e.classify_msg()
        )
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| "not a JSON object".to_owned())?;
    Ok(object
        .iter()
        .filter_map(|(k, v)| match v {
            serde_json::Value::String(s) => Some((k.clone(), s.clone())),
            serde_json::Value::Number(n) => Some((k.clone(), n.to_string())),
            serde_json::Value::Bool(b) => Some((k.clone(), b.to_string())),
            _ => None,
        })
        .collect())
}

/// `serde_json`'s message without the parts that could echo input.
trait Classify {
    fn classify_msg(&self) -> &'static str;
}

impl Classify for serde_json::Error {
    fn classify_msg(&self) -> &'static str {
        match self.classify() {
            serde_json::error::Category::Io => "read error",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "unexpected data",
            serde_json::error::Category::Eof => "unexpected end",
        }
    }
}

fn dotenv(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut lines = text.lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, rest) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected KEY=VALUE", n + 1))?;
        let key = key.trim();
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(format!("line {}: not a variable name", n + 1));
        }
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let mut raw = rest[1..].to_owned();
                loop {
                    if let Some(end) = closing(&raw, q) {
                        raw.truncate(end);
                        break;
                    }
                    let (_, next) = lines
                        .next()
                        .ok_or_else(|| format!("{key}: unterminated quote"))?;
                    raw.push('\n');
                    raw.push_str(next);
                }
                if q == '"' {
                    unescape(&raw)
                } else {
                    raw
                }
            }
            _ => rest.trim_end().to_owned(),
        };
        out.insert(key.to_owned(), value);
    }
    Ok(out)
}

/// The byte index of the closing `q` in `raw` (`\"` doesn't close a
/// double-quoted value).
fn closing(raw: &str, q: char) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in raw.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' && q == '"' {
            escaped = true;
        } else if c == q {
            return Some(i);
        }
    }
    None
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}
