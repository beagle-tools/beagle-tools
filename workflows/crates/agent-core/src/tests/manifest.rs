use super::*;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn temp_root(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("agent-core-manifest-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn script_runner_takes_command_not_prompt() {
    let root = temp_root("script");
    write(
        &root.join("demo/fanout.yaml"),
        "kind: Agent\nname: fanout\nrunner: script\nworkdir: /tmp\ncommand: ./check.sh\n",
    );
    write(
        &root.join("demo/bad-noprompt.yaml"),
        "kind: Agent\nname: bad-noprompt\nrunner: claude\nworkdir: /tmp\n",
    );
    write(
        &root.join("demo/bad-tools.yaml"),
        "kind: Agent\nname: bad-tools\nrunner: script\nworkdir: /tmp\ncommand: x\nallowed_tools: [Bash]\n",
    );

    let out = load_root(&root).unwrap();
    assert_eq!(out.agents.len(), 1);
    let script = &out.agents[0];
    assert_eq!(script.runner, Runner::Script);
    assert_eq!(script.command.as_deref(), Some("./check.sh"));
    assert!(script.prompt.is_none());
    assert_eq!(out.issues.len(), 2);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn directory_prompt_must_contain_md_files() {
    let root = temp_root("dirprompt");
    write(
        &root.join("personal/prompts/digest/00-persona.md"),
        "you are…",
    );
    std::fs::create_dir_all(root.join("personal/prompts/empty")).unwrap();
    write(
        &root.join("personal/digest.yaml"),
        "kind: Agent\nname: digest\nrunner: claude\nworkdir: /tmp\nprompt: prompts/digest\n",
    );
    write(
        &root.join("personal/broken.yaml"),
        "kind: Agent\nname: broken\nrunner: claude\nworkdir: /tmp\nprompt: prompts/empty\n",
    );

    let out = load_root(&root).unwrap();
    assert_eq!(out.agents.len(), 1);
    assert!(out.agents[0].prompt.as_ref().unwrap().is_dir());
    assert_eq!(out.issues.len(), 1);
    assert!(out.issues[0].message.contains("no .md files"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn loads_project_with_agent_and_reports_bad_files() {
    let root = temp_root("load");
    let prompt = root.join("personal/prompts/digest.md");
    write(&prompt, "Summarize my day.");
    write(
        &root.join("personal/project.yaml"),
        "kind: Project\ndescription: personal automations\n",
    );
    write(
        &root.join("personal/digest.yaml"),
        "kind: Agent\nname: digest\nrunner: claude\nschedule: \"0 7 * * *\"\nworkdir: /tmp\nprompt: prompts/digest.md\n",
    );
    write(
        &root.join("personal/broken.yaml"),
        "kind: Agent\nname: [not a string\n",
    );

    let out = load_root(&root).unwrap();
    assert_eq!(out.projects.len(), 1);
    assert_eq!(
        out.projects[0].description.as_deref(),
        Some("personal automations")
    );
    assert_eq!(out.agents.len(), 1);
    let agent = &out.agents[0];
    assert_eq!(agent.qualified_name(), "personal/digest");
    assert!(agent.prompt.as_ref().unwrap().is_absolute());
    assert_eq!(agent.timeout_minutes, 30);
    assert_eq!(out.issues.len(), 1);

    let _ = std::fs::remove_dir_all(&root);
}
