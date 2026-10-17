//! Tests for skill installation (`skill`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn bundled_skills_are_embedded_and_look_like_themselves() {
    assert!(
        Skill::Beagle.markdown().contains("name: beagle"),
        "the beagle skill is embedded"
    );
    assert!(
        Skill::Beagle.markdown().contains("## 1. Scaffold"),
        "the full beagle body is embedded"
    );
    assert!(
        Skill::Review.markdown().contains("name: beagle-review"),
        "the review skill is embedded"
    );
    assert!(
        Skill::Review.markdown().contains("beagle context <id>"),
        "the review skill drives the context command"
    );
    assert_eq!(Skill::Review.slug(), "beagle-review");
    // SKILL_MD is still the beagle skill for back-compat callers.
    assert_eq!(SKILL_MD, Skill::Beagle.markdown());
}

#[test]
fn body_without_frontmatter_strips_the_yaml_block() {
    let md = "---\nname: beagle\ndescription: x\n---\n\n# Heading\n\nbody";
    assert_eq!(body_without_frontmatter(md), "# Heading\n\nbody");
    // No frontmatter → unchanged.
    assert_eq!(body_without_frontmatter("# Just a doc\n"), "# Just a doc\n");
    // The real skill has frontmatter, so stripping shortens it.
    assert!(body_without_frontmatter(SKILL_MD).len() < SKILL_MD.len());
    assert!(!body_without_frontmatter(SKILL_MD).starts_with("---"));
}

#[test]
fn agents_write_to_the_expected_paths() {
    let home = Path::new("/home/dev");
    assert!(Agent::Claude
        .skill_path(Skill::Beagle, home)
        .ends_with(".claude/skills/beagle/SKILL.md"));
    assert!(Agent::Codex
        .skill_path(Skill::Beagle, home)
        .ends_with(".codex/prompts/beagle.md"));
    // The review skill gets its own slug-derived paths.
    assert!(Agent::Claude
        .skill_path(Skill::Review, home)
        .ends_with(".claude/skills/beagle-review/SKILL.md"));
    assert!(Agent::Codex
        .skill_path(Skill::Review, home)
        .ends_with(".codex/prompts/beagle-review.md"));
    // Claude keeps frontmatter; Codex gets the body only.
    assert!(Agent::Claude.content(Skill::Beagle).starts_with("---"));
    assert!(!Agent::Codex.content(Skill::Beagle).starts_with("---"));
}

#[test]
fn status_is_missing_then_current_then_outdated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();

    assert_eq!(
        status(Skill::Beagle, Agent::Claude, home),
        SkillStatus::Missing
    );

    let path = install(Skill::Beagle, Agent::Claude, home).expect("install");
    assert!(path.exists());
    assert_eq!(
        status(Skill::Beagle, Agent::Claude, home),
        SkillStatus::Current
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        Agent::Claude.content(Skill::Beagle)
    );

    std::fs::write(&path, "hand-edited\n").expect("edit");
    assert_eq!(
        status(Skill::Beagle, Agent::Claude, home),
        SkillStatus::Outdated
    );
}

#[test]
fn install_creates_missing_directories_for_every_skill_and_agent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    for agent in Agent::ALL {
        for skill in Skill::ALL {
            let path = install(skill, agent, home).expect("install");
            assert!(
                path.exists(),
                "{} /{} installed",
                agent.name(),
                skill.slug()
            );
            assert_eq!(
                std::fs::read_to_string(&path).expect("read"),
                agent.content(skill)
            );
        }
    }
}

#[test]
fn is_present_follows_the_config_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    assert!(!Agent::Claude.is_present(home));
    std::fs::create_dir_all(home.join(".claude")).expect("mkdir");
    assert!(Agent::Claude.is_present(home));
    assert!(!Agent::Codex.is_present(home));
}

#[cfg(unix)]
#[test]
fn install_refuses_to_write_through_a_symlinked_skill_dir() {
    let home = tempfile::tempdir().expect("home");
    let checkout = tempfile::tempdir().expect("checkout");
    // A repo checkout's `skills/beagle/SKILL.md`, linked into ~/.claude/skills.
    let repo_skill = checkout.path().join("skills").join("beagle");
    std::fs::create_dir_all(&repo_skill).expect("repo skill dir");
    std::fs::write(repo_skill.join("SKILL.md"), "repo copy").expect("repo skill");
    let skills_dir = home.path().join(".claude").join("skills");
    std::fs::create_dir_all(&skills_dir).expect("skills dir");
    std::os::unix::fs::symlink(&repo_skill, skills_dir.join("beagle")).expect("symlink");

    assert_eq!(
        status(Skill::Beagle, Agent::Claude, home.path()),
        SkillStatus::Linked
    );
    let err = install(Skill::Beagle, Agent::Claude, home.path()).expect_err("refused");
    assert!(err.to_string().contains("symlink"), "{err}");
    assert_eq!(
        std::fs::read_to_string(repo_skill.join("SKILL.md")).expect("read"),
        "repo copy",
        "the checkout is untouched"
    );
    // Unlinked skills still install normally.
    install(Skill::Review, Agent::Claude, home.path()).expect("review installs");
}

#[cfg(unix)]
#[test]
fn install_refuses_a_symlinked_skill_file() {
    let home = tempfile::tempdir().expect("home");
    let target = home.path().join("elsewhere.md");
    std::fs::write(&target, "mine").expect("target");
    let prompts = home.path().join(".codex").join("prompts");
    std::fs::create_dir_all(&prompts).expect("prompts");
    std::os::unix::fs::symlink(&target, prompts.join("beagle.md")).expect("symlink");

    assert_eq!(
        status(Skill::Beagle, Agent::Codex, home.path()),
        SkillStatus::Linked
    );
    assert!(install(Skill::Beagle, Agent::Codex, home.path()).is_err());
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "mine");
}
