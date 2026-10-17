// TypeScript mirrors of the Rust domain types (tandem-core). Keep field
// names in sync with the serde output: snake_case fields, snake_case
// enum values, internally-tagged enums using `kind`.

export interface RepoRef {
  owner: string;
  name: string;
}

export interface User {
  login: string;
  avatar_url: string | null;
}

export type PrState = "open" | "closed" | "merged";

export interface PullRequest {
  repo: RepoRef;
  number: number;
  title: string;
  body: string;
  state: PrState;
  draft: boolean;
  author: User;
  head_ref: string;
  head_sha: string;
  base_ref: string;
  additions: number;
  deletions: number;
  changed_files: number;
  created_at: string;
  updated_at: string;
  labels: string[];
  mergeable_state: string | null;
  requested_reviewers: string[];
  node_id: string | null;
  review_decision: string | null;
  checks_state: string | null;
  unresolved_threads: number;
}

export type CheckState = "pending" | "success" | "failure" | "neutral" | "cancelled" | "skipped";

export interface CheckRun {
  name: string;
  state: CheckState;
  details_url: string | null;
}

export type ReviewVerdict =
  "approved" | "changes_requested" | "commented" | "dismissed" | "pending";

export interface GithubReview {
  id: number;
  author: User;
  verdict: ReviewVerdict;
  body: string;
  submitted_at: string | null;
}

export interface GithubComment {
  id: number;
  author: User;
  body: string;
  path: string | null;
  line: number | null;
  original_line: number | null;
  in_reply_to_id: number | null;
  created_at: string;
}

export interface ReviewThreadMeta {
  id: string;
  resolved: boolean;
  root_comment_id: number;
}

export interface PrDetail {
  pull_request: PullRequest;
  checks: CheckRun[];
  reviews: GithubReview[];
  comments: GithubComment[];
  review_bodies: GithubReview[];
  review_threads: ReviewThreadMeta[];
}

// ---- diff ----------------------------------------------------------------

export type LineKind = "context" | "added" | "removed";

export interface DiffLine {
  kind: LineKind;
  old_line: number | null;
  new_line: number | null;
  content: string;
}

export interface Hunk {
  old_start: number;
  old_count: number;
  new_start: number;
  new_count: number;
  section: string;
  lines: DiffLine[];
}

export type FileStatus = "added" | "removed" | "modified" | "renamed";

export interface FileDiff {
  old_path: string;
  new_path: string;
  status: FileStatus;
  hunks: Hunk[];
  additions: number;
  deletions: number;
  is_binary: boolean;
}

// ---- local review ----------------------------------------------------------

export type CommentAuthorKind = "human" | "agent";
export type CommentStatus = "open" | "accepted" | "rejected" | "resolved" | "archived";
export type CommentSeverity = "info" | "suggestion" | "issue" | "blocker";
export type DiffSide = "old" | "new";

export interface LocalComment {
  id: string;
  repo: RepoRef;
  pr_number: number;
  head_sha: string;
  path: string;
  side: DiffSide;
  line: number;
  end_line: number | null;
  body: string;
  suggestion: string | null;
  author_kind: CommentAuthorKind;
  author_name: string;
  severity: CommentSeverity;
  /** Commit this suggestion became, once committed. */
  committed_sha: string | null;
  /** How many times the body has been rewritten. */
  edit_count: number;
  /** What kind of remark it is, e.g. "coding-standards" — free-form,
   * set by the agent; null when it didn't say. */
  tag: string | null;
  status: CommentStatus;
  run_id: string | null;
  parent_id: string | null;
  github_comment_id: number | null;
  posted_github_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface NewLocalComment {
  repo: RepoRef;
  pr_number: number;
  head_sha: string;
  path: string;
  side: DiffSide;
  line: number;
  end_line: number | null;
  body: string;
  suggestion: string | null;
  author_kind: CommentAuthorKind;
  author_name: string;
  severity: CommentSeverity;
  tag: string | null;
  run_id: string | null;
  parent_id: string | null;
  github_comment_id: number | null;
}

/** Lines read straight from a file at a commit, for comments anchored
 * outside the diff. */
export interface FileExcerpt {
  start_line: number;
  lines: string[];
}

/** Where a conflict resolution stands in the agent's checkout. */
export interface ResolutionState {
  unresolved: string[];
  ready: boolean;
}

/** A superseded version of a comment body. */
export interface CommentRevision {
  id: string;
  comment_id: string;
  edited_at: string;
  editor: string;
  body: string;
}

// ---- agents ---------------------------------------------------------------

/** An MCP server the local `claude` CLI knows about. */
export interface McpServer {
  name: string;
  /** Prefix its tools carry, e.g. "mcp__linear". */
  tool_prefix: string;
  detail: string;
  connected: boolean;
}

export type RunnerKind =
  | { kind: "claude_headless" }
  | { kind: "codex_headless" }
  | { kind: "grok_headless" }
  | { kind: "custom"; command: string };

export type AuthMode =
  | { kind: "cli_session" }
  | { kind: "api_key"; env_var: string }
  | { kind: "endpoint"; base_url: string; env_var: string | null };

export interface AgentSpec {
  name: string;
  runner: RunnerKind;
  auth: AuthMode;
  model: string | null;
  allowed_tools: string[];
  append_system_prompt: string | null;
  prompt: string;
  env: Record<string, string>;
  network_allowlist: string[];
  timeout_minutes: number;
  summary: boolean;
}

export type RunStatus = "starting" | "running" | "succeeded" | "failed" | "cancelled" | "timed_out";
export type RunEventKind = "lifecycle" | "runner" | "comment" | "raw";

export interface RunEvent {
  run_id: string;
  seq: number;
  at: string;
  kind: RunEventKind;
  payload: string;
}

export interface AgentRun {
  run_id: string;
  agent_name: string;
  repo_slug: string;
  pr_number: number;
  head_sha: string;
  status: RunStatus;
  started_at: string;
  finished_at: string | null;
  log_path: string;
  comment_count: number;
  purpose: string;
  target_comment_id: string | null;
  error: string | null;
}

// ---- app-level -------------------------------------------------------------

export type GithubAuth =
  { kind: "gh_cli" } | { kind: "token"; token: string } | { kind: "anonymous" };

export interface GithubConfig {
  auth: GithubAuth;
  api_base: string;
}

export interface PrFilters {
  query: string;
  author: string;
  label: string;
  hide_drafts: boolean;
}

export const EMPTY_FILTERS: PrFilters = { query: "", author: "", label: "", hide_drafts: false };

export interface PrCommit {
  sha: string;
  message: string;
  author: string;
  authored_at: string | null;
}

export interface WorkspaceInfo {
  path: string;
  managed: boolean;
  exists: boolean;
}

export interface Settings {
  github: GithubConfig;
  orgs: string[];
  default_repo: string | null;
  pr_filters: PrFilters;
  pr_sort: string;
  inbox_all_repos: boolean;
  refresh_minutes: number;
  editor_command: string;
  repo_paths: Record<string, string>;
  version: number;
}

export interface PrSummary {
  repo: RepoRef;
  pr_number: number;
  body: string;
  agent_name: string;
  run_id: string;
  head_sha: string;
  updated_at: string;
  /** Root of the local thread discussing this summary, once started. */
  thread_root_id: string | null;
}

export interface PrBundle {
  detail: PrDetail;
  diff: FileDiff[];
  comments: LocalComment[];
  summary: PrSummary | null;
}

export interface ArchivedPr {
  pull_request: PullRequest;
  archived_at: string;
  purge_after: string;
}

export interface RepoSummary {
  full_name: string;
  private: boolean;
  description: string | null;
}

export interface OwnerList {
  viewer: string;
  orgs: string[];
}

export type InboxScope = "requested" | "mentions" | "authored" | "approved";

export interface FileConfigInfo {
  path: string;
  agents: number;
  overrides: string[];
}

export interface MergeOptions {
  squash: boolean;
  merge: boolean;
  rebase: boolean;
  auto_merge: boolean;
}

export interface PrPage {
  prs: PullRequest[];
  has_more: boolean;
}

export interface SyncEvent {
  key: string;
  phase: "started" | "finished" | "error";
  error: string | null;
}

/** One published Tandem release (mirrors commands/updates.rs). */
export interface AppRelease {
  version: string;
  tag: string;
  published_at: string | null;
  installable: boolean;
}
