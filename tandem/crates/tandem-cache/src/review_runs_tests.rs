//! Tests for [`crate::ReviewStore`]'s comment mutations and agent runs —
//! beside `review_store_tests.rs` to honor the 400-line file limit.

#![allow(clippy::unwrap_used)]

use chrono::{Duration, Utc};
use tandem_core::agent::{AgentRun, RunStatus};
use tandem_core::github::RepoRef;
use tandem_core::review::{CommentAuthorKind, CommentSeverity, DiffSide, NewLocalComment};

use crate::store::Cache;
use crate::ReviewStore;

fn comment(repo: &RepoRef, pr: u64, run: Option<&str>) -> NewLocalComment {
    NewLocalComment {
        repo: repo.clone(),
        pr_number: pr,
        head_sha: "abc".into(),
        path: "src/main.rs".into(),
        side: DiffSide::New,
        line: 3,
        body: "first draft".into(),
        author_kind: CommentAuthorKind::Agent,
        author_name: "claude".into(),
        severity: CommentSeverity::Suggestion,
        run_id: run.map(str::to_owned),
        tag: None,
        parent_id: None,
        end_line: None,
        suggestion: None,
        github_comment_id: None,
    }
}

fn run(id: &str, repo: &str, pr: u64, age_mins: i64, status: RunStatus) -> AgentRun {
    AgentRun {
        run_id: id.into(),
        agent_name: "claude".into(),
        repo_slug: repo.into(),
        pr_number: pr,
        head_sha: "abc".into(),
        status,
        started_at: Utc::now() - Duration::minutes(age_mins),
        finished_at: None,
        log_path: "/tmp/x".into(),
        comment_count: 0,
        purpose: "pr review".into(),
        target_comment_id: None,
        error: None,
    }
}

fn ids(runs: &[AgentRun]) -> Vec<&str> {
    runs.iter().map(|r| r.run_id.as_str()).collect()
}

#[test]
fn posting_committing_and_editing_are_recorded() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("acme/widgets").unwrap();
    let c = cache.add_comment(comment(&repo, 1, None)).unwrap();

    cache.set_comment_posted(&c.id, 4242).unwrap();
    cache.set_comment_committed(&c.id, "c0ffee").unwrap();
    cache.update_comment_body(&c.id, "second draft").unwrap();
    cache.update_comment_body(&c.id, "final").unwrap();

    let got = cache.get_comment(&c.id).unwrap().unwrap();
    assert_eq!(got.posted_github_id, Some(4242));
    assert_eq!(got.committed_sha.as_deref(), Some("c0ffee"));
    assert_eq!(got.body, "final");
    assert_eq!(got.edit_count, 2);
    assert!(got.updated_at >= got.created_at);
    assert!(cache.get_comment("missing").unwrap().is_none());
    assert!(cache.set_comment_posted("missing", 1).is_err());
}

#[test]
fn comments_are_listed_per_pr() {
    let cache = Cache::open_in_memory().unwrap();
    let a = RepoRef::parse("acme/widgets").unwrap();
    let b = RepoRef::parse("acme/gadgets").unwrap();
    cache.add_comment(comment(&a, 1, None)).unwrap();
    cache.add_comment(comment(&a, 2, None)).unwrap();
    cache.add_comment(comment(&b, 1, None)).unwrap();
    assert_eq!(cache.list_comments(&a, 1).unwrap().len(), 1);
    assert_eq!(cache.list_comments(&a, 2).unwrap().len(), 1);
    assert_eq!(cache.list_comments(&b, 1).unwrap().len(), 1);
    assert!(cache.list_comments(&b, 2).unwrap().is_empty());
}

#[test]
fn runs_round_trip_and_list_newest_first() {
    let cache = Cache::open_in_memory().unwrap();
    cache
        .put_agent_run(&run("old", "acme/widgets", 1, 60, RunStatus::Succeeded))
        .unwrap();
    cache
        .put_agent_run(&run("new", "acme/widgets", 1, 5, RunStatus::Failed))
        .unwrap();
    cache
        .put_agent_run(&run("other", "acme/gadgets", 9, 1, RunStatus::Succeeded))
        .unwrap();

    let got = cache.get_agent_run("new").unwrap().unwrap();
    assert_eq!(got.status, RunStatus::Failed);
    assert!(cache.get_agent_run("nope").unwrap().is_none());

    let repo = RepoRef::parse("acme/widgets").unwrap();
    assert_eq!(
        ids(&cache.list_agent_runs(&repo, 1).unwrap()),
        ["new", "old"]
    );
    assert_eq!(ids(&cache.list_recent_runs(2).unwrap()), ["other", "new"]);

    // Re-putting a run replaces it rather than duplicating it.
    let mut done = got;
    done.status = RunStatus::Succeeded;
    cache.put_agent_run(&done).unwrap();
    assert_eq!(cache.list_recent_runs(10).unwrap().len(), 3);
    assert_eq!(
        cache.get_agent_run("new").unwrap().unwrap().status,
        RunStatus::Succeeded
    );
}

#[test]
fn deleting_a_run_takes_its_conversation_with_it() {
    let cache = Cache::open_in_memory().unwrap();
    let repo = RepoRef::parse("acme/widgets").unwrap();
    cache
        .put_agent_run(&run("r1", "acme/widgets", 1, 5, RunStatus::Succeeded))
        .unwrap();
    let from_run = cache.add_comment(comment(&repo, 1, Some("r1"))).unwrap();
    let mut reply = comment(&repo, 1, None);
    reply.parent_id = Some(from_run.id.clone());
    reply.author_kind = CommentAuthorKind::Human;
    cache.add_comment(reply).unwrap();
    let unrelated = cache.add_comment(comment(&repo, 1, Some("r2"))).unwrap();

    cache.delete_agent_run("r1").unwrap();

    assert!(cache.get_agent_run("r1").unwrap().is_none());
    let left = cache.list_comments(&repo, 1).unwrap();
    assert_eq!(left.len(), 1, "the run's comment and its reply are gone");
    assert_eq!(left[0].id, unrelated.id);
}

#[test]
fn a_restart_marks_unfinished_runs_failed() {
    let cache = Cache::open_in_memory().unwrap();
    for (id, status) in [
        ("starting", RunStatus::Starting),
        ("running", RunStatus::Running),
        ("done", RunStatus::Succeeded),
        ("cancelled", RunStatus::Cancelled),
    ] {
        cache
            .put_agent_run(&run(id, "acme/widgets", 1, 5, status))
            .unwrap();
    }

    assert_eq!(cache.sweep_stale_runs().unwrap(), 2);

    for id in ["starting", "running"] {
        let swept = cache.get_agent_run(id).unwrap().unwrap();
        assert_eq!(swept.status, RunStatus::Failed, "{id}");
        assert!(swept.finished_at.is_some(), "{id}");
        assert!(swept.error.unwrap().contains("interrupted"), "{id}");
    }
    assert_eq!(
        cache.get_agent_run("done").unwrap().unwrap().status,
        RunStatus::Succeeded
    );
    assert_eq!(
        cache.get_agent_run("cancelled").unwrap().unwrap().status,
        RunStatus::Cancelled
    );
    // Nothing is left to sweep the second time.
    assert_eq!(cache.sweep_stale_runs().unwrap(), 0);
}
