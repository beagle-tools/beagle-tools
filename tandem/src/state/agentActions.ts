// Agent-related store actions, split out of store.ts to honor the
// 400-line rule. Spread into the store object at creation.

import { ipc } from "../lib/ipc";
import type { AgentSpec } from "../lib/types";
import { pushInfo } from "./toasts";
import type { AppStore } from "./storeTypes";

// Starts already in flight, keyed agent+repo+PR. The spawn takes long
// enough that a double click (or a held keyboard shortcut) would
// otherwise launch the same agent on the same PR twice.
const inFlight = new Set<string>();

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

export function agentActions(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
  reloadComments: () => Promise<void>,
  reloadRuns: () => Promise<void>,
): Pick<
  AppStore,
  | "saveAgentSpec"
  | "deleteAgentSpec"
  | "startAgentReview"
  | "cancelRun"
  | "replyToComment"
  | "commitSuggestion"
  | "mentionAgent"
  | "quickStartReview"
> {
  return {
    saveAgentSpec: async (spec: AgentSpec) => {
      try {
        await ipc.saveAgentSpec(spec);
        set({ agentSpecs: await ipc.listAgentSpecs() });
      } catch (e) {
        fail(e);
      }
    },

    deleteAgentSpec: async (name: string) => {
      try {
        await ipc.deleteAgentSpec(name);
        set({ agentSpecs: await ipc.listAgentSpecs() });
      } catch (e) {
        fail(e);
      }
    },

    startAgentReview: async (agentName: string) => {
      const { selectedRepo, selectedPr } = get();
      if (!selectedRepo || selectedPr === null) return;
      const key = `${agentName}@${selectedRepo}#${String(selectedPr)}`;
      // A different agent on the same PR is intent; the same one twice
      // in the same breath is a double click.
      if (inFlight.has(key)) return;
      inFlight.add(key);
      try {
        await ipc.startAgentReview(agentName, selectedRepo, selectedPr);
        await reloadRuns();
      } catch (e) {
        fail(e);
      } finally {
        inFlight.delete(key);
      }
    },

    cancelRun: async (runId: string) => {
      try {
        await ipc.cancelAgentRun(runId);
        await reloadRuns();
      } catch (e) {
        fail(e);
      }
    },

    replyToComment: async (commentId: string, body: string, agentName: string) => {
      try {
        await ipc.replyToComment(agentName, commentId, body);
        await reloadComments();
        await reloadRuns();
      } catch (e) {
        fail(e);
      }
    },

    commitSuggestion: async (commentId: string) => {
      // Deliberately rethrows: a failed commit has to be visible next
      // to the button that failed, not just as a toast that fades.
      const sha = await ipc.commitSuggestion(commentId);
      await reloadComments();
      // The branch advanced; pull the new head + diff.
      await get().refreshBundle();
      return sha;
    },

    mentionAgent: async (agentName: string, commentId: string) => {
      try {
        await ipc.mentionAgent(agentName, commentId);
        await reloadRuns();
      } catch (e) {
        fail(e);
      }
    },

    // Start a review on a PR without navigating to it first (e.g. from
    // the Review Status page). start_agent_review reads the diff from
    // cache rather than syncing it itself, so this syncs the bundle
    // first — the same step selectPr() already does for the normal
    // open-a-PR flow.
    quickStartReview: async (repo: string, number: number, agentName: string) => {
      try {
        await ipc.syncPrBundle(repo, number);
        await ipc.startAgentReview(agentName, repo, number);
        pushInfo(`Started ${agentName} review of ${repo}#${String(number)}`);
        if (get().selectedRepo === repo && get().selectedPr === number) {
          await reloadRuns();
        }
      } catch (e) {
        fail(e);
      }
    },
  };
}
