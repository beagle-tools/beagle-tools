// Review-status store actions (account-wide inbox lists and the
// review-request feed), split out of store.ts to honor the 400-line rule.
// Spread into the store object at creation.

import { ipc } from "../lib/ipc";
import type { InboxScope, PullRequest } from "../lib/types";
import { withRateLimitRetry } from "./rateLimitRetry";
import type { AppStore } from "./storeTypes";
import { pushInfo } from "./toasts";

function reviewRequestKey(pr: PullRequest): string {
  return `${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`;
}

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

export function inboxActions(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
): Pick<
  AppStore,
  "loadInbox" | "loadQueues" | "refreshInboxes" | "refreshReviewRequests" | "loadReviewing"
> {
  return {
    // Account-wide, like everything else on the review-status page —
    // these lists are never scoped to the selected repo.
    loadInbox: async (scope, force) => {
      if (!force && get().inbox[scope]) return;
      try {
        // Deliberately does NOT clear the current list first: the
        // background refresh would otherwise flash a spinner over a
        // perfectly good list every few minutes.
        const prs = await withRateLimitRetry(() => ipc.listMyPrs(scope, null));
        set((s) => ({ inbox: { ...s.inbox, [scope]: prs } }));
      } catch (e) {
        fail(e);
      }
    },

    // Everything the review-status page shows, loaded together so no
    // tab is empty when you reach it. Firing these in parallel can trip
    // the rate limit, which is why each one waits and retries rather
    // than failing — see state/rateLimitRetry.ts.
    loadQueues: async (force) => {
      await Promise.all([
        get()
          .refreshReviewRequests()
          .then(() => get().loadReviewing()),
        get().loadInbox("authored", force),
        get().loadInbox("mentions", force),
        get().loadInbox("approved", force),
      ]);
    },

    // One pass over everything the review-status page shows: the
    // review-request feed plus whichever inbox lists have been opened.
    refreshInboxes: async () => {
      await get().refreshReviewRequests();
      for (const scope of Object.keys(get().inbox) as InboxScope[]) {
        if (get().inbox[scope] !== undefined) await get().loadInbox(scope, true);
      }
    },

    // Account-wide "review requested", independent of whatever repo is
    // selected (unlike loadInbox("requested"), which scopes to it).
    // Toasts for anything new since the last refresh — but not on the
    // very first load, which would spam one toast per already-pending
    // request on every app start.
    refreshReviewRequests: async () => {
      try {
        const prs = await withRateLimitRetry(() => ipc.listMyPrs("requested", null));
        if (get().reviewRequestsSeeded) {
          const known = new Set(get().reviewRequests.map(reviewRequestKey));
          const fresh = prs.filter((pr) => !known.has(reviewRequestKey(pr)));
          const first = fresh[0];
          if (fresh.length === 1 && first) {
            pushInfo(`Review requested: ${reviewRequestKey(first)} — ${first.title}`);
          } else if (fresh.length > 1) {
            pushInfo(`${String(fresh.length)} new review requests`);
          }
        }
        set({ reviewRequests: prs, reviewRequestsSeeded: true });
      } catch (e) {
        fail(e);
      }
    },

    loadReviewing: async () => {
      try {
        const reviewed = await withRateLimitRetry(() => ipc.listReviewedPrs());
        // Anything GitHub is already asking for belongs in the main
        // list, not the tail — it's one or the other, never both.
        const requested = new Set(get().reviewRequests.map(reviewRequestKey));
        // Your own PRs are not "PRs you reviewed without being asked";
        // they live in mine, where they already lead if a review ran.
        const viewer = get().viewer;
        set({
          reviewing: reviewed.filter(
            (pr) => !requested.has(reviewRequestKey(pr)) && pr.author.login !== viewer,
          ),
        });
      } catch (e) {
        fail(e);
      }
    },
  };
}
