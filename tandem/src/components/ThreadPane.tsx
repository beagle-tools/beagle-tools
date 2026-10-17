// One thread, floating over whatever you were doing. Opened from the
// agent runs dashboard: a finished thread reply is something you read
// and answer, not a reason to lose your place in another PR.

import { create } from "zustand";
import { ChevronLeft, ChevronRight, ExternalLink, GitPullRequest } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

import { ipc } from "../lib/ipc";
import { openExternal, prUrl } from "../lib/open";
import type { FileDiff, LocalComment } from "../lib/types";
import { CodeExcerpt } from "./CodeExcerpt";
import { CommentThread, groupThreads } from "./comments";
import { useHighlight } from "../state/notifications";
import { useAppStore } from "../state/store";
import { Button, Modal, Spinner } from "./ui";

interface Target {
  repo: string;
  number: number;
  commentId: string;
}

interface ThreadPaneStore {
  target: Target | null;
  open: (target: Target) => void;
  /** Move to another thread on the same PR, keeping the pane open. */
  show: (commentId: string) => void;
  close: () => void;
}

export const useThreadPane = create<ThreadPaneStore>((set) => ({
  target: null,
  open: (target) => {
    set({ target });
  },
  show: (commentId) => {
    set((s) => (s.target === null ? s : { target: { ...s.target, commentId } }));
  },
  close: () => {
    set({ target: null });
  },
}));

/** The PR's comments and diff, cache-first, refetched only when the PR
 * (not just the thread) changes. `comments` is null while loading. */
function usePrThreadData(
  prKey: string | null,
  repo: string | undefined,
  number: number | undefined,
): { comments: LocalComment[] | null; diff: FileDiff[] } {
  const [comments, setComments] = useState<LocalComment[] | null>(null);
  const [diff, setDiff] = useState<FileDiff[]>([]);
  useEffect(() => {
    if (prKey === null || repo === undefined || number === undefined) return;
    setComments(null);
    let live = true;
    // Cache-first, like everything else — the bundle carries both the
    // thread and the diff the comment is anchored to.
    ipc
      .getPrBundle(repo, number)
      .then((bundle) => {
        if (!live) return;
        setComments(bundle?.comments ?? []);
        setDiff(bundle?.diff ?? []);
      })
      .catch((e: unknown) => {
        console.warn("couldn't load thread", e);
        if (live) setComments([]);
      });
    return () => {
      live = false;
    };
  }, [prKey, repo, number]);
  return { comments, diff };
}

type Thread = ReturnType<typeof groupThreads>[number];

/** Reading order, not insertion order: down the file, top to bottom,
 * with PR-level threads first since they're about the whole thing.
 * Archived threads are left out. */
function readingOrder(comments: LocalComment[]): Thread[] {
  return groupThreads(comments)
    .filter((t) => t.root.status !== "archived")
    .sort(
      (a, b) =>
        a.root.path.localeCompare(b.root.path) ||
        a.root.line - b.root.line ||
        a.root.created_at.localeCompare(b.root.created_at),
    );
}

/** Modal title: PR, file:line, and ←/→ steppers with "n / total". */
function PaneTitle({
  target,
  thread,
  at,
  total,
  step,
}: {
  target: Target;
  thread: Thread | null;
  at: number;
  total: number;
  step: (delta: number) => void;
}): ReactElement {
  return (
    <span className="flex items-center gap-2">
      <span className="font-mono">
        {target.repo}#{target.number}
      </span>
      {thread === null ? (
        <span className="text-muted">thread</span>
      ) : (
        <>
          <span className="truncate font-mono text-muted">
            {thread.root.path || "the PR"}
            {thread.root.line > 0 ? `:${String(thread.root.line)}` : ""}
          </span>
          <span className="ml-auto flex shrink-0 items-center gap-1">
            <NavStep
              title="previous comment (←)"
              disabled={at <= 0}
              onClick={() => {
                step(-1);
              }}
            >
              <ChevronLeft size={13} />
            </NavStep>
            <span className="min-w-12 text-center text-[10px] tabular-nums text-muted">
              {at + 1} / {total}
            </span>
            <NavStep
              title="next comment (→)"
              disabled={at >= total - 1}
              onClick={() => {
                step(1);
              }}
            >
              <ChevronRight size={13} />
            </NavStep>
          </span>
        </>
      )}
    </span>
  );
}

export function ThreadPane(): ReactElement | null {
  const target = useThreadPane((s) => s.target);
  const close = useThreadPane((s) => s.close);
  const openPr = useAppStore((s) => s.openPr);
  const setHighlight = useHighlight((s) => s.set);
  const show = useThreadPane((s) => s.show);
  const endRef = useRef<HTMLDivElement | null>(null);
  // Assigned below once the thread list is known; the key handler calls
  // through it so it can be registered before the early return.
  const stepRef = useRef<(delta: number) => void>(() => undefined);
  // Stepping between threads must not refetch the PR — only opening a
  // different PR does.
  const prKey = target === null ? null : `${target.repo}#${String(target.number)}`;
  const repo = target?.repo;
  const number = target?.number;

  const { comments, diff } = usePrThreadData(prKey, repo, number);

  // The latest reply is what you opened this for; a long thread would
  // otherwise make you scroll past everything you've already read.
  useEffect(() => {
    if (comments === null) return;
    const id = requestAnimationFrame(() => {
      endRef.current?.scrollIntoView({ block: "end" });
    });
    return () => {
      cancelAnimationFrame(id);
    };
  }, [comments, target?.commentId]);

  useEffect(() => {
    if (target === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      const el = e.target as HTMLElement | null;
      // Arrows belong to the reply box while you're typing in it.
      if (el?.closest("input, textarea, select, [contenteditable='true']")) return;
      e.preventDefault();
      stepRef.current(e.key === "ArrowRight" ? 1 : -1);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [target]);

  if (target === null) return null;

  const threads = comments === null ? [] : readingOrder(comments);
  const at = threads.findIndex(
    (t) => t.root.id === target.commentId || t.replies.some((r) => r.id === target.commentId),
  );
  const thread = at === -1 ? null : (threads[at] ?? null);
  const step = (delta: number) => {
    const next = threads[at + delta];
    if (next) show(next.root.id);
  };
  stepRef.current = step;

  return (
    <Modal
      title={
        <PaneTitle target={target} thread={thread} at={at} total={threads.length} step={step} />
      }
      onClose={close}
    >
      {comments === null ? (
        <Spinner label="loading the thread…" />
      ) : thread === null ? (
        <div className="py-8 text-center text-xs text-muted">
          that thread is gone — it may have been deleted, or the PR&apos;s data purged after it
          merged
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <CodeExcerpt
            diff={diff}
            path={thread.root.path}
            line={thread.root.line}
            side={thread.root.side}
            repo={thread.root.repo}
            gitRef={thread.root.head_sha}
          />
          <CommentThread root={thread.root} replies={thread.replies} />
          <div ref={endRef} />
          <div className="flex justify-end gap-2 border-t border-edge/60 pt-2">
            <Button
              kind="primary"
              onClick={() => {
                // Same landing as a click from the runs dashboard: the
                // PR, scrolled to this thread.
                close();
                void openPr(target.repo, target.number);
                setHighlight({ commentId: thread.root.id });
              }}
              title="open this PR in Tandem, scrolled to the thread"
            >
              <GitPullRequest size={12} /> open PR in Tandem
            </Button>
            <Button
              onClick={() => {
                openExternal(prUrl(thread.root.repo, thread.root.pr_number));
              }}
              title="open the pull request on github.com"
            >
              <ExternalLink size={12} /> github
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

function NavStep({
  title,
  disabled,
  onClick,
  children,
}: {
  title: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className="inline-flex size-6 items-center justify-center rounded-md text-muted transition-colors hover:bg-panel-2 hover:text-cream disabled:pointer-events-none disabled:opacity-30"
    >
      {children}
    </button>
  );
}
