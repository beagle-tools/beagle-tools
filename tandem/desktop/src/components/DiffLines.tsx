// One diff line and the threads under it: the line row (line numbers,
// + button, highlighted content) and the GitHub + local threads anchored
// there. Split from DiffViewer.tsx.

import type { ReactElement } from "react";

import { highlightLine } from "../lib/highlight";
import type { FileDiff, GithubComment, ReviewThreadMeta } from "../lib/types";
import type { groupThreads } from "./comments";
import { GithubThread } from "./GithubThread";
import { LocalThreadBox } from "./LocalThreadBox";

function LineContent({ content, language }: { content: string; language: string | null }) {
  const html = language ? highlightLine(content, language) : null;
  if (html === null) return <>{content}</>;
  // hljs escapes its output, so this only injects highlight spans.
  return <span dangerouslySetInnerHTML={{ __html: html }} />;
}

type LocalThread = ReturnType<typeof groupThreads>[number];

interface GhThread {
  ghRoot: GithubComment;
  ghReplies: GithubComment[];
  outdated: boolean;
}

/** GitHub review threads rooted at this diff line. GitHub comments only
 * anchor to the new side; outdated/resolved threads have `line = null`
 * but keep `original_line`. */
export function githubThreadsAt(
  githubComments: GithubComment[],
  anchorSide: "old" | "new",
  anchorLine: number | null,
): GhThread[] {
  return githubComments
    .filter(
      (c) =>
        anchorSide === "new" &&
        c.in_reply_to_id === null &&
        (c.line ?? c.original_line) === anchorLine,
    )
    .map((ghRoot) => ({
      ghRoot,
      ghReplies: githubComments.filter((c) => c.in_reply_to_id === ghRoot.id),
      outdated: ghRoot.line === null,
    }));
}

/** Every thread under one diff line: GitHub threads (with the local
 * discussions attached to them), then purely local threads. */
export function LineThreads({
  ghThreads,
  threads,
  reviewThreads,
  onDiscuss,
}: {
  ghThreads: GhThread[];
  threads: LocalThread[];
  reviewThreads: ReviewThreadMeta[];
  onDiscuss: (ghRoot: GithubComment) => void;
}): ReactElement {
  const ghIds = new Set(ghThreads.flatMap((t) => [t.ghRoot.id, ...t.ghReplies.map((r) => r.id)]));
  return (
    <>
      {ghThreads.map(({ ghRoot, ghReplies, outdated }) => (
        <GithubThread
          key={ghRoot.id}
          root={ghRoot}
          replies={ghReplies}
          outdated={outdated}
          resolved={reviewThreads.find((m) => m.root_comment_id === ghRoot.id)?.resolved ?? false}
          threadId={reviewThreads.find((m) => m.root_comment_id === ghRoot.id)?.id ?? null}
          localThreads={threads.filter(
            (t) => t.root.github_comment_id !== null && ghIds.has(t.root.github_comment_id),
          )}
          onDiscuss={() => {
            onDiscuss(ghRoot);
          }}
        />
      ))}
      {threads
        .filter((t) => t.root.github_comment_id === null || !ghIds.has(t.root.github_comment_id))
        .map((thread) => (
          <LocalThreadBox key={thread.root.id} thread={thread} />
        ))}
    </>
  );
}

/** One diff line: the + button, both line numbers (drag handles for a
 * range comment), and the highlighted content. */
export function DiffLineRow({
  line,
  rowClass,
  language,
  onMouseEnter,
  onStartDrag,
}: {
  line: FileDiff["hunks"][number]["lines"][number];
  rowClass: string;
  language: string | null;
  onMouseEnter: () => void;
  onStartDrag: (e: React.MouseEvent) => void;
}): ReactElement {
  const marker = line.kind === "added" ? "+" : line.kind === "removed" ? "−" : " ";
  return (
    <div
      className={`group-line relative ${rowClass}`}
      data-new-line={line.new_line ?? undefined}
      data-old-line={line.old_line ?? undefined}
      onMouseEnter={onMouseEnter}
    >
      <button
        type="button"
        className="diff-add-btn"
        title="comment on this line (drag to select a range)"
        onMouseDown={onStartDrag}
      >
        +
      </button>
      <div className="diff-line">
        <button type="button" className="diff-line-num" onMouseDown={onStartDrag}>
          {line.old_line ?? ""}
        </button>
        <button type="button" className="diff-line-num" onMouseDown={onStartDrag}>
          {line.new_line ?? ""}
        </button>
        <div className="diff-content">
          <span className="select-none pr-1 text-muted">{marker}</span>
          <LineContent content={line.content} language={language} />
        </div>
      </div>
    </div>
  );
}
