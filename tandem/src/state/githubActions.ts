// Explicit GitHub-write store actions, split out of store.ts to honor
// the 400-line rule. Spread into the store object at creation.

import { ipc } from "../lib/ipc";
import type { AppStore } from "./storeTypes";

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

export function githubActions(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
  reloadComments: () => Promise<void>,
): Pick<AppStore, "postToGithub" | "approvePr"> {
  return {
    postToGithub: async (commentId: string) => {
      try {
        await ipc.postCommentToGithub(commentId);
        await reloadComments();
      } catch (e) {
        fail(e);
      }
    },

    approvePr: async (body: string | null) => {
      const { selectedRepo, selectedPr } = get();
      if (!selectedRepo || selectedPr === null) return;
      try {
        await ipc.approvePr(selectedRepo, selectedPr, body);
        set({ bundle: await ipc.syncPrBundle(selectedRepo, selectedPr) });
        // GitHub's review state is eventually consistent — the instant
        // re-sync often still reads the old "awaiting" decision, so
        // sync once more after it settles, and move the PR out of the
        // requested inbox.
        setTimeout(() => {
          const { selectedRepo: r, selectedPr: n } = get();
          if (r === selectedRepo && n === selectedPr) {
            ipc
              .syncPrBundle(r, n)
              .then((bundle) => {
                set({ bundle });
              })
              .catch(console.warn);
          }
        }, 2500);
        get().loadInbox("requested", true).catch(console.warn);
        get().loadInbox("approved", true).catch(console.warn);
      } catch (e) {
        fail(e);
      }
    },
  };
}
