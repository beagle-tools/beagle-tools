// Pushing a conflict resolution, in two steps: `review` loads the diff
// the push will commit and opens the confirmation; `push` (its button)
// commits and pushes. Nothing leaves the machine without the diff shown.

import { useState } from "react";

import { ipc } from "../lib/ipc";
import { pushGithubError, pushInfo } from "./toasts";

export interface ConflictPush {
  /** null when the confirmation is closed, else its diff (null while loading). */
  review: { diff: string | null } | null;
  pushing: boolean;
  openReview: (repo: string, number: number) => void;
  push: (repo: string, number: number) => void;
  close: () => void;
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e));

export function useConflictPush(): ConflictPush {
  const [review, setReview] = useState<{ diff: string | null } | null>(null);
  const [pushing, setPushing] = useState(false);

  const openReview = (repo: string, number: number) => {
    setReview({ diff: null });
    ipc
      .conflictResolutionDiff(repo, number)
      .then((diff) => {
        setReview({ diff });
      })
      .catch((e: unknown) => {
        setReview(null);
        pushGithubError(message(e));
      });
  };

  const push = (repo: string, number: number) => {
    setPushing(true);
    ipc
      .pushConflictResolution(repo, number)
      .then((sha) => {
        setReview(null);
        pushInfo(`pushed the resolution (${sha.slice(0, 7)})`);
      })
      .catch((e: unknown) => {
        pushGithubError(message(e));
      })
      .finally(() => {
        setPushing(false);
      });
  };

  const close = () => {
    setReview(null);
  };

  return { review, pushing, openReview, push, close };
}
