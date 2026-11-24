// Realistic fixture data for the e2e IPC mock (src/e2e/mockIpc.ts). Only
// loaded when vite runs with `--mode e2e`; never part of a production
// bundle. Shapes mirror src/lib/types.ts (and so the Rust serde output).

import type {
  AgentRun,
  AgentSpec,
  AppRelease,
  FileDiff,
  GithubComment,
  LocalComment,
  PrBundle,
  PullRequest,
  RepoRef,
  Settings,
} from "../lib/types";

export const VIEWER = "octo-dev";
export const REPO: RepoRef = { owner: "acme", name: "widgets" };
export const REPO_SLUG = "acme/widgets";
export const APP_VERSION = "0.4.2";

const user = (login: string) => ({ login, avatar_url: null });

function pr(number: number, title: string, overrides: Partial<PullRequest> = {}): PullRequest {
  return {
    repo: REPO,
    number,
    title,
    body: "",
    state: "open",
    draft: false,
    author: user("alice"),
    head_ref: `feature/${String(number)}`,
    head_sha: `${String(number)}a1b2c3d4e5f60718293a4b5c6d7e8f90`.slice(0, 40),
    base_ref: "main",
    additions: 42,
    deletions: 7,
    changed_files: 2,
    created_at: "2026-10-12T09:15:00Z",
    updated_at: "2026-10-15T16:40:00Z",
    labels: [],
    mergeable_state: "clean",
    requested_reviewers: [VIEWER],
    node_id: `PR_kwDO${String(number)}`,
    review_decision: "review_required",
    checks_state: "success",
    unresolved_threads: 1,
    ...overrides,
  };
}

export const REVIEW_REQUESTS: PullRequest[] = [
  pr(128, "Add retry with jitter to the webhook dispatcher", {
    body: "Webhook deliveries fail hard on a single 502. This adds bounded retries with full jitter.",
    labels: ["backend", "reliability"],
  }),
  pr(131, "Cache rendered markdown per comment", {
    author: user("bob"),
    additions: 88,
    deletions: 12,
    labels: ["frontend"],
    checks_state: "pending",
    unresolved_threads: 0,
  }),
];

export const AUTHORED: PullRequest[] = [
  pr(117, "Bump the SQLite schema to v9", {
    author: user(VIEWER),
    requested_reviewers: ["alice"],
    review_decision: "approved",
    unresolved_threads: 0,
  }),
];

export const MENTIONS: PullRequest[] = [
  pr(109, "Document the release process", {
    author: user("carol"),
    requested_reviewers: [],
    unresolved_threads: 0,
  }),
];

export const ALL_PRS: PullRequest[] = [...REVIEW_REQUESTS, ...AUTHORED, ...MENTIONS];

export const SETTINGS: Settings = {
  github: { auth: { kind: "gh_cli" }, api_base: "https://api.github.com" },
  orgs: ["acme"],
  default_repo: REPO_SLUG,
  pr_filters: { query: "", author: "", label: "", hide_drafts: false },
  pr_sort: "opened-desc",
  inbox_all_repos: false,
  refresh_minutes: 5,
  editor_command: "code",
  repo_paths: {},
  version: 1,
};

export const AGENT_SPECS: AgentSpec[] = [
  {
    name: "claude-reviewer",
    runner: { kind: "claude_headless" },
    auth: { kind: "cli_session" },
    model: null,
    allowed_tools: ["Read", "Grep", "Glob"],
    append_system_prompt: null,
    prompt: "Review this pull request for correctness bugs and missing tests.",
    env: {},
    network_allowlist: [],
    timeout_minutes: 20,
    summary: true,
  },
];

const DIFF: FileDiff[] = [
  {
    old_path: "src/webhooks/dispatch.rs",
    new_path: "src/webhooks/dispatch.rs",
    status: "modified",
    additions: 6,
    deletions: 1,
    is_binary: false,
    hunks: [
      {
        old_start: 40,
        old_count: 4,
        new_start: 40,
        new_count: 9,
        section: "pub async fn deliver(&self, hook: &Hook) -> Result<()> {",
        lines: [
          {
            kind: "context",
            old_line: 40,
            new_line: 40,
            content: "    let body = hook.payload()?;",
          },
          {
            kind: "removed",
            old_line: 41,
            new_line: null,
            content: "    self.client.post(&hook.url).body(body).send().await?;",
          },
          { kind: "added", old_line: null, new_line: 41, content: "    let mut attempt = 0;" },
          { kind: "added", old_line: null, new_line: 42, content: "    loop {" },
          {
            kind: "added",
            old_line: null,
            new_line: 43,
            content: "        match self.try_send(&hook.url, &body).await {",
          },
          {
            kind: "added",
            old_line: null,
            new_line: 44,
            content: "            Err(e) if attempt < MAX_RETRIES => attempt += 1,",
          },
          {
            kind: "added",
            old_line: null,
            new_line: 45,
            content: "            other => return other,",
          },
          { kind: "added", old_line: null, new_line: 46, content: "        }" },
          { kind: "context", old_line: 42, new_line: 47, content: "    }" },
          { kind: "context", old_line: 43, new_line: 48, content: "}" },
        ],
      },
    ],
  },
  {
    old_path: "src/webhooks/backoff.rs",
    new_path: "src/webhooks/backoff.rs",
    status: "added",
    additions: 3,
    deletions: 0,
    is_binary: false,
    hunks: [
      {
        old_start: 0,
        old_count: 0,
        new_start: 1,
        new_count: 3,
        section: "",
        lines: [
          {
            kind: "added",
            old_line: null,
            new_line: 1,
            content: "pub fn full_jitter(base_ms: u64, attempt: u32) -> u64 {",
          },
          {
            kind: "added",
            old_line: null,
            new_line: 2,
            content: "    fastrand::u64(0..base_ms << attempt.min(6))",
          },
          { kind: "added", old_line: null, new_line: 3, content: "}" },
        ],
      },
    ],
  },
];

const GITHUB_COMMENTS: GithubComment[] = [
  {
    id: 9001,
    author: user("bob"),
    body: "Should MAX_RETRIES be configurable per hook?",
    path: "src/webhooks/dispatch.rs",
    line: 44,
    original_line: 44,
    in_reply_to_id: null,
    created_at: "2026-10-14T11:02:00Z",
  },
];

export const LOCAL_COMMENTS: LocalComment[] = [
  {
    id: "lc-1",
    repo: REPO,
    pr_number: 128,
    head_sha: REVIEW_REQUESTS[0]?.head_sha ?? "",
    path: "src/webhooks/dispatch.rs",
    side: "new",
    line: 44,
    end_line: null,
    body: "The retry loop never sleeps between attempts — call `full_jitter` before retrying.",
    suggestion: null,
    author_kind: "agent",
    author_name: "claude-reviewer",
    severity: "issue",
    committed_sha: null,
    edit_count: 0,
    tag: "correctness",
    status: "open",
    run_id: "run-1",
    parent_id: null,
    github_comment_id: null,
    posted_github_id: null,
    created_at: "2026-10-15T17:00:00Z",
    updated_at: "2026-10-15T17:00:00Z",
  },
];

export const AGENT_RUNS: AgentRun[] = [
  {
    run_id: "run-1",
    agent_name: "claude-reviewer",
    repo_slug: REPO_SLUG,
    pr_number: 128,
    head_sha: REVIEW_REQUESTS[0]?.head_sha ?? "",
    status: "succeeded",
    started_at: "2026-10-15T16:55:00Z",
    finished_at: "2026-10-15T17:00:30Z",
    log_path: "/tmp/tandem-e2e/runs/run-1/events.jsonl",
    comment_count: 1,
    purpose: "review",
    target_comment_id: null,
    error: null,
  },
];

/** The full review bundle for a PR, or null when it isn't a fixture. */
export function bundleFor(number: number): PrBundle | null {
  const pull = ALL_PRS.find((p) => p.number === number);
  if (!pull) return null;
  const rich = number === 128;
  return {
    detail: {
      pull_request: pull,
      checks: [
        { name: "ci / rust", state: "success", details_url: null },
        { name: "ci / frontend", state: "success", details_url: null },
      ],
      reviews: [],
      comments: rich ? GITHUB_COMMENTS : [],
      review_bodies: [],
      review_threads: rich ? [{ id: "PRRT_1", resolved: false, root_comment_id: 9001 }] : [],
    },
    diff: rich ? DIFF : DIFF.slice(1),
    comments: LOCAL_COMMENTS.filter((c) => c.pr_number === number),
    summary: null,
  };
}

export const RELEASES: AppRelease[] = [
  {
    version: "0.4.3",
    tag: "beagle-tandem-v0.4.3",
    published_at: "2026-10-15T08:00:00Z",
    installable: true,
  },
  {
    version: "0.4.2",
    tag: "beagle-tandem-v0.4.2",
    published_at: "2026-10-09T08:00:00Z",
    installable: true,
  },
  {
    version: "0.4.1",
    tag: "beagle-tandem-v0.4.1",
    published_at: "2026-10-01T08:00:00Z",
    installable: true,
  },
];
