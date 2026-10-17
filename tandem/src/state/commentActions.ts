// Settings and local-comment store actions, split out of store.ts to
// honor the 400-line rule. Spread into the store object at creation.

import { ipc } from "../lib/ipc";
import type { AppStore } from "./storeTypes";

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

export function commentActions(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
  reloadComments: () => Promise<void>,
): Pick<
  AppStore,
  "saveSettings" | "addComment" | "setCommentStatus" | "deleteSummary" | "deleteComment"
> {
  return {
    saveSettings: async (settings) => {
      const prevOrgs = [...(get().settings?.orgs ?? [])].sort();
      try {
        const saved = await ipc.updateSettings(settings);
        set({ settings: saved });
        if (JSON.stringify(prevOrgs) !== JSON.stringify([...saved.orgs].sort())) {
          get().loadOrgRepos().catch(console.warn);
        }
      } catch (e) {
        fail(e);
      }
    },

    addComment: async (comment) => {
      try {
        const created = await ipc.addLocalComment(comment);
        await reloadComments();
        return created;
      } catch (e) {
        fail(e);
        return null;
      }
    },

    setCommentStatus: async (id, status) => {
      const { selectedRepo, selectedPr } = get();
      if (!selectedRepo || selectedPr === null) return;
      try {
        await ipc.setCommentStatus(id, status, selectedRepo, selectedPr);
        await reloadComments();
      } catch (e) {
        fail(e);
      }
    },

    deleteSummary: async () => {
      const { selectedRepo, selectedPr, bundle } = get();
      if (!selectedRepo || selectedPr === null || !bundle) return;
      try {
        await ipc.deletePrSummary(selectedRepo, selectedPr);
        set({ bundle: { ...bundle, summary: null } });
      } catch (e) {
        fail(e);
      }
    },

    deleteComment: async (id) => {
      const { selectedRepo, selectedPr } = get();
      if (!selectedRepo || selectedPr === null) return;
      try {
        await ipc.deleteLocalComment(id, selectedRepo, selectedPr);
        await reloadComments();
      } catch (e) {
        fail(e);
      }
    },
  };
}
