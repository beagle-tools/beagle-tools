// Typed wrappers around Tauri invoke — the only place command names and
// argument shapes appear.

import { invoke } from "@tauri-apps/api/core";

import type {
  AgentRun,
  AgentSpec,
  AppRelease,
  FileConfigInfo,
  FileExcerpt,
  ResolutionState,
  MergeOptions,
  OwnerList,
  RepoSummary,
  CommentRevision,
  CommentStatus,
  LocalComment,
  McpServer,
  NewLocalComment,
  PrBundle,
  PrCommit,
  PrPage,
  PullRequest,
  RunEvent,
  Settings,
  WorkspaceInfo,
} from "./types";

export const ipc = {
  // pull requests
  getPullRequests: (repo: string): Promise<PullRequest[]> =>
    invoke<PullRequest[]>("get_pull_requests", { repo }),
  syncPullRequests: (repo: string, page = 1): Promise<PrPage> =>
    invoke<PrPage>("sync_pull_requests", { repo, page }),
  getPrBundle: (repo: string, number: number): Promise<PrBundle | null> =>
    invoke<PrBundle | null>("get_pr_bundle", { repo, number }),
  syncPrBundle: (repo: string, number: number): Promise<PrBundle> =>
    invoke<PrBundle>("sync_pr_bundle", { repo, number }),
  getLastSynced: (key: string): Promise<string | null> =>
    invoke<string | null>("get_last_synced", { key }),
  listReviewedPrs: (): Promise<PullRequest[]> => invoke<PullRequest[]>("list_reviewed_prs"),
  listMyPrs: (scope: string, repo: string | null): Promise<PullRequest[]> =>
    invoke<PullRequest[]>("list_my_prs", { scope, repo }),
  listFailingChecks: (repo: string, number: number): Promise<string[]> =>
    invoke<string[]>("list_failing_checks", { repo, number }),
  listPrCommits: (repo: string, number: number): Promise<PrCommit[]> =>
    invoke<PrCommit[]>("list_pr_commits", { repo, number }),
  forgetPr: (repo: string, number: number): Promise<null> =>
    invoke<null>("forget_pr", { repo, number }),
  markReadyForReview: (repo: string, number: number): Promise<null> =>
    invoke<null>("mark_ready_for_review", { repo, number }),
  replyToSummary: (
    agentName: string,
    repo: string,
    number: number,
    body: string,
  ): Promise<string> => invoke<string>("reply_to_summary", { agentName, repo, number, body }),

  // local review comments
  listLocalComments: (repo: string, number: number): Promise<LocalComment[]> =>
    invoke<LocalComment[]>("list_local_comments", { repo, number }),
  addLocalComment: (comment: NewLocalComment): Promise<LocalComment> =>
    invoke<LocalComment>("add_local_comment", { comment }),
  setCommentStatus: (
    id: string,
    status: CommentStatus,
    repo: string,
    number: number,
  ): Promise<null> => invoke<null>("set_comment_status", { id, status, repo, number }),
  getFileExcerpt: (
    repo: string,
    path: string,
    gitRef: string,
    line: number,
    radius: number,
  ): Promise<FileExcerpt> =>
    invoke<FileExcerpt>("get_file_excerpt", { repo, path, gitRef, line, radius }),
  listCommentRevisions: (id: string): Promise<CommentRevision[]> =>
    invoke<CommentRevision[]>("list_comment_revisions", { id }),
  updateCommentBody: (id: string, body: string, repo: string, number: number): Promise<null> =>
    invoke<null>("update_comment_body", { id, body, repo, number }),
  deleteLocalComment: (id: string, repo: string, number: number): Promise<null> =>
    invoke<null>("delete_local_comment", { id, repo, number }),
  deletePrSummary: (repo: string, number: number): Promise<null> =>
    invoke<null>("delete_pr_summary", { repo, number }),

  // agents
  listMcpServers: (): Promise<McpServer[]> => invoke<McpServer[]>("list_mcp_servers"),
  listAgentSpecs: (): Promise<AgentSpec[]> => invoke<AgentSpec[]>("list_agent_specs"),
  saveAgentSpec: (spec: AgentSpec): Promise<null> => invoke<null>("save_agent_spec", { spec }),
  deleteAgentSpec: (name: string): Promise<null> => invoke<null>("delete_agent_spec", { name }),
  listAgentRuns: (repo: string, number: number): Promise<AgentRun[]> =>
    invoke<AgentRun[]>("list_agent_runs", { repo, number }),
  listAllAgentRuns: (): Promise<AgentRun[]> => invoke<AgentRun[]>("list_all_agent_runs"),
  getRunLog: (runId: string): Promise<RunEvent[]> => invoke<RunEvent[]>("get_run_log", { runId }),
  startAgentReview: (agentName: string, repo: string, number: number): Promise<string> =>
    invoke<string>("start_agent_review", { agentName, repo, number }),
  cancelAgentRun: (runId: string): Promise<null> => invoke<null>("cancel_agent_run", { runId }),
  deleteAgentRun: (runId: string): Promise<null> => invoke<null>("delete_agent_run", { runId }),
  replyToComment: (agentName: string, commentId: string, body: string): Promise<string> =>
    invoke<string>("reply_to_comment", { agentName, commentId, body }),
  mentionAgent: (agentName: string, commentId: string): Promise<string> =>
    invoke<string>("mention_agent", { agentName, commentId }),
  startConflictFix: (agentName: string, repo: string, number: number): Promise<string> =>
    invoke<string>("start_conflict_fix", { agentName, repo, number }),
  conflictResolutionState: (repo: string, number: number): Promise<ResolutionState> =>
    invoke<ResolutionState>("conflict_resolution_state", { repo, number }),
  pushConflictResolution: (repo: string, number: number): Promise<string> =>
    invoke<string>("push_conflict_resolution", { repo, number }),
  startConflictResolution: (agentName: string, repo: string, number: number): Promise<string> =>
    invoke<string>("start_conflict_resolution", { agentName, repo, number }),

  // explicit GitHub writes (user-triggered only)
  postCommentToGithub: (commentId: string): Promise<number> =>
    invoke<number>("post_comment_to_github", { commentId }),
  commitSuggestion: (commentId: string): Promise<string> =>
    invoke<string>("commit_suggestion", { commentId }),
  resolveGithubThread: (threadId: string): Promise<null> =>
    invoke<null>("resolve_github_thread", { threadId }),
  mergePr: (repo: string, number: number, method: string): Promise<null> =>
    invoke<null>("merge_pr", { repo, number, method }),
  updatePrBranch: (repo: string, number: number): Promise<null> =>
    invoke<null>("update_pr_branch", { repo, number }),
  repoMergeOptions: (repo: string): Promise<MergeOptions> =>
    invoke<MergeOptions>("repo_merge_options", { repo }),
  enableAutoMerge: (repo: string, number: number, method: string): Promise<null> =>
    invoke<null>("enable_auto_merge", { repo, number, method }),
  approvePr: (repo: string, number: number, body: string | null): Promise<null> =>
    invoke<null>("approve_pr", { repo, number, body }),
  replyOnGithub: (request: {
    repo: string;
    number: number;
    body: string;
    review_comment_id: number | null;
    path: string | null;
    line: number | null;
    side_new: boolean | null;
  }): Promise<number> => invoke<number>("reply_on_github", { request }),

  // editor / working trees
  resolveWorkspace: (repo: string): Promise<WorkspaceInfo> =>
    invoke<WorkspaceInfo>("resolve_workspace", { repo }),
  openInEditor: (repo: string, number: number): Promise<string> =>
    invoke<string>("open_in_editor", { repo, number }),

  // text utilities
  polishText: (text: string): Promise<string> => invoke<string>("polish_text", { text }),

  // settings
  getSettings: (): Promise<Settings> => invoke<Settings>("get_settings"),
  fileConfigInfo: (): Promise<FileConfigInfo | null> =>
    invoke<FileConfigInfo | null>("file_config_info"),
  listGithubOwners: (): Promise<OwnerList> => invoke<OwnerList>("list_github_owners"),
  listGithubRepos: (owner: string, isViewer: boolean): Promise<RepoSummary[]> =>
    invoke<RepoSummary[]>("list_github_repos", { owner, isViewer }),
  listCollaborators: (repo: string): Promise<string[]> =>
    invoke<string[]>("list_collaborators", { repo }),
  updateSettings: (settings: Settings): Promise<Settings> =>
    invoke<Settings>("update_settings", { settings }),

  // app version management
  listAppReleases: (): Promise<AppRelease[]> => invoke<AppRelease[]>("list_app_releases"),
  installAppVersion: (tag: string): Promise<null> => invoke<null>("install_app_version", { tag }),
};
