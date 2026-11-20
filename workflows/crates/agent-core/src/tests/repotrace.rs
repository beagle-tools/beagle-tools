use super::*;

fn acme() -> Vec<String> {
    vec!["acme".to_owned()]
}

fn branches(cmd: &str) -> Vec<Option<String>> {
    writes_in(cmd, Path::new("/work"))
        .into_iter()
        .map(|w| w.branch)
        .collect()
}

#[test]
fn writes_name_their_branch_when_the_command_does() {
    let b = |s: &str| Some(s.to_owned());
    assert_eq!(
        branches("git push origin HEAD:abc-1-fix; git push -u origin +refs/heads/main; git push; git push origin HEAD; git commit -m x"),
        vec![b("abc-1-fix"), b("main"), None, None, None]
    );
    assert_eq!(
        branches("gh pr create -R acme/widgets --base main --head abc-2 --title x"),
        vec![b("abc-2")]
    );
}

fn verbs(cmd: &str) -> Vec<(Target, String)> {
    writes_in(cmd, Path::new("/work"))
        .into_iter()
        .map(|w| (w.target, w.verb))
        .collect()
}

#[test]
fn git_writes_name_their_checkout() {
    assert_eq!(
        verbs("git -C /r/widgets add main.tf && git -C /r/widgets commit -m x && git -C /r/widgets push origin HEAD:b"),
        vec![
            (Target::Local("/r/widgets".into()), "commit".to_owned()),
            (Target::Local("/r/widgets".into()), "push".to_owned()),
        ]
    );
    // Reads are not writes.
    assert_eq!(
        verbs(
            "git -C /r log --oneline -5; git status; git worktree list; git branch --show-current"
        ),
        Vec::new()
    );
    // No -C: the run's workdir.
    assert_eq!(verbs("git push")[0].0, Target::Local("/work".into()));
    assert_eq!(
        verbs("git -C /r worktree add -B x /r-x origin/b")[0].1,
        "worktree add"
    );
}

#[test]
fn gh_writes_name_their_repo() {
    assert_eq!(
        verbs("gh pr create --draft --repo acme/widgets --title x"),
        vec![(
            Target::GitHub("acme/widgets".to_owned()),
            "pr create".to_owned()
        )]
    );
    assert_eq!(
        verbs("gh pr comment https://github.com/acme/gadgets/pull/770 --body hi")[0].0,
        Target::GitHub("acme/gadgets".to_owned())
    );
    assert_eq!(
        verbs("gh api repos/acme/sprockets/pulls/1/comments/2/replies --method POST -f body=x")[0],
        (
            Target::GitHub("acme/sprockets".to_owned()),
            "api write".to_owned()
        )
    );
    // Reads: view, diff, checks, a plain GET, a graphql query.
    assert_eq!(
        verbs("gh pr view 1 -R a/b; gh pr diff 1; gh api repos/a/b/pulls/1/comments; gh api graphql -f query='query{x}'"),
        Vec::new()
    );
    // A graphql mutation is a write.
    assert_eq!(
        verbs("gh api graphql -f query='mutation{resolveReviewThread}' -f t=1")[0].1,
        "api write"
    );
}

#[test]
fn the_summary_splits_by_branch_and_flags_a_protected_main_push() {
    let w = |r: &str, b: &str, v: &str| (r.to_owned(), Some(b.to_owned()), v.to_owned());
    let writes = [
        w("acme/widgets", "abc-835-fix", "commit"),
        w("acme/widgets", "abc-835-fix", "commit"),
        w("acme/widgets", "abc-835-fix", "push"),
        w("jdoe/notes", "main", "push"),
    ];
    assert_eq!(
        summary(&writes, &acme()).unwrap(),
        "repos touched: acme/widgets @ abc-835-fix (commit ×2, push) · jdoe/notes @ main (push)"
    );
    let bad = [w("acme/widgets", "main", "push")];
    assert!(summary(&bad, &acme())
        .unwrap()
        .ends_with("@ main (push) ⚠ PUSHED TO MAIN"));
    // A local commit on main is not a push; unknown branch is shown bare.
    let local = [
        w("acme/widgets", "main", "commit"),
        ("acme/gadgets".to_owned(), None, "pr comment".to_owned()),
    ];
    assert_eq!(
        summary(&local, &acme()).unwrap(),
        "repos touched: acme/gadgets (pr comment) · acme/widgets @ main (commit)"
    );
    assert_eq!(summary(&[], &acme()), None);
}

#[test]
fn no_protected_orgs_guards_every_repo() {
    let push = [(
        "acme/widgets".to_owned(),
        Some("main".to_owned()),
        "push".to_owned(),
    )];
    assert!(summary(&push, &[]).unwrap().contains("PUSHED TO MAIN"));
    assert!(is_protected_main("acme/widgets", Some("main"), &[]));
    assert!(!is_protected_main("acme/widgets", Some("abc-1"), &[]));
    assert!(!is_protected_main("jdoe/widgets", Some("main"), &acme()));
    assert!(is_protected_main("ACME/widgets", Some("master"), &acme()));
    assert!(!is_protected_main("acme/widgets", Some("abc-1"), &acme()));
}
