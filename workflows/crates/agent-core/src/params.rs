//! Declared job parameters: the `params:` block of an agent manifest, and
//! the validation that turns `-p k=v,k2=v2` into the run's param map.
//!
//! An agent that declares no params keeps the old free-form behaviour (any
//! `-p` is passed through). Once it declares them, the set is closed: every
//! `required` one must be supplied and unknown names are rejected, so a typo
//! fails loudly at submit time instead of confusing the agent at run time.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// One declared parameter of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Key as passed on the command line (`-p name=value`).
    pub name: String,
    /// Required params must be supplied; the run is refused without them.
    #[serde(default)]
    pub required: bool,
    /// Used when the param is omitted. Implies the param is not required.
    #[serde(default)]
    pub default: Option<String>,
    /// Shown when a run is refused, so the caller learns what to pass.
    #[serde(default)]
    pub description: Option<String>,
}

/// Why a set of supplied params was refused. Rendered for a human (and for
/// an agent reading stderr) by [`std::fmt::Display`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamError {
    /// A `-p` entry that is not `KEY=VALUE`.
    Malformed(String),
    /// Required params with no value supplied.
    Missing {
        /// The job (`project/name`) being run.
        job: String,
        /// The required params that had no value.
        missing: Vec<ParamSpec>,
        /// Everything the job declares, for the error message.
        declared: Vec<ParamSpec>,
    },
    /// Supplied names the job does not declare.
    Unknown {
        /// The job (`project/name`) being run.
        job: String,
        /// The supplied names the job does not declare.
        unknown: Vec<String>,
        /// Everything the job declares, for the error message.
        declared: Vec<ParamSpec>,
    },
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(entry) => {
                write!(f, "parameter {entry:?} is not KEY=VALUE")
            }
            Self::Missing {
                job,
                missing,
                declared,
            } => {
                writeln!(f, "job {job:?} is missing required parameter(s):\n")?;
                for spec in missing {
                    writeln!(f, "{}", describe(spec))?;
                }
                write!(f, "\n{}", usage(job, declared))
            }
            Self::Unknown {
                job,
                unknown,
                declared,
            } => {
                writeln!(
                    f,
                    "job {job:?} does not declare parameter(s): {}\n",
                    unknown.join(", ")
                )?;
                write!(f, "{}", usage(job, declared))
            }
        }
    }
}

impl std::error::Error for ParamError {}

/// `  name  (required)  description` — one line per declared param.
fn describe(spec: &ParamSpec) -> String {
    let mut line = format!("  {:<12}", spec.name);
    if spec.required {
        line.push_str("(required) ");
    } else if let Some(default) = &spec.default {
        let _ = write!(line, "(default: {default}) ");
    } else {
        line.push_str("(optional) ");
    }
    if let Some(description) = &spec.description {
        line.push_str(description);
    }
    line.trim_end().to_owned()
}

/// The `-p` invocation that would satisfy the job, plus every declared param.
fn usage(job: &str, declared: &[ParamSpec]) -> String {
    let required: Vec<String> = declared
        .iter()
        .filter(|s| s.required)
        .map(|s| format!("{}=<value>", s.name))
        .collect();
    let mut out = String::from("usage: agentctl -j ");
    out.push_str(job);
    if !required.is_empty() {
        out.push_str(" -p ");
        out.push_str(&required.join(","));
    }
    if declared.len() > required.len() {
        out.push_str("\n\nall parameters:\n");
        for spec in declared {
            out.push_str(&describe(spec));
            out.push('\n');
        }
        out.pop();
    }
    out
}

/// Split `-p` entries into a map. Each entry may itself be a comma-separated
/// list (`-p a=1,b=2`), and `-p` may be repeated. The value keeps everything
/// after the first `=`, so `url=https://x/y?a=b` survives intact; a value
/// containing a comma needs its own `-p`.
pub fn parse_pairs(entries: &[String]) -> Result<BTreeMap<String, String>, ParamError> {
    let mut out = BTreeMap::new();
    for entry in entries {
        for pair in entry.split(',').filter(|p| !p.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| ParamError::Malformed(pair.to_owned()))?;
            if key.is_empty() {
                return Err(ParamError::Malformed(pair.to_owned()));
            }
            out.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(out)
}

/// Check `supplied` against what `job` declares and fill in defaults.
///
/// With no declared params the supplied map passes through untouched — that
/// is the pre-`params:` behaviour, so existing manifests keep working.
pub fn resolve(
    job: &str,
    declared: &[ParamSpec],
    supplied: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ParamError> {
    if declared.is_empty() {
        return Ok(supplied);
    }
    let unknown: Vec<String> = supplied
        .keys()
        .filter(|k| !declared.iter().any(|s| &&s.name == k))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(ParamError::Unknown {
            job: job.to_owned(),
            unknown,
            declared: declared.to_vec(),
        });
    }

    let mut out = supplied;
    let mut missing = Vec::new();
    for spec in declared {
        if out.contains_key(&spec.name) {
            continue;
        }
        if let Some(default) = &spec.default {
            out.insert(spec.name.clone(), default.clone());
        } else if spec.required {
            missing.push(spec.clone());
        }
    }
    if !missing.is_empty() {
        return Err(ParamError::Missing {
            job: job.to_owned(),
            missing,
            declared: declared.to_vec(),
        });
    }
    Ok(out)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn spec(name: &str, required: bool, default: Option<&str>) -> ParamSpec {
        ParamSpec {
            name: name.to_owned(),
            required,
            default: default.map(str::to_owned),
            description: Some(format!("the {name}")),
        }
    }

    #[test]
    fn pairs_split_on_commas_and_the_first_equals() {
        let got = parse_pairs(&["url=https://x/y?a=b,model=sonnet".to_owned()]).unwrap();
        assert_eq!(got["url"], "https://x/y?a=b");
        assert_eq!(got["model"], "sonnet");
    }

    #[test]
    fn repeated_flags_merge() {
        let got = parse_pairs(&["a=1".to_owned(), "b=2,c=3".to_owned()]).unwrap();
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn malformed_pairs_are_refused() {
        assert_eq!(
            parse_pairs(&["nope".to_owned()]).unwrap_err(),
            ParamError::Malformed("nope".to_owned())
        );
        assert!(parse_pairs(&["=v".to_owned()]).is_err());
    }

    #[test]
    fn undeclared_jobs_accept_anything() {
        let supplied = parse_pairs(&["whatever=1".to_owned()]).unwrap();
        assert_eq!(resolve("j", &[], supplied.clone()).unwrap(), supplied);
    }

    #[test]
    fn missing_required_params_explain_themselves() {
        let declared = vec![spec("url", true, None), spec("depth", false, Some("2"))];
        let err = resolve("linear", &declared, BTreeMap::new()).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("missing required parameter"));
        assert!(text.contains("url"));
        assert!(text.contains("the url"));
        // The fix is spelled out, including the optional params.
        assert!(text.contains("agentctl -j linear -p url=<value>"));
        assert!(text.contains("depth"));
    }

    #[test]
    fn defaults_fill_in_and_required_passes() {
        let declared = vec![spec("url", true, None), spec("depth", false, Some("2"))];
        let supplied = parse_pairs(&["url=u".to_owned()]).unwrap();
        let got = resolve("linear", &declared, supplied).unwrap();
        assert_eq!(got["url"], "u");
        assert_eq!(got["depth"], "2");
    }

    #[test]
    fn unknown_params_are_refused_with_the_declared_set() {
        let declared = vec![spec("url", true, None)];
        let supplied = parse_pairs(&["url=u".to_owned(), "typo=1".to_owned()]).unwrap();
        let err = resolve("linear", &declared, supplied).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("does not declare"));
        assert!(text.contains("typo"));
    }
}
