// The differ: renders structured FileDiffs with syntax highlighting and
// per-line commenting. Hover a line and hit + (or click a line number)
// to attach a local comment — never posted to GitHub.

import { ChevronDown, ChevronRight, CornerDownRight, MessageSquare } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactElement } from "react";

import { fileAnchorId } from "../lib/format";
import { highlightLine, languageForPath } from "../lib/highlight";
import type { FileDiff, GithubComment, LocalComment, ReviewThreadMeta } from "../lib/types";
import { groupThreads } from "./comments";
import { LocalThreadBox } from "./LocalThreadBox";
import { GithubThread } from "./GithubThread";
import { InlineComposerSection } from "./InlineCommentForm";
import { Pill } from "./ui";

interface DiffViewerProps {
  diff: FileDiff[];
  comments: LocalComment[];
  githubComments: GithubComment[];
  reviewThreads: ReviewThreadMeta[];
}

export function DiffViewer({
  diff,
  comments,
  githubComments,
  reviewThreads,
}: DiffViewerProps): ReactElement {
  if (diff.length === 0) {
    return <div className="p-6 text-center text-xs text-muted">diff not loaded yet</div>;
  }
  return (
    <div className="flex flex-col gap-3 p-3">
      {diff.map((file) => (
        <FileCard
          key={`${file.old_path}:${file.new_path}`}
          file={file}
          comments={comments.filter((c) => c.path === file.new_path || c.path === file.old_path)}
          githubComments={githubComments.filter(
            (c) => c.path === file.new_path || c.path === file.old_path,
          )}
          reviewThreads={reviewThreads}
        />
      ))}
    </div>
  );
}

function statusPill(file: FileDiff) {
  switch (file.status) {
    case "added":
      return <Pill tone="moss">added</Pill>;
    case "removed":
      return <Pill tone="ember">deleted</Pill>;
    case "renamed":
      return <Pill tone="amber">renamed</Pill>;
    case "modified":
      return null;
  }
}

interface FileCardProps {
  file: FileDiff;
  comments: LocalComment[];
  githubComments: GithubComment[];
  reviewThreads: ReviewThreadMeta[];
}

function FileCard({ file, comments, githubComments, reviewThreads }: FileCardProps) {
  const [collapsed, setCollapsed] = useState(false);
  const displayPath = file.status === "removed" ? file.old_path : file.new_path;
  const language = languageForPath(displayPath);
  const commentCount = comments.length + githubComments.length;

  return (
    <section
      id={fileAnchorId(displayPath)}
      data-nav-file={displayPath}
      className="animate-fade-up scroll-mt-3 overflow-hidden rounded-xl border border-edge bg-panel shadow-sm"
    >
      <button
        type="button"
        onClick={() => {
          setCollapsed((c) => !c);
        }}
        className="flex w-full items-center gap-2 border-b border-edge bg-panel-2/60 px-3 py-2 text-left transition-colors hover:bg-panel-2"
      >
        <span className="text-muted">
          {collapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
        </span>
        <span className="truncate font-mono text-xs text-cream">{displayPath}</span>
        {file.status === "renamed" ? (
          <span className="inline-flex items-center gap-1 truncate font-mono text-[11px] text-muted">
            <CornerDownRight size={11} /> from {file.old_path}
          </span>
        ) : null}
        {statusPill(file)}
        {commentCount > 0 ? (
          <Pill tone="sky">
            <MessageSquare size={11} /> {commentCount}
          </Pill>
        ) : null}
        <span className="ml-auto shrink-0 text-[11px]">
          <span className="text-moss">+{file.additions}</span>{" "}
          <span className="text-ember">−{file.deletions}</span>
        </span>
      </button>

      {collapsed ? null : file.is_binary ? (
        <div className="px-3 py-4 text-center text-xs text-muted">binary file</div>
      ) : (
        file.hunks.map((hunk, i) => (
          <HunkView
            key={i}
            path={displayPath}
            language={language}
            hunk={hunk}
            comments={comments}
            githubComments={githubComments}
            reviewThreads={reviewThreads}
          />
        ))
      )}
    </section>
  );
}

interface HunkProps {
  path: string;
  language: string | null;
  hunk: FileDiff["hunks"][number];
  comments: LocalComment[];
  githubComments: GithubComment[];
  reviewThreads: ReviewThreadMeta[];
}

interface ComposerAnchor {
  line: number;
  end: number;
  side: "old" | "new";
  githubCommentId?: number | undefined;
}

function HunkView({
  path,
  language,
  hunk,
  comments,
  githubComments,
  reviewThreads,
}: HunkProps): ReactElement {
  const [commentAt, setCommentAt] = useState<ComposerAnchor | null>(null);
  // Click-drag across line numbers selects a range for the comment.
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef<number | null>(null);

  useEffect(() => {
    if (!dragging) return;
    const up = () => {
      setDragging(false);
    };
    window.addEventListener("mouseup", up);
    return () => {
      window.removeEventListener("mouseup", up);
    };
  }, [dragging]);

  // Thread first, place second. A reply can be anchored somewhere other
  // than its root — an agent re-anchors a suggestion to the exact range
  // it replaces — and bucketing by line before grouping tore those
  // replies out of their thread and showed them as separate comments.
  const fileThreads = useMemo(
    () => groupThreads(comments.filter((c) => c.status !== "archived")),
    [comments],
  );

  return (
    <div className="border-b border-edge/40 last:border-b-0">
      <div className="bg-sky/5 px-3 py-1 font-mono text-[11px] text-sky-deep">
        @@ -{hunk.old_start},{hunk.old_count} +{hunk.new_start},{hunk.new_count} @@ {hunk.section}
      </div>
      {hunk.lines.map((line, i) => {
        const anchorSide = line.kind === "removed" ? "old" : "new";
        const anchorLine = line.kind === "removed" ? line.old_line : line.new_line;
        // A thread sits where its root sits.
        const threads = fileThreads.filter(
          (t) => t.root.side === anchorSide && t.root.line === anchorLine,
        );
        const ghThreads = githubThreadsAt(githubComments, anchorSide, anchorLine);
        const inRange =
          commentAt !== null &&
          anchorLine !== null &&
          anchorSide === commentAt.side &&
          anchorLine >= commentAt.line &&
          anchorLine <= commentAt.end;
        const rowClass = `${
          line.kind === "added" ? "diff-added" : line.kind === "removed" ? "diff-removed" : ""
        }${inRange ? " diff-range" : ""}`;

        const startDrag = (e: React.MouseEvent) => {
          if (anchorLine === null) return;
          e.preventDefault();
          dragStart.current = anchorLine;
          setCommentAt({ line: anchorLine, end: anchorLine, side: anchorSide });
          setDragging(true);
        };
        const extendDrag = () => {
          const start = dragStart.current;
          if (!dragging || start === null || anchorLine === null) return;
          setCommentAt((c) =>
            c?.side === anchorSide
              ? { ...c, line: Math.min(start, anchorLine), end: Math.max(start, anchorLine) }
              : c,
          );
        };

        return (
          <div key={i}>
            <DiffLineRow
              line={line}
              rowClass={rowClass}
              language={language}
              onMouseEnter={extendDrag}
              onStartDrag={startDrag}
            />

            <LineThreads
              ghThreads={ghThreads}
              threads={threads}
              reviewThreads={reviewThreads}
              onDiscuss={(ghRoot) => {
                const l = ghRoot.line ?? ghRoot.original_line ?? anchorLine ?? 0;
                setCommentAt({ line: l, end: l, side: "new", githubCommentId: ghRoot.id });
              }}
            />

            {!dragging &&
            commentAt !== null &&
            commentAt.end === anchorLine &&
            commentAt.side === anchorSide ? (
              <div className="border-y border-sky/30 bg-panel-2 px-4 py-2">
                <InlineComposerSection
                  path={path}
                  line={commentAt.line}
                  side={commentAt.side}
                  githubCommentId={commentAt.githubCommentId}
                  endLine={commentAt.end}
                  onEndLineChange={(end) => {
                    setCommentAt({ ...commentAt, end });
                  }}
                  onDone={() => {
                    setCommentAt(null);
                  }}
                />
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

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
function githubThreadsAt(
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
function LineThreads({
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
function DiffLineRow({
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
