// The floating panes behind the right-hand comment list. A GitHub
// comment or review opens with two tabs: what was said on GitHub, and
// the discussion you're having about it in Tandem — because the usual
// move is "someone raised this, let me think it through with an agent
// before I answer".

import type { ReactElement, ReactNode } from "react";
import { useState } from "react";

import type { GithubComment, GithubReview, LocalComment } from "../lib/types";
import { useAppStore } from "../state/store";
import { CodeExcerpt } from "./CodeExcerpt";
import { CommentThread, GithubCommentCard } from "./comments";
import { MarkdownBody } from "./Markdown";
import { verdictTone } from "./PanelCards";
import { ReplySection } from "./ReplySection";
import { GithubMark, Modal, Pill } from "./ui";

export interface Thread {
  root: LocalComment;
  replies: LocalComment[];
}

/** Code the comment points at, folded away until asked for. */
function Excerpt({ path, line, endLine }: { path: string; line: number; endLine?: number | null }) {
  const bundle = useAppStore((s) => s.bundle);
  if (!path || line <= 0) return null;
  return (
    <CodeExcerpt
      diff={bundle?.diff ?? []}
      path={path}
      line={line}
      endLine={endLine}
      side="new"
      repo={bundle?.detail.pull_request.repo}
      gitRef={bundle?.detail.pull_request.head_sha}
    />
  );
}

export function LocalThreadModal({
  thread,
  onClose,
}: {
  thread: Thread;
  onClose: () => void;
}): ReactElement {
  return (
    <Modal
      title={
        thread.root.path ? (
          <span className="font-mono text-sky">
            {thread.root.path}:{thread.root.line}
          </span>
        ) : (
          "PR comment (local)"
        )
      }
      onClose={onClose}
    >
      <Excerpt path={thread.root.path} line={thread.root.line} endLine={thread.root.end_line} />
      <div className="mt-3">
        <CommentThread root={thread.root} replies={thread.replies} />
      </div>
    </Modal>
  );
}

/** Shared shell for anything that came from GitHub. The original stays
 * on screen whatever you do — you can't discuss a comment that the UI
 * has swapped out — and the composer stays with it, defaulting to a
 * local reply. The tabs only choose which conversation you're reading:
 * yours in Tandem, or the one on GitHub. */
function GithubShell(props: {
  title: ReactNode;
  original: ReactNode;
  threads: Thread[];
  githubReplies: GithubComment[];
  path: string;
  line: number;
  githubCommentId: number;
  reviewCommentId?: number | undefined;
  onClose: () => void;
}) {
  // Local first: discussing it here before answering is the point.
  const [tab, setTab] = useState<"local" | "github">("local");
  const local = props.threads;

  return (
    <Modal title={props.title} onClose={props.onClose}>
      <Excerpt path={props.path} line={props.line} />
      <div className="mt-3">{props.original}</div>

      <div className="mt-4 flex items-center gap-1.5 border-b border-edge pb-2">
        <Tab
          active={tab === "local"}
          onClick={() => {
            setTab("local");
          }}
        >
          in tandem{local.length > 0 ? ` (${String(local.length)})` : ""}
        </Tab>
        <Tab
          active={tab === "github"}
          onClick={() => {
            setTab("github");
          }}
        >
          <GithubMark size={10} /> on github
          {props.githubReplies.length > 0 ? ` (${String(props.githubReplies.length)})` : ""}
        </Tab>
      </div>

      <div className="mt-3">
        {tab === "local" ? (
          local.length === 0 ? (
            <div className="rounded-lg border border-edge/60 bg-panel-2/40 px-3 py-4 text-center text-xs leading-relaxed text-muted">
              nothing local yet — reply below and it stays in Tandem, where you can @mention an
              agent to think it through before you answer on GitHub
            </div>
          ) : (
            local.map((t) => (
              <div key={t.root.id} className="mb-3 rounded-lg border border-edge bg-panel-2/40 p-3">
                <CommentThread root={t.root} replies={t.replies} />
              </div>
            ))
          )
        ) : props.githubReplies.length === 0 ? (
          <div className="rounded-lg border border-edge/60 bg-panel-2/40 px-3 py-4 text-center text-xs text-muted">
            no replies on GitHub yet
          </div>
        ) : (
          props.githubReplies.map((reply) => (
            <div key={reply.id} className="mb-3">
              <GithubCommentCard comment={reply} />
            </div>
          ))
        )}
      </div>

      <ReplySection
        path={props.path}
        line={props.line}
        githubCommentId={props.githubCommentId}
        reviewCommentId={props.reviewCommentId}
      />
    </Modal>
  );
}

export function GithubCommentModal(props: {
  comment: GithubComment;
  threads: Thread[];
  githubReplies: GithubComment[];
  onClose: () => void;
}): ReactElement {
  const { comment } = props;
  return (
    <GithubShell
      title={
        <span className="inline-flex items-center gap-1.5">
          <GithubMark size={11} />
          {comment.path ? (
            <span className="font-mono text-sky">
              {comment.path}
              {comment.line !== null ? `:${String(comment.line)}` : ""}
            </span>
          ) : (
            "PR comment"
          )}
        </span>
      }
      original={<GithubCommentCard comment={comment} />}
      threads={props.threads}
      githubReplies={props.githubReplies}
      path={comment.path ?? ""}
      line={comment.line ?? comment.original_line ?? 0}
      githubCommentId={comment.id}
      reviewCommentId={comment.path !== null ? comment.id : undefined}
      onClose={props.onClose}
    />
  );
}

export function ReviewModal(props: {
  review: GithubReview;
  threads: Thread[];
  onClose: () => void;
}): ReactElement {
  const { review } = props;
  return (
    <GithubShell
      title={
        <span className="inline-flex items-center gap-1.5">
          <GithubMark size={11} /> review by {review.author.login}
        </span>
      }
      original={
        <>
          <div className="mb-2 flex items-center gap-2 text-xs">
            <Pill tone={verdictTone(review.verdict)}>{review.verdict.replace("_", " ")}</Pill>
          </div>
          <MarkdownBody text={review.body} />
        </>
      }
      threads={props.threads}
      githubReplies={[]}
      path=""
      line={0}
      githubCommentId={review.id}
      onClose={props.onClose}
    />
  );
}

function Tab({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium transition-colors ${
        active ? "bg-sky/20 text-sky" : "bg-panel-2 text-muted hover:text-cream"
      }`}
    >
      {children}
    </button>
  );
}
