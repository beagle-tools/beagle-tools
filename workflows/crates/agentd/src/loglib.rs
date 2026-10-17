//! The standard logger handed to script stages.
//!
//! A script's output has no type — every line used to be stored as an
//! untyped `raw` event, so the log views could only guess what mattered.
//! `log.sh` gives scripts a one-line way to say: each call prints
//! `LEVEL source: message`, the exact format `agent_core::logtext` reads.
//!
//! The daemon writes it on startup into the state directory, replacing
//! whatever is there so an updated agentd ships an updated logger, and
//! every run gets its path as `$AGENTD_LOG_SH`.

use std::path::PathBuf;

use agent_core::paths;
use anyhow::{Context, Result};

const LOG_SH: &str = include_str!("../assets/log.sh");

pub(crate) fn path() -> Result<PathBuf> {
    Ok(paths::state_dir()
        .context("resolve state dir")?
        .join("lib")
        .join("log.sh"))
}

pub(crate) fn install() -> Result<PathBuf> {
    let path = path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    std::fs::write(&path, LOG_SH).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use agent_core::logtext::{parse_script_line, Level};

    use super::LOG_SH;

    /// Run the real logger in a real shell and read its output back with
    /// the real parser. The two halves are in different languages; this
    /// is the only thing that keeps them agreeing about the format.
    fn logged(call: &str) -> String {
        let dir = std::env::temp_dir().join(format!("agentd-logsh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("log.sh");
        std::fs::write(&script, LOG_SH).unwrap();
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(". '{}'; {call}", script.display()))
            .env("AGENTD_AGENT", "pr-watch")
            .env_remove("AGENTD_LOG_SOURCE")
            .output()
            .unwrap();
        let mut text = String::from_utf8(out.stdout).unwrap();
        text.push_str(&String::from_utf8(out.stderr).unwrap());
        text.trim_end().to_owned()
    }

    #[test]
    fn every_level_the_logger_prints_is_read_back_as_that_level() {
        for (call, level) in [
            ("log_debug hello", Level::Debug),
            ("log_info hello", Level::Info),
            ("log_step hello", Level::Step),
            ("log_ok hello", Level::Ok),
            ("log_warn hello", Level::Warn),
            ("log_error hello", Level::Error),
        ] {
            let line = parse_script_line(&logged(call));
            assert_eq!(line.level, level, "{call}");
            assert_eq!(line.source.as_deref(), Some("pr-watch"), "{call}");
            assert_eq!(line.message, "hello", "{call}");
        }
    }

    #[test]
    fn a_message_with_colons_and_spaces_survives_whole() {
        let line = parse_script_line(&logged("log_info 'wrote: 6 files to /tmp/x'"));
        assert_eq!(line.message, "wrote: 6 files to /tmp/x");
    }

    #[test]
    fn the_source_can_be_narrowed() {
        let line = parse_script_line(&logged("AGENTD_LOG_SOURCE=gather log_info x"));
        assert_eq!(line.source.as_deref(), Some("gather"));
    }
}
