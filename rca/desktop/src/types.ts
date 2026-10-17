// The IPC contract with the Rust side. Mirrors
// desktop/src-tauri/src/dto.rs — keep the two in sync when either changes.

export type Severity = "critical" | "high" | "medium" | "low" | "info";

export type Status = "investigating" | "review" | "agent" | "final-review" | "finished";

/** One workspace row for the sidebar. */
export interface Workspace {
  id: string;
  title: string;
  severity: Severity;
  status: Status;
  /** RFC 3339 creation timestamp. */
  created: string;
  systems: string[];
  tags: string[];
  prs: string[];
  archived: boolean;
}

/** A workspace directory that exists on disk but could not load. */
export interface Broken {
  dir_name: string;
  reason: string;
}

/** Everything `list_workspaces` returns in one call. */
export interface Listing {
  root: string;
  workspaces: Workspace[];
  broken: Broken[];
  warnings: string[];
}

// The agents control surface. Mirrors desktop/src-tauri/src/agents.rs.

/** One agent's status. */
export interface AgentStatus {
  id: string;
  enabled: boolean;
  running: boolean;
  last_tick: string | null;
  active_sessions: number;
  last_results: string[];
}

/** A snapshot of the whole beagle-agentd daemon. */
export interface AgentsStatus {
  version: string;
  agents: AgentStatus[];
}

/** The `agents-status` event payload: a snapshot, or an offline reason. */
export interface AgentsEvent {
  connected: boolean;
  status?: AgentsStatus;
  error?: string;
}

// The in-app updater. Mirrors desktop/src-tauri/src/dto.rs's AppRelease.

/** One published rca release for the version list, newest first. */
export interface AppRelease {
  /** `X.Y.Z`, from the tag. */
  version: string;
  /** The full tag, e.g. `beagle-rca-v0.30.0`. */
  tag: string;
  /** RFC 3339 publish time, when known. */
  published_at: string | null;
  /** Whether the release carries an updater manifest (`latest.json`). */
  installable: boolean;
}
