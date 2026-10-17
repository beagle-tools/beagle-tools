// e2e IPC mock: answers every Tauri command the UI sends with fixture data,
// so Playwright can drive the real frontend in a plain browser. main.tsx
// imports this only when vite runs with `--mode e2e`; production builds
// drop it (and the fixtures) entirely.
//
// Knobs (query string on the page URL):
//   ?update=X.Y.Z   the updater reports X.Y.Z as available (default: none)

import type { InvokeArgs } from "@tauri-apps/api/core";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import type { LocalComment, NewLocalComment, Settings } from "../lib/types";
import {
  AGENT_RUNS,
  AGENT_SPECS,
  ALL_PRS,
  APP_VERSION,
  AUTHORED,
  LOCAL_COMMENTS,
  MENTIONS,
  RELEASES,
  REPO_SLUG,
  REVIEW_REQUESTS,
  SETTINGS,
  VIEWER,
  bundleFor,
} from "./fixtures";

type Args = Record<string, unknown>;

function asArgs(payload: InvokeArgs | undefined): Args {
  return payload !== undefined && !Array.isArray(payload) && !ArrayBuffer.isView(payload)
    ? (payload as Args)
    : {};
}

const num = (a: Args, key: string): number => Number(a[key] ?? 0);
const str = (a: Args, key: string): string => (typeof a[key] === "string" ? a[key] : "");

let settings: Settings = SETTINGS;
const comments: LocalComment[] = [...LOCAL_COMMENTS];

function addComment(input: NewLocalComment): LocalComment {
  const now = new Date().toISOString();
  const comment: LocalComment = {
    ...input,
    id: `lc-${String(comments.length + 1)}`,
    committed_sha: null,
    edit_count: 0,
    status: "open",
    posted_github_id: null,
    created_at: now,
    updated_at: now,
  };
  comments.push(comment);
  return comment;
}

function inbox(scope: string) {
  switch (scope) {
    case "requested":
      return REVIEW_REQUESTS;
    case "authored":
      return AUTHORED;
    case "mentions":
      return MENTIONS;
    default:
      return [];
  }
}

/** Tauri plugin commands (`plugin:<name>|<cmd>`) the UI relies on. */
function plugin(cmd: string): unknown {
  switch (cmd) {
    case "plugin:app|version":
      return APP_VERSION;
    case "plugin:updater|check": {
      const version = new URLSearchParams(window.location.search).get("update");
      if (version === null) return null;
      return {
        rid: 1,
        currentVersion: APP_VERSION,
        version,
        date: "2026-10-16T00:00:00Z",
        body: "",
        rawJson: {},
      };
    }
    default:
      // window/opener/process calls: accept silently.
      return null;
  }
}

function prCommand(cmd: string, a: Args): unknown {
  switch (cmd) {
    case "get_pull_requests":
      return ALL_PRS;
    case "sync_pull_requests":
      return { prs: ALL_PRS, has_more: false };
    case "get_pr_bundle":
    case "sync_pr_bundle": {
      const bundle = bundleFor(num(a, "number"));
      return (
        bundle && { ...bundle, comments: comments.filter((c) => c.pr_number === num(a, "number")) }
      );
    }
    case "get_last_synced":
      return "2026-10-16T08:00:00Z";
    case "list_reviewed_prs":
      return [];
    case "list_my_prs":
      return inbox(str(a, "scope"));
    case "list_failing_checks":
      return [];
    case "list_pr_commits":
      return [
        { sha: "a1b2c3d", message: "Retry webhook deliveries", author: "alice", authored_at: null },
      ];
    default:
      return undefined;
  }
}

function reviewCommand(cmd: string, a: Args): unknown {
  switch (cmd) {
    case "list_local_comments":
      return comments.filter((c) => c.pr_number === num(a, "number"));
    case "add_local_comment":
      return addComment(a.comment as NewLocalComment);
    case "get_file_excerpt":
      return { start_line: num(a, "line"), lines: ["    let body = hook.payload()?;"] };
    case "list_comment_revisions":
      return [];
    case "list_mcp_servers":
      return [{ name: "linear", tool_prefix: "mcp__linear", detail: "stdio", connected: true }];
    case "list_agent_specs":
      return AGENT_SPECS;
    case "list_agent_runs":
      return AGENT_RUNS.filter((r) => r.pr_number === num(a, "number"));
    case "list_all_agent_runs":
      return AGENT_RUNS;
    case "get_run_log":
      return [];
    case "start_agent_review":
    case "reply_to_comment":
    case "mention_agent":
    case "start_conflict_fix":
    case "start_conflict_resolution":
      return "run-e2e";
    case "conflict_resolution_state":
      return { unresolved: [], ready: false };
    default:
      return undefined;
  }
}

function appCommand(cmd: string, a: Args): unknown {
  switch (cmd) {
    case "get_settings":
      return settings;
    case "update_settings":
      settings = a.settings as Settings;
      return settings;
    case "file_config_info":
      return null;
    case "list_github_owners":
      return { viewer: VIEWER, orgs: ["acme"] };
    case "list_github_repos":
      return [{ full_name: REPO_SLUG, private: false, description: "Widgets service" }];
    case "list_collaborators":
      return ["alice", "bob", "carol", VIEWER];
    case "repo_merge_options":
      return { squash: true, merge: true, rebase: false, auto_merge: true };
    case "resolve_workspace":
      return { path: "/tmp/tandem-e2e/widgets", managed: true, exists: false };
    case "list_app_releases":
      return RELEASES;
    case "polish_text":
      return str(a, "text");
    default:
      return undefined;
  }
}

function handle(cmd: string, payload?: InvokeArgs): unknown {
  if (cmd.startsWith("plugin:")) return plugin(cmd);
  const a = asArgs(payload);
  const answer = prCommand(cmd, a) ?? reviewCommand(cmd, a) ?? appCommand(cmd, a);
  if (answer !== undefined) return answer;
  // Writes (post, merge, approve, delete, …) succeed with no payload.
  return null;
}

/** Install the mock before the app renders. */
export function installIpcMocks(): void {
  mockWindows("main");
  mockIPC(handle, { shouldMockEvents: true });
}
