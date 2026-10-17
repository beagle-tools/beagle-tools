// The AppStore contract — state shape + actions. Implementation lives
// in store.ts; kept separate so both stay under the 400-line rule.

import type { PrSort } from "../lib/sort";
import type {
  AgentRun,
  AgentSpec,
  CommentStatus,
  LocalComment,
  NewLocalComment,
  PrBundle,
  InboxScope,
  PullRequest,
  RepoSummary,
  RunEvent,
  Settings,
} from "../lib/types";

export type View = "review" | "settings" | "agents" | "review-status";
export type Theme = "dark" | "light";

export interface AppStore {
  view: View;
  theme: Theme;
  rightPinned: boolean;
  settings: Settings | null;
  selectedRepo: string | null;
  prs: PullRequest[];
  selectedPr: number | null;
  bundle: PrBundle | null;
  agentSpecs: AgentSpec[];
  runs: AgentRun[];
  agentEvents: RunEvent[];
  syncing: Record<string, boolean>;
  prHasMore: boolean;
  prPage: number;
  inbox: Partial<Record<InboxScope, PullRequest[]>>;
  prSort: PrSort;
  viewer: string | null;
  collaborators: string[];
  /** Set while "all repositories" is loading; cleared when it finishes
   * or a different repo is selected before it does. */
  allRepoProgress: { done: number; total: number } | null;
  /** Repos across every tracked org — feeds the repo picker and "all
   * repositories" aggregation. Refreshed on init and whenever the
   * tracked org list changes. */
  orgRepos: RepoSummary[];
  /** Account-wide (every tracked org) PRs where your review is
   * requested — independent of whatever repo happens to be selected.
   * Refreshed on init and on a standing timer; feeds the "review
   * status" page and its new-arrival toasts. */
  reviewRequests: PullRequest[];
  /** Open PRs you've run a review on that nobody asked you to review —
   * they ride along in the requested tab until they merge. */
  reviewing: PullRequest[];
  /** False until the first `refreshReviewRequests` completes — guards
   * against toasting for every already-pending request on startup. */
  reviewRequestsSeeded: boolean;

  init: () => Promise<void>;
  setView: (view: View) => void;
  toggleTheme: () => void;
  goHome: () => void;
  toggleRightPane: () => void;
  replyToComment: (commentId: string, body: string, agentName: string) => Promise<void>;
  mentionAgent: (agentName: string, commentId: string) => Promise<void>;
  /** Resolves to the commit sha; rejects with the GitHub error so the
   * caller can show it where the button is. */
  commitSuggestion: (commentId: string) => Promise<string>;
  postToGithub: (commentId: string) => Promise<void>;
  approvePr: (body: string | null) => Promise<void>;
  loadMorePrs: () => Promise<void>;
  loadInbox: (scope: InboxScope, force?: boolean) => Promise<void>;
  /** Load req, mine, mentions and approved together, whichever tab
   * you're on; rate limits wait and retry rather than failing. */
  loadQueues: (force?: boolean) => Promise<void>;
  refreshInboxes: () => Promise<void>;
  setPrSort: (sort: PrSort) => void;
  openPr: (repoSlug: string, number: number) => Promise<void>;
  selectRepo: (slug: string, presetPr?: number) => Promise<void>;
  loadOrgRepos: () => Promise<void>;
  refreshReviewRequests: () => Promise<void>;
  loadReviewing: () => Promise<void>;
  selectPr: (number: number) => Promise<void>;
  refreshPrs: () => Promise<void>;
  refreshBundle: () => Promise<void>;
  saveSettings: (settings: Settings) => Promise<void>;
  addComment: (comment: NewLocalComment) => Promise<LocalComment | null>;
  setCommentStatus: (id: string, status: CommentStatus) => Promise<void>;
  deleteComment: (id: string) => Promise<void>;
  deleteSummary: () => Promise<void>;
  saveAgentSpec: (spec: AgentSpec) => Promise<void>;
  deleteAgentSpec: (name: string) => Promise<void>;
  startAgentReview: (agentName: string) => Promise<void>;
  quickStartReview: (repo: string, number: number, agentName: string) => Promise<void>;
  cancelRun: (runId: string) => Promise<void>;
}
