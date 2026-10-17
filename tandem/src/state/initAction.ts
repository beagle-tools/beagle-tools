// The store's startup action — event listeners, settings, the first
// repo, background refresh — split out of store.ts to honor the 400-line
// rule. Spread into the store object at creation.

import { listen } from "@tauri-apps/api/event";

import { ipc } from "../lib/ipc";
import { sanitizePrSort } from "../lib/sort";
import type { AgentRun, PrSummary, RunEvent, SyncEvent } from "../lib/types";
import { recordRunUpdate } from "./notifications";
import { applyTheme, saveViewer } from "./persist";
import { useRunBoard } from "./runBoard";
import type { AppStore } from "./storeTypes";
import { pushGithubError, pushInfo } from "./toasts";

type Set = (partial: Partial<AppStore> | ((state: AppStore) => Partial<AppStore>)) => void;
type Get = () => AppStore;

// React StrictMode double-invokes effects in dev; without this guard the
// event listeners register twice and every log line shows up duplicated.
let initStarted = false;

/** Keeps the review-status lists warm without hammering GitHub: ticks
 * on a coarse timer and on window focus, but only actually refetches
 * once `refresh_minutes` has elapsed, and never while the window is
 * hidden. Every list here is a GitHub *search*, which is the stingiest
 * rate limit on the API, so the cadence is a user setting (0 = off). */
function startAutoRefresh(get: () => AppStore) {
  const TICK_MS = 30_000;
  let last = Date.now();

  const maybeRefresh = () => {
    const minutes = get().settings?.refresh_minutes ?? 5;
    if (minutes <= 0 || document.hidden) return;
    if (Date.now() - last < minutes * 60_000) return;
    last = Date.now();
    void get().refreshInboxes();
  };

  setInterval(maybeRefresh, TICK_MS);
  window.addEventListener("focus", maybeRefresh);
}

export function initAction(
  set: Set,
  get: Get,
  fail: (e: unknown) => void,
  reloadComments: () => Promise<void>,
  reloadRuns: () => Promise<void>,
): Pick<AppStore, "init"> {
  return {
    init: async () => {
      if (initStarted) return;
      initStarted = true;
      applyTheme(get().theme);
      await listen<SyncEvent>("tandem://sync", (event) => {
        const { key, phase, error } = event.payload;
        set((s) => ({
          syncing: { ...s.syncing, [key]: phase === "started" },
        }));
        if (phase === "error") pushGithubError(error ?? "sync failed");
      });
      await listen<RunEvent>("tandem://agent-event", (event) => {
        set((s) => ({ agentEvents: [...s.agentEvents.slice(-499), event.payload] }));
      });
      await listen("tandem://comments-updated", () => {
        void reloadComments().catch(console.error);
      });
      await listen<AgentRun>("tandem://run-updated", (event) => {
        recordRunUpdate(event.payload);
        useRunBoard.getState().ingest(event.payload);
        void reloadRuns().catch(console.error);
      });
      await listen<PrSummary>("tandem://summary-updated", (event) => {
        const { selectedRepo, selectedPr, bundle } = get();
        const summary = event.payload;
        const slug = `${summary.repo.owner}/${summary.repo.name}`;
        if (!bundle || slug !== selectedRepo || summary.pr_number !== selectedPr) return;
        set({ bundle: { ...bundle, summary } });
      });
      await listen<{ run_id: string }>("tandem://run-deleted", (event) => {
        useRunBoard.getState().remove(event.payload.run_id);
        void reloadRuns().catch(console.error);
      });
      // Seed the agents dashboard + header badge with recent history.
      ipc
        .listAllAgentRuns()
        .then((runs) => {
          useRunBoard.getState().ingestMany(runs);
          // Runs the startup sweep just failed out were killed by the
          // restart that booted this very session — say so.
          const interrupted = runs.filter(
            (r) =>
              r.error?.includes("app restarted") &&
              r.finished_at !== null &&
              Date.now() - new Date(r.finished_at).getTime() < 3 * 60_000,
          ).length;
          if (interrupted > 0) {
            pushInfo(
              `${String(interrupted)} agent run${interrupted > 1 ? "s were" : " was"} interrupted by an app restart`,
            );
          }
        })
        .catch(console.warn);

      try {
        const settings = await ipc.getSettings();
        const agentSpecs = await ipc.listAgentSpecs();
        set({
          settings,
          agentSpecs,
          prSort: sanitizePrSort(settings.pr_sort),
        });
        await get().loadOrgRepos();
        const first = settings.inbox_all_repos ? "*" : settings.default_repo;
        // selectRepo() itself prefetches the review-request badge count;
        // with no repo to select yet, do it directly (auth-dependent, so
        // failures stay quiet).
        if (first) await get().selectRepo(first);
        // Account-wide review-requested feed for the "review status"
        // page — independent of the selected repo, kept warm so it's a
        // real notification source.
        get().refreshReviewRequests().catch(console.warn);
        startAutoRefresh(get);
        // Viewer login powers @mention highlighting and own-PR logic;
        // persisted so it's known instantly on every later launch.
        ipc
          .listGithubOwners()
          .then((o) => {
            saveViewer(o.viewer);
            set({ viewer: o.viewer });
          })
          .catch(console.warn);
      } catch (e) {
        fail(e);
      }
    },
  };
}
