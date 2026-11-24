//! Positionals and `--flag value` pairs.

/// A subcommand's arguments, after `--root` is lifted out.
pub(super) struct Args {
    pub(super) positionals: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    /// Splits `raw` against the value-taking flags it accepts. `--` ends
    /// flag parsing, so a log message may start with a dash.
    pub(super) fn parse(raw: &[String], accepted: &[&str]) -> Result<Self, String> {
        let mut positionals = Vec::new();
        let mut flags = Vec::new();
        let mut it = raw.iter();
        while let Some(arg) = it.next() {
            if arg == "--" {
                positionals.extend(it.by_ref().cloned());
                break;
            }
            if let Some(name) = arg.strip_prefix("--") {
                let (name, inline) = match name.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_owned())),
                    None => (name, None),
                };
                if !accepted.contains(&name) {
                    return Err(format!("unknown flag --{name}"));
                }
                let value = match inline {
                    Some(v) => v,
                    None => it
                        .next()
                        .cloned()
                        .ok_or(format!("--{name} needs a value"))?,
                };
                flags.push((name.to_owned(), value));
            } else {
                positionals.push(arg.clone());
            }
        }
        Ok(Self { positionals, flags })
    }

    /// Every value given for `--name`, in order.
    pub(super) fn all(&self, name: &str) -> Vec<String> {
        self.flags
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// The last value given for `--name`.
    pub(super) fn one(&self, name: &str) -> Option<String> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    /// Exactly `N` positionals, or a usage error naming them.
    pub(super) fn exact<const N: usize>(&self, names: [&str; N]) -> Result<[String; N], String> {
        <[String; N]>::try_from(self.positionals.clone())
            .map_err(|_| format!("expected {}", names.map(|n| format!("<{n}>")).join(" ")))
    }
}
