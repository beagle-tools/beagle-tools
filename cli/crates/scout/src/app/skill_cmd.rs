//! `beagle scout skill install | status`.

use crate::skill::{Agent, Skill, SkillStatus};
use crate::Result;

use super::home;

/// Installs every skill for every agent (and, on macOS, builds the
/// notification app).
pub(super) fn install() -> Result<()> {
    let home = home()?;
    // Installing scout's agent integration is also when its notification
    // app (the dog icon) gets built. Best effort: notifications still work
    // without it, just with the Terminal icon.
    if cfg!(target_os = "macos") {
        match crate::notify_app::setup(&home) {
            Ok(app) => println!("notifications: built {}", app.display()),
            Err(e) => eprintln!("notifications: skipped Scout.app ({e})"),
        }
    }
    for agent in Agent::ALL {
        for skill in Skill::ALL {
            let path = agent.install(skill, &home)?;
            println!(
                "{}: installed /{} → {}",
                agent.name(),
                skill.name(),
                path.display()
            );
        }
    }
    Ok(())
}

/// Whether each installed skill matches the one built into this binary.
pub(super) fn status() -> Result<()> {
    let home = home()?;
    for agent in Agent::ALL {
        for skill in Skill::ALL {
            let state = match agent.status(skill, &home) {
                SkillStatus::Current => "current",
                SkillStatus::Stale => "stale — run `beagle scout skill install`",
                SkillStatus::Missing => "not installed — run `beagle scout skill install`",
            };
            println!(
                "{}: /{} {state} ({})",
                agent.name(),
                skill.name(),
                agent.path(skill, &home).display()
            );
        }
    }
    Ok(())
}
