//! The skills, compiled into the binary so the instructions agents follow
//! always match the `beagle scout` they drive. `beagle scout skill install`
//! copies them to Claude Code and Codex.
//!
//! - `/scout` — plan a project and keep its status true while working.
//! - `/scout-build` — work an existing project task by task.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// A skill shipped with scout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skill {
    /// `/scout`: `skills/scout/SKILL.md` in this crate.
    Scout,
    /// `/scout-build`: `skills/scout-build/SKILL.md` in this crate.
    Build,
}

/// An agent CLI the skills can be installed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code: `~/.claude/skills/<name>/SKILL.md`.
    ClaudeCode,
    /// Codex: `~/.codex/prompts/<name>.md`.
    Codex,
}

/// Whether an installed copy matches this binary's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStatus {
    /// Not installed.
    Missing,
    /// Installed, but differs from this binary's copy.
    Stale,
    /// Installed and identical.
    Current,
}

impl Skill {
    /// Every shipped skill.
    pub const ALL: [Self; 2] = [Self::Scout, Self::Build];

    /// The skill's name, as typed after `/`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Scout => "scout",
            Self::Build => "scout-build",
        }
    }

    /// The skill's text, as it was at build time.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Scout => include_str!("../skills/scout/SKILL.md"),
            Self::Build => include_str!("../skills/scout-build/SKILL.md"),
        }
    }
}

impl Agent {
    /// Every supported agent.
    pub const ALL: [Self; 2] = [Self::ClaudeCode, Self::Codex];

    /// Display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// Where `skill` lives for this agent under `home`.
    #[must_use]
    pub fn path(self, skill: Skill, home: &Path) -> PathBuf {
        match self {
            Self::ClaudeCode => home
                .join(".claude/skills")
                .join(skill.name())
                .join("SKILL.md"),
            Self::Codex => home
                .join(".codex/prompts")
                .join(format!("{}.md", skill.name())),
        }
    }

    /// Compares the installed copy of `skill` with this binary's.
    #[must_use]
    pub fn status(self, skill: Skill, home: &Path) -> SkillStatus {
        match fs::read_to_string(self.path(skill, home)) {
            Ok(installed) if installed == skill.text() => SkillStatus::Current,
            Ok(_) => SkillStatus::Stale,
            Err(_) => SkillStatus::Missing,
        }
    }

    /// Writes `skill` to this agent's skill path, creating directories.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory or file cannot be written.
    pub fn install(self, skill: Skill, home: &Path) -> Result<PathBuf> {
        let path = self.path(skill, home);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(Error::io(dir))?;
        }
        fs::write(&path, skill.text()).map_err(Error::io(&path))?;
        Ok(path)
    }
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap/expect/panic is the clearest way.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn install_then_current_then_stale() {
        let home = tempfile::tempdir().unwrap();
        for agent in Agent::ALL {
            for skill in Skill::ALL {
                assert_eq!(agent.status(skill, home.path()), SkillStatus::Missing);
                let path = agent.install(skill, home.path()).unwrap();
                assert_eq!(agent.status(skill, home.path()), SkillStatus::Current);
                fs::write(path, "old").unwrap();
                assert_eq!(agent.status(skill, home.path()), SkillStatus::Stale);
            }
        }
        assert!(home
            .path()
            .join(".claude/skills/scout-build/SKILL.md")
            .is_file());
        assert!(home.path().join(".codex/prompts/scout-build.md").is_file());
    }

    #[test]
    fn frontmatter_names_match() {
        for skill in Skill::ALL {
            let header = format!("---\nname: {}\n", skill.name());
            assert!(skill.text().starts_with(&header), "{}", skill.name());
        }
    }
}
