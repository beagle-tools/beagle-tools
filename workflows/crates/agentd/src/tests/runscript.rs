use agent_core::types::ConcurrencyPolicy;

use super::*;
use crate::runenv::{env_exports, param_pairs};

fn spec(runner: Runner, model: Option<&str>) -> AgentSpec {
    AgentSpec {
        project: "personal".to_owned(),
        name: "digest".to_owned(),
        source: std::path::PathBuf::new(),
        runner,
        model: model.map(str::to_owned),
        schedule: None,
        workdir: "/tmp/repo".into(),
        prompt: Some("/tmp/prompts/digest.md".into()),
        command: None,
        allowed_tools: None,
        disallowed_tools: None,
        append_system_prompt: None,
        add_dirs: Vec::new(),
        params: Vec::new(),
        requires_mcp: Vec::new(),
        mcp_config: None,
        env: std::collections::BTreeMap::default(),
        env_file: None,
        database: false,
        max_concurrent: None,
        noop_retention_minutes: None,
        notify: None,
        notify_icon: None,
        notify_icon_failure: None,
        timeout_minutes: 30,
        concurrency: ConcurrencyPolicy::Forbid,
        isolation: Isolation::None,
        enabled: true,
        entrypoint: None,
    }
}

#[test]
fn claude_allowed_tools_flag() {
    let mut with_tools = spec(Runner::Claude, None);
    with_tools.allowed_tools = Some(vec!["Bash".to_owned(), "Read".to_owned()]);
    assert_eq!(
        runner_command(&with_tools, None),
        "claude -p --output-format stream-json --verbose --allowedTools 'Bash,Read'"
    );
}

#[test]
fn script_body_logs_and_marks_exit() {
    assert_eq!(
        script_body("./check.sh --fast", Path::new("/logs/run.jsonl")),
        "( ./check.sh --fast ) >> '/logs/run.jsonl' 2>&1; \
         printf \"__AGENTD_EXIT__:%s\\n\" \"$?\" >> '/logs/run.jsonl'"
    );
}

#[test]
fn the_prompt_is_told_the_chain_root_as_a_literal() {
    // The regression this exists for: linear-report was asked to
    // DERIVE the root id from the context pack's path, produced
    // `linear-<ts>-<suffix-of-a-different-run>`, and the write-up it
    // keyed on that name was swept as an orphan because no such run
    // existed. The root now arrives as a value, not a puzzle.
    let run = Run {
        id: "linear-report-3".to_owned(),
        project: "superblocks".to_owned(),
        agent: "linear-report".to_owned(),
        state: agent_core::types::RunState::Pending,
        trigger: agent_core::types::Trigger::Manual,
        parent_run_id: Some("linear-implement-2".to_owned()),
        params: None,
        tmux_session: None,
        log_path: None,
        workdir: None,
        model: None,
        created_at: chrono::Utc::now(),
        started_at: None,
        finished_at: None,
        exit_code: None,
        cost_usd: None,
        num_turns: None,
        usage_json: None,
    };
    let context = crate::runenv::run_context(&run, Path::new("/d"), "linear-1");
    assert!(
        context.contains("- chain root run id: linear-1"),
        "root missing from the run context:\n{context}"
    );
}

#[test]
fn env_exports_carry_identity_and_parent_links() {
    let run = Run {
        id: "child-1".to_owned(),
        project: "demo".to_owned(),
        agent: "child".to_owned(),
        state: agent_core::types::RunState::Pending,
        trigger: agent_core::types::Trigger::Manual,
        parent_run_id: Some("root-9".to_owned()),
        params: None,
        tmux_session: None,
        log_path: None,
        workdir: None,
        model: None,
        created_at: chrono::Utc::now(),
        started_at: None,
        finished_at: None,
        exit_code: None,
        cost_usd: None,
        num_turns: None,
        usage_json: None,
    };
    let mut with_env = spec(Runner::Claude, None);
    with_env
        .env
        .insert("NOTES_ROOT".to_owned(), "/stores/notes".to_owned());
    let exports = env_exports(&with_env, &run, Path::new("/state/data/child-1"), "root-1").unwrap();
    assert!(exports.contains("AGENTD_RUN_ID='child-1'"));
    assert!(exports.contains("AGENTD_PROJECT='demo'"));
    assert!(exports.contains("AGENTD_DATA_DIR='/state/data/child-1'"));
    assert!(exports.contains("AGENTD_PARENT_RUN_ID='root-9'"));
    assert!(exports.contains("AGENTD_PARENT_DATA_DIR='/state/data/root-9'"));
    assert!(exports.contains("NOTES_ROOT='/stores/notes'"));
    assert!(exports.contains(".cargo/bin"));
}

#[test]
fn prompt_params_keep_their_declared_names() {
    use crate::runenv::declared_pairs;
    let pairs = declared_pairs(Some(r#"{"url":"https://x/y","rca-title":"5xx"}"#));
    assert!(pairs.contains(&("url".to_owned(), "https://x/y".to_owned())));
    // The env form uppercases; the prompt form must not.
    assert!(pairs.contains(&("rca-title".to_owned(), "5xx".to_owned())));
}

#[test]
fn params_decode_to_env_safe_pairs() {
    let pairs = param_pairs(Some(r#"{"slug":"web-api-502","rca-title":"5xx spike"}"#));
    assert!(pairs.contains(&("SLUG".to_owned(), "web-api-502".to_owned())));
    assert!(pairs.contains(&("RCA_TITLE".to_owned(), "5xx spike".to_owned())));
    assert_eq!(
        param_pairs(None),
        [] as [(std::string::String, std::string::String); 0]
    );
    assert_eq!(
        param_pairs(Some("not json")),
        [] as [(std::string::String, std::string::String); 0]
    );
}

#[test]
fn claude_gets_append_system_prompt_and_add_dirs() {
    let mut full = spec(Runner::Claude, None);
    full.append_system_prompt = Some("/p/system.md".into());
    full.add_dirs = vec!["/stores/notes".into(), "/repos".into()];
    let cmd = runner_command(&full, None);
    assert!(cmd.contains("--append-system-prompt \"$(cat '/p/system.md')\""));
    assert!(cmd.contains("--add-dir '/stores/notes' --add-dir '/repos'"));
}

#[test]
fn disallowed_tools_carve_out_of_an_allowed_family() {
    // The case this exists for: `Bash(git:*)` is the only pattern that
    // matches `git -C <path> log`, but it also permits `git push`.
    let mut carved = spec(Runner::Claude, None);
    carved.allowed_tools = Some(vec!["Bash(git:*)".to_owned()]);
    carved.disallowed_tools = Some(vec![
        "Bash(git push:*)".to_owned(),
        "Bash(git reset:*)".to_owned(),
    ]);
    let cmd = runner_command(&carved, None);
    assert!(cmd.contains("--allowedTools 'Bash(git:*)'"));
    assert!(cmd.contains("--disallowedTools 'Bash(git push:*),Bash(git reset:*)'"));
    // Absent unless asked for.
    assert!(!runner_command(&spec(Runner::Claude, None), None).contains("--disallowedTools"));
}

#[test]
fn mcp_config_pins_the_server_set() {
    let mut pinned = spec(Runner::Claude, None);
    pinned.mcp_config = Some("/p/mcp.json".into());
    let cmd = runner_command(&pinned, None);
    assert!(cmd.contains("--mcp-config '/p/mcp.json' --strict-mcp-config"));
    // Without it, nothing is pinned and the ambient config applies.
    assert!(!runner_command(&spec(Runner::Claude, None), None).contains("--mcp-config"));
}

#[test]
fn run_model_overrides_the_manifest_model() {
    let with_model = spec(Runner::Claude, Some("sonnet"));
    assert!(runner_command(&with_model, Some("opus")).contains("--model 'opus'"));
    assert!(runner_command(&with_model, Some("sonnet")).contains("--model 'sonnet'"));
}

#[test]
fn claude_command_shape() {
    assert_eq!(
        runner_command(&spec(Runner::Claude, None), None),
        "claude -p --output-format stream-json --verbose"
    );
    assert_eq!(
        runner_command(&spec(Runner::Claude, None), Some("opus")),
        "claude -p --output-format stream-json --verbose --model 'opus'"
    );
}

#[test]
fn codex_command_shape_keeps_stdin_arg_last() {
    assert_eq!(
        runner_command(&spec(Runner::Codex, None), None),
        "codex exec --json --skip-git-repo-check -"
    );
    assert_eq!(
        runner_command(&spec(Runner::Codex, None), Some("o4-mini")),
        "codex exec --json --skip-git-repo-check -m 'o4-mini' -"
    );
}

#[test]
fn script_redirects_prompt_and_appends_exit_marker() {
    let script = run_script(
        &spec(Runner::Claude, None),
        None,
        &["/tmp/prompts/digest.md".into()],
        Path::new("/logs/run one.jsonl"),
    );
    assert_eq!(
        script,
        "claude -p --output-format stream-json --verbose < '/tmp/prompts/digest.md' \
         >> '/logs/run one.jsonl' 2>&1; \
         printf \"__AGENTD_EXIT__:%s\\n\" \"$?\" >> '/logs/run one.jsonl'"
    );
}

#[test]
fn script_concatenates_directory_prompts_in_order() {
    let script = run_script(
        &spec(Runner::Claude, None),
        None,
        &["/p/00-persona.md".into(), "/p/10-task.md".into()],
        Path::new("/logs/run.jsonl"),
    );
    assert_eq!(
        script,
        "cat '/p/00-persona.md' '/p/10-task.md' | \
         claude -p --output-format stream-json --verbose \
         >> '/logs/run.jsonl' 2>&1; \
         printf \"__AGENTD_EXIT__:%s\\n\" \"$?\" >> '/logs/run.jsonl'"
    );
}

#[test]
fn prompt_sources_lists_directory_md_files_sorted() {
    let dir = std::env::temp_dir().join(format!("agentd-prompts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("10-task.md"), "task").unwrap();
    std::fs::write(dir.join("00-persona.md"), "persona").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();

    let sources = prompt_sources(&dir).unwrap();
    let names: Vec<_> = sources
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
        .collect();
    assert_eq!(names, ["00-persona.md", "10-task.md"]);

    // A plain file passes through untouched; an empty dir is an error.
    let file = dir.join("00-persona.md");
    assert_eq!(prompt_sources(&file).unwrap(), vec![file]);
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(prompt_sources(&empty).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}
