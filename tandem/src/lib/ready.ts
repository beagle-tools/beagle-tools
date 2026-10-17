// The "approved and green" signal. mergeable_state alone is useless on
// merge-queue repos (always "blocked"), so readiness prefers GitHub's
// reviewDecision + status-check rollup when present.

import type { PullRequest } from "./types";

export function isReadyToMerge(pr: PullRequest): boolean {
  if (pr.draft) return false;
  if (pr.mergeable_state === "dirty") return false;
  // Unresolved review threads gate merging in most orgs — not ready.
  if (pr.unresolved_threads > 0) return false;
  if (pr.review_decision !== null || pr.checks_state !== null) {
    const approved = (pr.review_decision ?? "APPROVED") === "APPROVED";
    // A null rollup means no check suites on the head commit — GitHub
    // reports that even on repos whose "checks" live elsewhere. Only an
    // actual non-success rollup blocks readiness.
    const green = pr.checks_state === "SUCCESS" || pr.checks_state === null;
    return approved && green;
  }
  return pr.mergeable_state === "clean";
}

export function checksFailing(pr: PullRequest): boolean {
  return pr.checks_state === "FAILURE" || pr.checks_state === "ERROR";
}

/** Merge conflicts with the base branch — a harder blocker than "just
 * needs approval," so it gets the same red treatment as failing checks
 * rather than reading as a routine amber "awaiting review." */
export function hasMergeConflicts(pr: PullRequest): boolean {
  return pr.mergeable_state === "dirty";
}

/** Left-edge accent: green = ready to merge, red = checks failing or
 * merge conflicts. */
export function prEdgeClass(pr: PullRequest): string {
  if (isReadyToMerge(pr)) return "border-l-4 border-l-moss ";
  if (checksFailing(pr) || hasMergeConflicts(pr)) return "border-l-4 border-l-ember ";
  return "";
}
