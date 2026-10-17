// Builders for test data: a complete, valid object with only the fields
// a test cares about overridden.

import type { LocalComment, PullRequest } from "../lib/types";

export function makePr(overrides: Partial<PullRequest> = {}): PullRequest {
  return {
    repo: { owner: "acme", name: "widgets" },
    number: 1,
    title: "a pull request",
    body: "",
    state: "open",
    draft: false,
    author: { login: "alice", avatar_url: null },
    head_ref: "feature",
    head_sha: "0123456789abcdef",
    base_ref: "main",
    additions: 1,
    deletions: 0,
    changed_files: 1,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    labels: [],
    mergeable_state: null,
    requested_reviewers: [],
    node_id: null,
    review_decision: null,
    checks_state: null,
    unresolved_threads: 0,
    ...overrides,
  };
}

export function makeComment(overrides: Partial<LocalComment> = {}): LocalComment {
  return {
    id: "c1",
    repo: { owner: "acme", name: "widgets" },
    pr_number: 1,
    head_sha: "0123456789abcdef",
    path: "src/main.rs",
    side: "new",
    line: 10,
    end_line: null,
    body: "looks off",
    suggestion: null,
    author_kind: "human",
    author_name: "alice",
    severity: "info",
    committed_sha: null,
    edit_count: 0,
    tag: null,
    status: "open",
    run_id: null,
    parent_id: null,
    github_comment_id: null,
    posted_github_id: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}
