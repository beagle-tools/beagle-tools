//! Setup and self-management: `beagle config`, `beagle update` / `version
//! list`, `beagle agent <action>`, and `beagle skill <action>`.

use std::env;
use std::fs;
use std::io::IsTerminal as _;

use beagle::skill::{self, Agent, Skill, SkillStatus};
use beagle::{config, update, Error};

use crate::cli::{AgentAction, SkillAction};

/// `beagle config`: create the file from the template if absent, open it in
/// the user's editor, and validate it when the editor closes.
pub(crate) fn run_config() -> Result<(), Error> {
    let path = config::path();
    if !path.exists() {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        fs::write(&path, config::TEMPLATE).map_err(|e| Error::io(&path, e))?;
        println!("created {}", path.display());
    }

    // Choose the editor from the pre-edit config when it parses; a broken
    // config falls back to $VISUAL/$EDITOR/vim — that broken state is
    // exactly when the editor most needs to open.
    let pre_edit = config::load(&path).ok().flatten();
    let editor = config::editor(pre_edit.as_ref());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vim");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .map_err(|e| Error::Tool {
            tool: "editor",
            message: format!("could not launch `{editor}`: {e}"),
        })?;
    if !status.success() {
        return Err(Error::Tool {
            tool: "editor",
            message: format!("`{editor}` exited with {status}; config not validated"),
        });
    }

    match config::load(&path) {
        Ok(Some(config)) => {
            println!("✓ config OK ({})", path.display());
            if let Some(root) = &config.root {
                println!("  root   = {}", root.display());
            }
            if let Some(editor) = &config.editor {
                println!("  editor = {editor}");
            }
            Ok(())
        }
        Ok(None) => {
            println!("config file removed; beagle will use defaults");
            Ok(())
        }
        Err(e) => {
            eprintln!("your edits are saved, but the config does not validate;");
            eprintln!("run `beagle config` again to fix it:");
            Err(e)
        }
    }
}

/// `beagle version list`: interactive picker on a terminal (enter installs
/// the selected version), plain listing when piped.
pub(crate) fn run_version_list() -> Result<(), Error> {
    let releases = update::fetch_releases()?;
    if releases.is_empty() {
        println!("no releases published yet — {}/releases", update::REPO_URL);
        return Ok(());
    }
    let current = update::Version::current();
    if std::io::stdout().is_terminal() {
        match update::pick_version(&releases, current)? {
            Some(version) => install_version(version),
            None => Ok(()),
        }
    } else {
        for (i, release) in releases.iter().enumerate() {
            let mut markers = String::new();
            if i == 0 {
                markers.push_str("  latest");
            }
            if release.version == current {
                markers.push_str("  current");
            }
            println!("{}{markers}", release.version.tag());
        }
        Ok(())
    }
}

/// The newest published release, for a bare `beagle update`.
pub(crate) fn latest_release() -> Result<update::Version, Error> {
    update::fetch_releases()?
        .first()
        .map(|release| release.version)
        .ok_or_else(|| Error::Tool {
            tool: "update",
            message: format!("no releases published yet — {}/releases", update::REPO_URL),
        })
}

/// Installs `version` over the running binary — the same path for upgrades
/// and downgrades. A no-op (with a message) when already on that version.
pub(crate) fn install_version(version: update::Version) -> Result<(), Error> {
    let current = update::Version::current();
    if version == current {
        println!("already on beagle {current}; nothing to do");
        offer_skill_install();
        return Ok(());
    }
    let exe = env::current_exe().map_err(|e| Error::io("beagle", e))?;
    let verb = if version > current {
        "updating"
    } else {
        "downgrading"
    };
    println!("{verb} beagle {current} → {version} …");
    update::update_to(version, &exe)?;
    println!("✓ beagle {version} installed at {}", exe.display());
    println!("  restart any running beagle TUIs to pick it up");
    offer_skill_install();
    Ok(())
}

/// `beagle agent <action>`: install/uninstall/start/stop the daemon service
/// or print its status.
pub(crate) fn run_agent_service(action: AgentAction) -> Result<(), Error> {
    let outcome = match action {
        AgentAction::Install => beagle::agentd::install(),
        AgentAction::Uninstall => beagle::agentd::uninstall(),
        AgentAction::Start => beagle::agentd::start(),
        AgentAction::Stop => beagle::agentd::stop(),
        AgentAction::Status => beagle::agentd::status(),
    };
    match outcome {
        Ok(message) => {
            println!("{message}");
            Ok(())
        }
        Err(message) => Err(Error::Tool {
            tool: "beagle agent",
            message,
        }),
    }
}

/// `beagle skill`: report where each agent's copy of the skill stands, or
/// write the bundled skill for each.
pub(crate) fn run_skill(action: SkillAction) -> Result<(), Error> {
    let home = skill::home()?;
    match action {
        SkillAction::Status => {
            for agent in Agent::ALL {
                for skill in Skill::ALL {
                    let note = if agent.is_present(&home) {
                        match skill::status(skill, agent, &home) {
                            SkillStatus::Current => "up to date",
                            SkillStatus::Outdated => {
                                "outdated — `beagle skill install` refreshes it"
                            }
                            SkillStatus::Missing => {
                                "not installed — `beagle skill install` adds it"
                            }
                        }
                    } else {
                        "agent not detected here"
                    };
                    println!(
                        "{:<12} /{:<14} {note}\n             {}",
                        agent.name(),
                        skill.slug(),
                        agent.skill_path(skill, &home).display()
                    );
                }
            }
        }
        SkillAction::Install => {
            // Explicit intent: install for detected agents, or for all when
            // none are detected (so a fresh setup still gets the skills).
            let detected: Vec<Agent> = Agent::ALL
                .into_iter()
                .filter(|a| a.is_present(&home))
                .collect();
            let targets = if detected.is_empty() {
                Agent::ALL.to_vec()
            } else {
                detected
            };
            for agent in targets {
                for skill in Skill::ALL {
                    let path = skill::install(skill, agent, &home)?;
                    println!("✓ {} /{} → {}", agent.name(), skill.slug(), path.display());
                }
            }
        }
    }
    Ok(())
}

/// After an install, offer to install or refresh the beagle skills for
/// every detected agent whose copy of any skill is missing or stale.
/// Interactive only — piped output just prints a hint — and best-effort: a
/// decline or a write failure never fails the update.
fn offer_skill_install() {
    let Ok(home) = skill::home() else { return };
    // (skill, agent) pairs that are detected but not current.
    let pending: Vec<(Skill, Agent)> = Agent::ALL
        .into_iter()
        .filter(|a| a.is_present(&home))
        .flat_map(|agent| Skill::ALL.into_iter().map(move |skill| (skill, agent)))
        .filter(|(skill, agent)| skill::status(*skill, *agent, &home) != SkillStatus::Current)
        .collect();
    if pending.is_empty() {
        return;
    }
    let mut names: Vec<&str> = pending.iter().map(|(_, a)| a.name()).collect();
    names.dedup();
    let list = names.join(" and ");

    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        println!("\nbundled beagle skills are newer than what's installed for {list}.");
        println!("run `beagle skill install` to update them.");
        return;
    }

    print!("\ninstall/update the beagle skills for {list}? [y/N] ");
    if std::io::Write::flush(&mut std::io::stdout()).is_err() {
        return;
    }
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return;
    }
    if !matches!(answer.trim(), "y" | "Y" | "yes") {
        println!("skipped — run `beagle skill install` anytime.");
        return;
    }
    for (skill, agent) in pending {
        match skill::install(skill, agent, &home) {
            Ok(path) => println!("✓ {} /{} → {}", agent.name(), skill.slug(), path.display()),
            Err(e) => eprintln!(
                "could not install /{} for {}: {e}",
                skill.slug(),
                agent.name()
            ),
        }
    }
}
