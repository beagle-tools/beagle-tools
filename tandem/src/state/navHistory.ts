// Where you've been, so you can step back out of a detour — the same
// move as Slack's back arrow. A location is just the part of the store
// that says what's on screen; the stack watches the store rather than
// asking every navigation action to remember to record itself.

import { create } from "zustand";

import { useKeyNav } from "./keyNav";
import type { ReviewTab } from "./keyNav";
import { useAppStore } from "./store";
import type { View } from "./storeTypes";

/** Deliberately shallow: five steps each way covers "I wandered off,
 * take me back" without turning into a history nobody prunes. */
const MAX_DEPTH = 5;

interface Location {
  view: View;
  repo: string | null;
  pr: number | null;
  tab: ReviewTab;
}

function current(): Location {
  const { view, selectedRepo, selectedPr } = useAppStore.getState();
  return { view, repo: selectedRepo, pr: selectedPr, tab: useKeyNav.getState().tab };
}

function same(a: Location, b: Location): boolean {
  return a.view === b.view && a.repo === b.repo && a.pr === b.pr && a.tab === b.tab;
}

interface NavHistory {
  past: Location[];
  future: Location[];
}

const useHistory = create<NavHistory>(() => ({ past: [], future: [] }));

/** True while a step is being applied — the store changes it makes are
 * the step itself, not a new place worth remembering. */
let stepping = false;

let previous = current();

/** A move you made yourself: the place you left becomes history, and
 * whatever you'd stepped back from is no longer ahead of you. */
function record(): void {
  const now = current();
  if (stepping || same(now, previous)) return;
  const left = previous;
  useHistory.setState((s) => ({ past: [...s.past, left].slice(-MAX_DEPTH), future: [] }));
  previous = now;
}

useAppStore.subscribe(record);
useKeyNav.subscribe(record);

export function useCanGoBack(): boolean {
  return useHistory((s) => s.past.length > 0);
}

export function useCanGoForward(): boolean {
  return useHistory((s) => s.future.length > 0);
}

export async function goBack(): Promise<void> {
  await step("back");
}

export async function goForward(): Promise<void> {
  await step("forward");
}

/** Pop one side of the stack onto the other, with where you are now in
 * between — the same shape as a browser's back/forward. */
async function step(direction: "back" | "forward"): Promise<void> {
  const { past, future } = useHistory.getState();
  const from = direction === "back" ? past : future;
  const target = from.at(-1);
  if (target === undefined) return;
  const here = current();
  useHistory.setState((s) =>
    direction === "back"
      ? { past: s.past.slice(0, -1), future: [...s.future, here].slice(-MAX_DEPTH) }
      : { future: s.future.slice(0, -1), past: [...s.past, here].slice(-MAX_DEPTH) },
  );

  stepping = true;
  try {
    useKeyNav.getState().setTab(target.tab);
    if (target.pr !== null && target.repo !== null) {
      // openPr sets the repo, loads the PR and switches to the review
      // view in one go — the same path a click down a list takes.
      await useAppStore.getState().openPr(target.repo, target.pr);
    }
    useAppStore.getState().setView(target.view);
  } finally {
    stepping = false;
    previous = current();
  }
}
