// Tests panic on a broken fixture; that is the failure mode we want.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;
use tandem_core::diff_parse::parse_unified_diff;

const DIFF: &str = "diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,2 +1,2 @@
 keep
-old
+new
";

fn running(cache: &Cache, run_id: &str) {
    let repo = RepoRef::parse("o/r").unwrap();
    let files = parse_unified_diff(DIFF).unwrap();
    cache.put_diff(&repo, 7, "sha1", DIFF, &files).unwrap();
    cache
        .put_agent_run(&AgentRun {
            run_id: run_id.into(),
            agent_name: "claude".into(),
            repo_slug: "o/r".into(),
            pr_number: 7,
            head_sha: "sha1".into(),
            status: RunStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            log_path: String::new(),
            comment_count: 0,
            purpose: PURPOSE.into(),
            target_comment_id: None,
            error: None,
        })
        .unwrap();
}

const COMMENT: &str = r#"{"type":"tandem_comment","path":"a.rs","line":2,"severity":"issue","tag":"duplicate","body":"bug","suggestion":"fixed","context":"history","duplicate_of":"@bot https://x"}"#;

#[test]
fn a_review_loads_once_with_its_history_replies() {
    let cache = Cache::open_in_memory().unwrap();
    running(&cache, "r1");
    let text = format!(
        "{}\n{}\n{COMMENT}\n",
        r#"{"type":"tandem_verdict","verdict":"not-mergeable","reason":"a bug"}"#,
        r###"{"type":"tandem_summary","body":"## What is the problem\nX"}"###,
    );
    assert_eq!(ingest(&cache, "r1", &text).unwrap(), 1);

    let repo = RepoRef::parse("o/r").unwrap();
    let comments = cache.list_comments(&repo, 7).unwrap();
    assert_eq!(comments.len(), 2);
    let root = comments.iter().find(|c| c.parent_id.is_none()).unwrap();
    let reply = comments.iter().find(|c| c.parent_id.is_some()).unwrap();
    assert_eq!(reply.parent_id.as_deref(), Some(root.id.as_str()));
    assert_eq!(root.suggestion.as_deref(), Some("fixed"));
    assert!(reply
        .body
        .starts_with("**Duplicate of** @bot https://x\n\nhistory"));

    let summary = cache.get_pr_summary(&repo, 7).unwrap().unwrap();
    assert_eq!(
        Verdict::from_summary(&summary.body),
        Some(Verdict::NotMergeable)
    );
    assert!(summary.body.contains("## What is the problem"));

    let run = cache.get_agent_run("r1").unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.comment_count, 1);
    assert!(matches!(
        ingest(&cache, "r1", &text),
        Err(IngestError::Run(_))
    ));
}

#[test]
fn a_re_review_keeps_the_summary_and_swaps_the_verdict() {
    let cache = Cache::open_in_memory().unwrap();
    running(&cache, "r1");
    let first = format!(
        "{}\n{}",
        r#"{"type":"tandem_verdict","verdict":"not-mergeable","reason":"a bug"}"#,
        r#"{"type":"tandem_summary","body":"the story"}"#,
    );
    ingest(&cache, "r1", &first).unwrap();
    running(&cache, "r2");
    let second = r#"{"type":"tandem_verdict","verdict":"mergeable","reason":"fixed"}"#;
    ingest(&cache, "r2", second).unwrap();
    let repo = RepoRef::parse("o/r").unwrap();
    let body = cache.get_pr_summary(&repo, 7).unwrap().unwrap().body;
    assert_eq!(body, "**Mergeable** — fixed\n\nthe story");
}

#[test]
fn a_bad_review_writes_nothing() {
    let cache = Cache::open_in_memory().unwrap();
    running(&cache, "r1");
    let errors = match ingest(&cache, "r1", COMMENT) {
        Err(IngestError::Review(errors)) => errors,
        other => panic!("{other:?}"),
    };
    assert!(errors.iter().any(|e| e.contains("tandem_verdict")));
    let repo = RepoRef::parse("o/r").unwrap();
    assert!(cache.list_comments(&repo, 7).unwrap().is_empty());
    assert_eq!(
        cache.get_agent_run("r1").unwrap().unwrap().status,
        RunStatus::Running
    );
}

#[test]
fn fail_marks_the_run() {
    let cache = Cache::open_in_memory().unwrap();
    running(&cache, "r1");
    fail(&cache, "r1", "timed out").unwrap();
    let run = cache.get_agent_run("r1").unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.error.as_deref(), Some("timed out"));
}
