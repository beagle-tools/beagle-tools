// The only place app state lives. Components read from here and call
// actions here; none of them talk to ipc.ts directly.
//
// Refresh is event-driven, not timed: the Rust poller watches the SQLite
// file and emits `agentdesk://runs-updated` only when something actually
// changed, so an idle machine does no work at all.

import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import { errorMessage, ipc } from "../lib/ipc";
import type {
  AgentNode,
  AgentSpec,
  ForestRow,
  Link,
  PipelineSummary,
  Run,
  Transcript,
} from "../lib/types";

interface State {
  runs: Run[];
  jobs: AgentSpec[];
  selectedId: string | null;
  links: Link[];
  log: Transcript | null;
  logFilter: string;
  error: string | null;
  loading: boolean;

  pipelines: PipelineSummary[];
  forest: ForestRow[];
  agents: AgentNode[];
  /** Pin the DAG to one chain — agenttop's `D`. */
  dagFocus: string | null;

  init: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => Promise<void>;
  setLogFilter: (needle: string) => void;
  loadDag: () => Promise<void>;
  setDagFocus: (runId: string | null) => Promise<void>;
  kill: (id: string) => Promise<void>;
  retry: (id: string) => Promise<string | null>;
  trigger: (
    project: string,
    agent: string,
    params: Record<string, string>,
    model: string | null,
  ) => Promise<string | null>;
  dismissError: () => void;
}

export const useStore = create<State>((set, get) => ({
  runs: [],
  jobs: [],
  selectedId: null,
  links: [],
  log: null,
  logFilter: "",
  error: null,
  loading: true,
  pipelines: [],
  forest: [],
  agents: [],
  dagFocus: null,

  init: async () => {
    await get().refresh();
    // Open on the newest run rather than an empty pane. list_runs
    // sorts active first, so this is "whatever is happening now",
    // falling back to the last thing that did.
    const first = get().runs[0];
    if (first && !get().selectedId) await get().select(first.id);
    try {
      set({ jobs: await ipc.listJobs() });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
    set({ loading: false });

    await listen("agentdesk://runs-updated", () => {
      void get().refresh();
      // Keep the open run's panes live too — state, cost and log all
      // move while you are looking at them.
      const id = get().selectedId;
      if (id) void get().select(id);
    });
    // An empty payload clears the banner: the poller sends one when the
    // store starts answering again, so a transient lock does not leave a
    // stale error on screen forever.
    await listen<string>("agentdesk://store-problem", (event) => {
      set({ error: event.payload ? event.payload : null });
    });
  },

  refresh: async () => {
    try {
      // One tick, one set of facts: the jobs, the runs and the forest
      // all come from the same read, so no view can show a run another
      // has not heard of.
      const [runs, forest, pipelines] = await Promise.all([
        ipc.listRuns(),
        ipc.runForest(get().dagFocus),
        ipc.listPipelines(),
      ]);
      set({ runs, forest, pipelines, error: null });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  loadDag: async () => {
    try {
      set({ agents: await ipc.agentGraph() });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  setDagFocus: async (runId) => {
    set({ dagFocus: runId });
    await get().refresh();
  },

  select: async (id) => {
    set({ selectedId: id });
    if (!id) {
      set({ links: [], log: null });
      return;
    }
    try {
      const [links, log] = await Promise.all([ipc.runLinks(id), ipc.readTranscript(id)]);
      // Guard against a slower request for a run the user already left.
      if (get().selectedId === id) set({ links, log });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  setLogFilter: (needle) => {
    set({ logFilter: needle });
  },

  kill: async (id) => {
    try {
      await ipc.killRun(id);
      await get().refresh();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  retry: async (id) => {
    try {
      const newId = await ipc.retryRun(id);
      await get().refresh();
      await get().select(newId);
      return newId;
    } catch (e) {
      set({ error: errorMessage(e) });
      return null;
    }
  },

  trigger: async (project, agent, params, model) => {
    try {
      const id = await ipc.triggerJob(project, agent, params, model);
      await get().refresh();
      return id;
    } catch (e) {
      set({ error: errorMessage(e) });
      return null;
    }
  },

  dismissError: () => {
    set({ error: null });
  },
}));
