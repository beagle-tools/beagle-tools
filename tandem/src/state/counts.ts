// One count, used by the tabs and by the navbar badge. They disagreed
// because each did its own arithmetic: the badge counted every review
// request including bots and ignored "mine", while the tabs filtered.
// Whatever the lists show, this is what the badge says.

import { useMemo } from "react";

import { isBotLogin } from "../lib/bots";
import type { PullRequest } from "../lib/types";
import { useAppStore } from "./store";

const key = (pr: PullRequest) => `${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`;

export interface QueueCounts {
  /** Review requests from people, bots excluded. */
  requested: number;
  /** Reviewed without being asked, not yet approved by you. */
  reviewing: number;
  authored: number;
  mentions: number;
  /** What the navbar badge shows: everything waiting on you. */
  total: number;
}

export function useQueueCounts(): QueueCounts {
  const reviewRequests = useAppStore((s) => s.reviewRequests);
  const reviewing = useAppStore((s) => s.reviewing);
  const inbox = useAppStore((s) => s.inbox);

  return useMemo(() => {
    const approved = new Set((inbox.approved ?? []).map(key));
    const requested = reviewRequests.filter((pr) => !isBotLogin(pr.author.login)).length;
    const unapproved = reviewing.filter((pr) => !approved.has(key(pr))).length;
    // "mine"/"mentions" load lazily, so they count zero until their tab
    // has been opened — a low badge beats a wrong one.
    const authored = inbox.authored?.length ?? 0;
    const mentions = inbox.mentions?.length ?? 0;
    return {
      requested,
      reviewing: unapproved,
      authored,
      mentions,
      total: requested + unapproved + authored,
    };
  }, [reviewRequests, reviewing, inbox]);
}
