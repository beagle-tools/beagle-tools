// The home page: account-wide review status across every tracked org,
// never scoped to one repo. "req" is kept fresh by a standing timer
// (state/repoActions.ts refreshReviewRequests) that also toasts new
// arrivals; "mine" and "mentions" load lazily on first visit; "find"
// browses any repo's open PRs. Clicking a row opens it in the review
// view, switching the repo to match.

import { AtSign, Bot, Inbox, RefreshCw, Search, User, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactElement } from "react";

import { isBotLogin } from "../lib/bots";
import { fuzzyScore } from "../lib/fuzzy";
import { sortPrs } from "../lib/sort";
import type { InboxScope, PullRequest } from "../lib/types";
import type { ReviewTab } from "../state/keyNav";
import { useKeyNav } from "../state/keyNav";
import { useReviewedKeys } from "../state/runBoard";
import { useQueueCounts } from "../state/counts";
import { useAppStore } from "../state/store";
import { FindPrTab } from "./FindPrTab";
import { ApprovedFooter, InboxRow, ReadyGroupedRows, rowKey } from "./PrRows";
import { QuickReviewControl } from "./QuickReviewControl";
import { Button, CountBadge, Spinner } from "./ui";

const TABS: { id: ReviewTab; label: string; icon: typeof Inbox }[] = [
  { id: "requested", label: "req", icon: Inbox },
  { id: "authored", label: "mine", icon: User },
  { id: "mentions", label: "mentions", icon: AtSign },
  { id: "find", label: "find PR", icon: Search },
];

export function ReviewStatusView(): ReactElement {
  const tab = useKeyNav((s) => s.tab);
  const setTab = useKeyNav((s) => s.setTab);
  const reviewRequests = useAppStore((s) => s.reviewRequests);
  const reviewRequestsSeeded = useAppStore((s) => s.reviewRequestsSeeded);
  const prSort = useAppStore((s) => s.prSort);
  const loadQueues = useAppStore((s) => s.loadQueues);
  const reviewing = useAppStore((s) => s.reviewing);
  // What you've already approved (the footer's list, loaded on this
  // same tab) — an approved PR has moved on from "I reviewed this".
  const approved = useAppStore((s) => s.inbox.approved);
  // "mine"/"mentions" live in the store's account-wide inbox map so the
  // `r` shortcut can refresh whichever one is on screen.
  // These load with everything else now; the tab only chooses which
  // already-loaded list to show.
  const scope: InboxScope | null = tab === "authored" || tab === "mentions" ? tab : null;
  const scopeList = useAppStore((s) => (scope === null ? undefined : s.inbox[scope]));

  // Every list, in parallel, whichever tab is showing — switching tabs
  // should never be the thing that starts a fetch.
  useEffect(() => {
    void loadQueues();
  }, [loadQueues]);

  const queue = useQueueCounts();
  const tabCount = (id: ReviewTab): number | undefined => {
    // Bots are excluded on purpose: a count is "things waiting on you",
    // and a dependency bump isn't that.
    if (id === "requested") return queue.requested + queue.reviewing;
    if (id === "authored") return queue.authored;
    if (id === "mentions") return queue.mentions;
    return undefined;
  };
  const [filter, setFilter] = useState("");
  const reviewedKeys = useReviewedKeys();
  // An agent reviewed it and then nothing happened — not approved, not
  // merged. That review is sitting there unused, so it goes first.
  const unacted = useCallback(
    (pr: PullRequest) =>
      reviewedKeys.has(`${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`) &&
      pr.state === "open" &&
      pr.review_decision !== "approved",
    [reviewedKeys],
  );
  const all = sortPrs(tab === "requested" ? reviewRequests : (scopeList ?? []), prSort);
  const prs = useMemo(() => matching(all, filter), [all, filter]);
  const unapproved = useMemo(() => {
    const done = new Set(
      (approved ?? []).map((pr) => `${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`),
    );
    return reviewing.filter(
      (pr) => !done.has(`${pr.repo.owner}/${pr.repo.name}#${String(pr.number)}`),
    );
  }, [reviewing, approved]);
  const shownReviewing = useMemo(() => matching(unapproved, filter), [unapproved, filter]);
  const loading =
    tab === "requested" ? !reviewRequestsSeeded : scope !== null && scopeList === undefined;

  // "mine" reorders into ready/open/draft groups and publishes its own
  // keyNav list; "find" publishes its own too.
  // Bots open a steady stream that would drown the human queue, so
  // they get their own collapsed section near the bottom.
  const bots = useMemo(() => prs.filter((pr) => isBotLogin(pr.author.login)), [prs]);
  const mine = useMemo(() => prs.filter((pr) => !isBotLogin(pr.author.login)), [prs]);
  // Requested splits into "reviewed, nothing done" then the rest; mine
  // lets ReadyGroupedRows do the same at the top of its own grouping.
  const waiting = useMemo(() => mine.filter(unacted), [mine, unacted]);
  const rest = useMemo(() => mine.filter((pr) => !unacted(pr)), [mine, unacted]);
  const navPrs = useMemo(
    () => (tab === "requested" ? [...waiting, ...rest, ...shownReviewing, ...bots] : prs),
    [tab, waiting, rest, prs, shownReviewing, bots],
  );
  useEffect(() => {
    if (tab !== "requested" && tab !== "mentions") return;
    useKeyNav
      .getState()
      .setList(
        navPrs.map((pr) => ({ slug: `${pr.repo.owner}/${pr.repo.name}`, number: pr.number })),
      );
  }, [tab, navPrs]);

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-3 p-6">
      <ReviewStatusHeader showRefresh={tab !== "find"} loadQueues={loadQueues} />

      <ReviewTabs tab={tab} setTab={setTab} tabCount={tabCount} />

      {tab === "find" ? null : (
        <PrFilter
          value={filter}
          onChange={setFilter}
          shown={prs.length + (tab === "requested" ? shownReviewing.length : 0)}
          total={all.length + (tab === "requested" ? unapproved.length : 0)}
        />
      )}

      {tab === "find" ? (
        <FindPrTab />
      ) : loading ? (
        <div className="px-3 py-8">
          <Spinner label="searching github…" />
        </div>
      ) : mine.length === 0 &&
        !(tab === "requested" && (shownReviewing.length > 0 || bots.length > 0)) ? (
        <div className="flex flex-col items-center gap-2 py-16 text-center text-sm text-muted">
          <Inbox size={28} strokeWidth={1.5} />
          {filter.trim()
            ? `nothing matches “${filter.trim()}”`
            : tab === "requested"
              ? "nothing to review — all clear"
              : "nothing here"}
        </div>
      ) : tab === "authored" ? (
        <ReadyGroupedRows
          prs={prs}
          unacted={unacted}
          renderAside={(pr) => <QuickReviewControl pr={pr} />}
        />
      ) : (
        <div className="space-y-1.5">
          {waiting.length > 0 ? (
            <>
              <div className="px-1 text-[10px] font-medium uppercase tracking-wide text-amber">
                reviewed · nothing done with it yet ({waiting.length})
              </div>
              {waiting.map((pr) => (
                <InboxRow key={rowKey(pr)} pr={pr} aside={<QuickReviewControl pr={pr} />} />
              ))}
              <div className="mx-3 my-2 border-t border-edge/70" />
            </>
          ) : null}
          {rest.map((pr) => (
            <InboxRow key={rowKey(pr)} pr={pr} aside={<QuickReviewControl pr={pr} />} />
          ))}
        </div>
      )}

      {tab === "requested" && shownReviewing.length > 0 ? (
        <ReviewingRows prs={shownReviewing} />
      ) : null}

      {tab === "requested" && bots.length > 0 ? <BotRows prs={bots} /> : null}

      {tab === "requested" && !loading ? <ApprovedFooter /> : null}
    </div>
  );
}

/** Fuzzy-match a list against the filter box: repo, number, title and
 * author all count, so "myr 1295" or "observability" both land. */
function matching(prs: PullRequest[], filter: string): PullRequest[] {
  const query = filter.trim();
  if (!query) return prs;
  return prs
    .map((pr) => ({
      pr,
      rank: fuzzyScore(
        query,
        `${pr.repo.owner}/${pr.repo.name} #${String(pr.number)} ${pr.title} ${pr.author.login}`,
      ),
    }))
    .filter((x): x is { pr: PullRequest; rank: number } => x.rank !== null)
    .sort((a, b) => b.rank - a.rank)
    .map((x) => x.pr);
}

/** Filter box under the tabs. `/` focuses it from anywhere on the page,
 * the way list UIs have done since forever; Esc clears and lets go. */
function PrFilter({
  value,
  onChange,
  shown,
  total,
}: {
  value: string;
  onChange: (value: string) => void;
  shown: number;
  total: number;
}) {
  const ref = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
      const el = e.target as HTMLElement | null;
      if (el?.closest("input, textarea, select, [contenteditable='true']")) return;
      e.preventDefault();
      ref.current?.focus();
      ref.current?.select();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <div className="flex h-8 items-center gap-2 rounded-lg border border-edge bg-panel-2 px-2.5 focus-within:border-sky">
      <Search size={12} className="shrink-0 text-muted" />
      <input
        ref={ref}
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          if (value) onChange("");
          else e.currentTarget.blur();
        }}
        placeholder="filter these pull requests — repo, number, title or author (/)"
        className="h-full min-w-0 flex-1 bg-transparent text-xs text-cream outline-none placeholder:text-muted"
      />
      {value.trim() ? (
        <>
          <span className="shrink-0 text-[10px] tabular-nums text-muted">
            {shown} / {total}
          </span>
          <button
            type="button"
            title="clear (esc)"
            onClick={() => {
              onChange("");
            }}
            className="inline-flex size-5 shrink-0 items-center justify-center rounded text-muted transition-colors hover:bg-panel hover:text-cream"
          >
            <X size={11} />
          </button>
        </>
      ) : null}
    </div>
  );
}

/** PRs you reviewed without being asked — kept in sight until they
 * merge, below the ones GitHub actually assigned you. */
function ReviewingRows({ prs }: { prs: PullRequest[] }) {
  return (
    <div className="space-y-1">
      <div className="mx-3 my-2 border-t border-edge/70" />
      <div className="px-1 text-[10px] font-medium uppercase tracking-wide text-muted">
        reviewing · not requested ({prs.length})
      </div>
      {prs.map((pr) => (
        <InboxRow
          key={rowKey(pr)}
          pr={pr}
          note="reviewing"
          aside={<QuickReviewControl pr={pr} />}
        />
      ))}
    </div>
  );
}

/** Bot-authored PRs, folded away by default. They are real work, just
 * never the thing you opened the app for. */
function BotRows({ prs }: { prs: PullRequest[] }) {
  return (
    <details className="border-t border-edge/60 pt-2">
      <summary className="flex cursor-pointer items-center gap-1.5 px-1 py-1 text-[10px] font-medium uppercase tracking-wide text-muted hover:text-cream">
        <Bot size={11} /> from bots ({prs.length})
      </summary>
      <div className="mt-1 space-y-1.5">
        {prs.map((pr) => (
          <InboxRow key={rowKey(pr)} pr={pr} aside={<QuickReviewControl pr={pr} />} />
        ))}
      </div>
    </details>
  );
}

/** Title plus the "refresh everything" button (hidden on "find"). */
function ReviewStatusHeader({
  showRefresh,
  loadQueues,
}: {
  showRefresh: boolean;
  loadQueues: (force?: boolean) => Promise<void>;
}): ReactElement {
  const [refreshing, setRefreshing] = useState(false);
  return (
    <div className="flex items-center justify-between">
      <div>
        <h1 className="text-lg font-semibold text-cream">Review status</h1>
        <p className="text-xs text-muted">every PR across your tracked orgs, account-wide</p>
      </div>
      {showRefresh ? (
        <Button
          disabled={refreshing}
          onClick={() => {
            const done = () => {
              setRefreshing(false);
            };
            setRefreshing(true);
            void loadQueues(true).finally(done);
          }}
        >
          <RefreshCw size={11} className={refreshing ? "animate-spin" : ""} /> refresh
        </Button>
      ) : null}
    </div>
  );
}

/** The tab strip, each tab badged with its waiting count. */
function ReviewTabs({
  tab,
  setTab,
  tabCount,
}: {
  tab: ReviewTab;
  setTab: (tab: ReviewTab) => void;
  tabCount: (id: ReviewTab) => number | undefined;
}): ReactElement {
  return (
    <div className="flex border-b border-edge">
      {TABS.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          type="button"
          onClick={() => {
            setTab(id);
          }}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors ${
            tab === id ? "border-b-2 border-sky text-cream" : "text-muted hover:text-cream"
          }`}
        >
          <Icon size={12} /> {label}
          <CountBadge count={tabCount(id)} />
        </button>
      ))}
    </div>
  );
}
