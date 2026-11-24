// The pull-request review view: header, summary, file tree and diff,
// plus the right-hand comments/agents pane. Split out of App.tsx to
// honor the 400-line rule.

import { Bot, MessageSquare, Settings } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { AgentPanel } from "./AgentPanel";
import { CommentsPanel } from "./CommentsPanel";
import { DiffViewer } from "./DiffViewer";
import { FileTreePanel } from "./FileTree";
import { PrHeader } from "./PrHeader";
import { SidePane } from "./SidePane";
import { SummarySection } from "./SummarySection";
import { Button, EmptyState } from "./ui";
import { isWorking } from "../state/runBoard";
import { useAppStore } from "../state/store";

type RightTab = "comments" | "agents";

/** How many agent runs are working on the open PR right now — the same
 * pulsing count the header's agents button shows, so an in-flight run
 * is just as visible from the PR's own right-hand pane. `inline` drops
 * the corner-overlay positioning for use next to a tab label. */
function WorkingBadge({ count, inline }: { count: number; inline?: boolean }) {
  return (
    <span
      className={`flex size-3.5 animate-pulse items-center justify-center rounded-full bg-sky text-[8px] font-bold text-ground ${
        inline ? "" : "absolute -right-1.5 -top-1.5"
      }`}
    >
      {count > 9 ? "9+" : count}
    </span>
  );
}

export function ReviewLayout(): ReactElement {
  const bundle = useAppStore((s) => s.bundle);
  const selectedPr = useAppStore((s) => s.selectedPr);
  const settings = useAppStore((s) => s.settings);
  const setView = useAppStore((s) => s.setView);
  const runs = useAppStore((s) => s.runs);
  const [tab, setTab] = useState<RightTab>("comments");

  const agentsWorking = runs.filter(isWorking).length;
  const commentCount = bundle
    ? bundle.comments.length + bundle.detail.comments.length + bundle.detail.review_bodies.length
    : 0;
  const selectedRepo = useAppStore((s) => s.selectedRepo);
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  const prKey = `${selectedRepo ?? ""}#${String(selectedPr ?? "")}`;
  const prOpen = bundle !== null;

  // While a PR is on screen, quietly poll for new comments/commits/
  // checks — the cached bundle means reopening is instant either way.
  useEffect(() => {
    if (!prOpen) return;
    const timer = setInterval(() => {
      void refreshBundle();
    }, 60_000);
    return () => {
      clearInterval(timer);
    };
  }, [prKey, prOpen, refreshBundle]);

  // First run: no orgs tracked yet — onboard from the main page.
  if (settings?.orgs.length === 0) {
    return (
      <main className="flex-1">
        <EmptyState
          title="welcome aboard"
          hint="track a GitHub org (or yourself) to start reviewing — Tandem reads PRs, checks and comments, and keeps all review notes local"
          action={
            <Button
              kind="primary"
              onClick={() => {
                setView("settings");
              }}
            >
              <Settings size={12} /> set up an organization
            </Button>
          }
        />
      </main>
    );
  }

  return (
    <>
      <main className="flex min-w-0 flex-1 flex-col">
        {bundle ? (
          <>
            <PrHeader detail={bundle.detail} />
            <div className="border-b border-edge px-4 pb-2">
              <SummarySection />
            </div>
            <div className="flex min-h-0 flex-1">
              <FileTreePanel key={prKey} prKey={prKey} />
              <div id="diff-scroll" className="min-h-0 flex-1 overflow-y-auto">
                <DiffViewer
                  diff={bundle.diff}
                  comments={bundle.comments}
                  githubComments={bundle.detail.comments}
                  reviewThreads={bundle.detail.review_threads}
                />
              </div>
            </div>
          </>
        ) : (
          <EmptyState
            title={selectedPr === null ? "pick a pull request" : "loading pull request…"}
            hint={
              selectedPr === null
                ? "open one from review status (⌘K jumps to any PR) — Tandem loads from cache instantly and syncs GitHub in the background"
                : undefined
            }
          />
        )}
      </main>

      {bundle ? (
        <ReviewSidePane
          tab={tab}
          setTab={setTab}
          agentsWorking={agentsWorking}
          commentCount={commentCount}
        />
      ) : null}
    </>
  );
}

/** The right-hand pane for an open PR. The selected tab lives in
 * {@link ReviewLayout} so it survives the pane unmounting while the next
 * PR loads. */
function ReviewSidePane({
  tab,
  setTab,
  agentsWorking,
  commentCount,
}: {
  tab: RightTab;
  setTab: (tab: RightTab) => void;
  agentsWorking: number;
  commentCount: number;
}): ReactElement {
  const rightPinned = useAppStore((s) => s.rightPinned);
  const toggleRightPane = useAppStore((s) => s.toggleRightPane);
  return (
    <SidePane
      side="right"
      pinned={rightPinned}
      onTogglePin={() => {
        toggleRightPane();
      }}
      widthClass="w-80"
      rail={
        <>
          <span className="relative">
            <Bot size={16} />
            {agentsWorking > 0 ? <WorkingBadge count={agentsWorking} /> : null}
          </span>
          {commentCount > 0 ? (
            <span className="rounded-full bg-panel-2 px-1.5 py-0.5 text-[10px] font-semibold text-cream">
              {commentCount}
            </span>
          ) : null}
        </>
      }
    >
      <div className="flex h-full flex-col">
        <div className="flex border-b border-edge">
          {(["comments", "agents"] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => {
                setTab(t);
              }}
              className={`flex flex-1 items-center justify-center gap-1.5 px-3 py-2 text-xs font-medium transition-colors ${
                tab === t ? "border-b-2 border-sky text-cream" : "text-muted hover:text-cream"
              }`}
            >
              {t === "agents" ? <Bot size={13} /> : <MessageSquare size={13} />}
              {t === "agents" ? "agents" : `comments (${String(commentCount)})`}
              {t === "agents" && agentsWorking > 0 ? (
                <WorkingBadge count={agentsWorking} inline />
              ) : null}
            </button>
          ))}
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {tab === "agents" ? <AgentPanel /> : <CommentsPanel />}
        </div>
      </div>
    </SidePane>
  );
}
