// Repo-selection and PR-list-loading store actions, split out of
// store.ts to honor the 400-line rule. Spread into the store object at
// creation.

import { ipc } from "../lib/ipc";
import { sanitizePrSort } from "../lib/sort";
import { runPool } from "../lib/pool";
import type { AppStore } from "./storeTypes";

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

// Bumped on every selectRepo() call; a call only applies its results if
// its token is still the latest one by the time they land, so switching
// repos mid-load can't have a stale response stomp the current repo's
// state.
let selectRepoToken = 0;

// How many repos load at once for "all repositories".
const ALL_REPOS_CONCURRENCY = 5;

export function repoActions(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
): Pick<AppStore, "selectRepo" | "refreshPrs" | "loadMorePrs" | "loadOrgRepos"> {
  return {
    selectRepo: async (slug: string, presetPr?: number) => {
      const token = ++selectRepoToken;
      const current = () => token === selectRepoToken;

      set({
        selectedRepo: slug,
        // openPr() already knows which PR it's headed to — keeping it
        // set (instead of null) through the repo switch means the main
        // pane reads "loading pull request…" the whole time instead of
        // flashing "pick a pull request" until selectPr() catches up.
        selectedPr: presetPr ?? null,
        bundle: null,
        prs: [],
        prPage: 1,
        // Re-seed the ordering from the saved default.
        prSort: sanitizePrSort(get().settings?.pr_sort ?? "opened-desc"),
        collaborators: [],
        allRepoProgress: null,
      });
      // "*" = all repositories: aggregate open PRs across every repo in
      // tracked orgs, ALL_REPOS_CONCURRENCY at a time; inbox tabs search
      // account-wide.
      if (slug === "*") {
        const repos = get().orgRepos.map((r) => r.full_name);
        if (repos.length === 0) return;
        set({ allRepoProgress: { done: 0, total: repos.length } });
        try {
          const pages = await runPool(
            repos,
            ALL_REPOS_CONCURRENCY,
            (r) => ipc.syncPullRequests(r, 1).catch(() => null),
            (done, total) => {
              if (current()) set({ allRepoProgress: { done, total } });
            },
            current,
          );
          if (!current()) return;
          set({
            prs: pages.filter((p) => p !== null).flatMap((p) => p.prs),
            prHasMore: false,
            prPage: 1,
            allRepoProgress: null,
          });
        } catch (e) {
          if (current()) fail(e);
        } finally {
          if (current()) set({ allRepoProgress: null });
        }
        return;
      }
      // People autocomplete for @mentions; quiet failure (needs perms).
      ipc
        .listCollaborators(slug)
        .then((collaborators) => {
          if (current()) set({ collaborators });
        })
        .catch(console.warn);
      try {
        const cached = await ipc.getPullRequests(slug);
        if (!current()) return;
        set({ prs: cached });
        const page = await ipc.syncPullRequests(slug, 1);
        if (!current()) return;
        set({ prs: page.prs, prHasMore: page.has_more, prPage: 1 });
      } catch (e) {
        if (current()) fail(e);
      }
    },

    refreshPrs: async () => {
      const repo = get().selectedRepo;
      if (!repo) return;
      try {
        const page = await ipc.syncPullRequests(repo, 1);
        set({ prs: page.prs, prHasMore: page.has_more, prPage: 1 });
      } catch (e) {
        fail(e);
      }
    },

    loadMorePrs: async () => {
      const { selectedRepo, prPage, prs } = get();
      if (!selectedRepo) return;
      try {
        const next = prPage + 1;
        const page = await ipc.syncPullRequests(selectedRepo, next);
        set({ prs: [...prs, ...page.prs], prHasMore: page.has_more, prPage: next });
      } catch (e) {
        fail(e);
      }
    },

    // Refetches every repo across tracked orgs — feeds the repo picker
    // and "all repositories". `viewer` selects the /user/repos endpoint
    // (private repos included) instead of the /orgs/:org/repos one.
    loadOrgRepos: async () => {
      const orgs = get().settings?.orgs ?? [];
      if (orgs.length === 0) {
        set({ orgRepos: [] });
        return;
      }
      const viewer = get().viewer;
      try {
        const lists = await runPool(orgs, ALL_REPOS_CONCURRENCY, (org) =>
          ipc.listGithubRepos(org, org === viewer).catch(() => []),
        );
        set({ orgRepos: lists.flat() });
      } catch (e) {
        fail(e);
      }
    },
  };
}
