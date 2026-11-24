// Client-side ordering for PR lists (open tab + inbox tabs).

import type { PullRequest } from "./types";

export type PrSort = "opened-asc" | "opened-desc" | "updated-desc" | "number-asc" | "number-desc";

export const PR_SORTS: [PrSort, string][] = [
  ["opened-asc", "oldest opened"],
  ["opened-desc", "newest opened"],
  ["updated-desc", "recently updated"],
  ["number-asc", "number ↑"],
  ["number-desc", "number ↓"],
];

export function sortPrs(prs: PullRequest[], sort: PrSort): PullRequest[] {
  const sorted = [...prs];
  switch (sort) {
    case "opened-asc":
      return sorted.sort((a, b) => a.created_at.localeCompare(b.created_at));
    case "opened-desc":
      return sorted.sort((a, b) => b.created_at.localeCompare(a.created_at));
    case "updated-desc":
      return sorted.sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    case "number-asc":
      return sorted.sort((a, b) => a.number - b.number);
    case "number-desc":
      return sorted.sort((a, b) => b.number - a.number);
  }
}

export function sanitizePrSort(sort: string): PrSort {
  const valid: PrSort[] = [
    "opened-asc",
    "opened-desc",
    "updated-desc",
    "number-asc",
    "number-desc",
  ];
  return valid.includes(sort as PrSort) ? (sort as PrSort) : "opened-desc";
}
