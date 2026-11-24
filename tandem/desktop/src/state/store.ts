// Global app state (zustand). Components read slices; all IPC flows
// through the actions here so loading/sync state stays consistent.

import { create } from "zustand";

import { ipc } from "../lib/ipc";

import { pushGithubError } from "./toasts";
import { agentActions } from "./agentActions";
import { githubActions } from "./githubActions";
import { repoActions } from "./repoActions";
import { inboxActions } from "./inboxActions";
import { initAction } from "./initAction";
import { commentActions } from "./commentActions";
import { applyTheme, loadPinned, loadTheme, loadViewer } from "./persist";
import type { AppStore } from "./storeTypes";

export type { Theme, View } from "./storeTypes";

export const useAppStore = create<AppStore>((set, get) => {
  const fail = (e: unknown) => {
    pushGithubError(e instanceof Error ? e.message : String(e));
  };

  const reloadComments = async () => {
    const { selectedRepo, selectedPr, bundle } = get();
    if (!selectedRepo || selectedPr === null || !bundle) return;
    const comments = await ipc.listLocalComments(selectedRepo, selectedPr);
    set({ bundle: { ...bundle, comments } });
  };

  // A merged PR is done with: once you navigate away, drop everything
  // cached for it so the app stops carrying finished work. Fire and
  // forget — nothing on screen depends on it finishing.
  const forgetIfMerged = () => {
    const { selectedRepo, selectedPr, bundle } = get();
    if (!selectedRepo || selectedPr === null) return;
    if (bundle?.detail.pull_request.state !== "merged") return;
    void ipc.forgetPr(selectedRepo, selectedPr).catch(console.warn);
  };

  const reloadRuns = async () => {
    const { selectedRepo, selectedPr } = get();
    if (!selectedRepo || selectedPr === null) return;
    set({ runs: await ipc.listAgentRuns(selectedRepo, selectedPr) });
  };

  return {
    view: "review-status",
    theme: loadTheme(),
    rightPinned: loadPinned("tandem-pin-right"),
    settings: null,
    selectedRepo: null,
    prs: [],
    selectedPr: null,
    bundle: null,
    agentSpecs: [],
    runs: [],
    agentEvents: [],
    syncing: {},
    prHasMore: false,
    prPage: 1,
    inbox: {},
    prSort: "opened-desc",
    viewer: loadViewer(),
    collaborators: [],
    allRepoProgress: null,
    orgRepos: [],
    reviewRequests: [],
    reviewing: [],
    reviewRequestsSeeded: false,

    setView: (view) => {
      if (view !== "review") forgetIfMerged();
      set({ view });
    },

    setPrSort: (sort) => {
      set({ prSort: sort });
    },

    openPr: async (repoSlug, number) => {
      // Opening a PR always means "show the review page" — callers on
      // another view (review-status, settings, agents) shouldn't have
      // to remember to switch back themselves.
      set({ view: "review" });
      if (get().selectedRepo !== repoSlug) {
        await get().selectRepo(repoSlug, number);
      }
      await get().selectPr(number);
    },

    toggleTheme: () => {
      const theme = get().theme === "dark" ? "light" : "dark";
      applyTheme(theme);
      set({ theme });
    },

    goHome: () => {
      // Review status is home; the open PR is dropped on the way so the
      // review view doesn't linger behind it.
      forgetIfMerged();
      set({ view: "review-status", selectedPr: null, bundle: null });
    },

    toggleRightPane: () => {
      const value = !get().rightPinned;
      localStorage.setItem("tandem-pin-right", String(value));
      set({ rightPinned: value });
    },

    selectPr: async (number) => {
      const repo = get().selectedRepo;
      if (!repo) return;
      if (get().selectedPr !== number) forgetIfMerged();
      set({ selectedPr: number, bundle: null, agentEvents: [] });
      try {
        const cached = await ipc.getPrBundle(repo, number);
        if (cached) set({ bundle: cached });
        await reloadRuns();
        const fresh = await ipc.syncPrBundle(repo, number);
        set({ bundle: fresh });
      } catch (e) {
        fail(e);
      }
    },

    refreshBundle: async () => {
      const { selectedRepo, selectedPr } = get();
      if (!selectedRepo || selectedPr === null) return;
      try {
        set({ bundle: await ipc.syncPrBundle(selectedRepo, selectedPr) });
      } catch (e) {
        fail(e);
      }
    },

    ...initAction(set, get, fail, reloadComments, reloadRuns),
    ...commentActions(set, get, fail, reloadComments),
    ...agentActions(set, get, fail, reloadComments, reloadRuns),
    ...githubActions(set, get, fail, reloadComments),
    ...repoActions(set, get, fail),
    ...inboxActions(set, get, fail),
  };
});
