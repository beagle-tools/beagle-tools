// Right-panel comment overview. Each thread shows as a compact summary
// card (huge plan-style comments stay two lines here); clicking a card
// opens the full thread in a centered floating pane with replies and
// actions. Sections: open / triaged / archived / on github.

import { MessageSquarePlus, Search, X } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import type { PrBundle } from "../lib/types";
import { useAppStore } from "../state/store";
import { groupThreads } from "./comments";
import { ConflictSection } from "./ConflictSection";
import type { Selected } from "./PanelCards";
import { GithubSummary, ReviewSummary, ThreadSummary } from "./PanelCards";
import { GithubCommentModal, LocalThreadModal, ReviewModal } from "./PanelModals";
import { ReplySection } from "./ReplySection";
import { Button, Modal } from "./ui";

export function CommentsPanel(): ReactElement {
  const bundle = useAppStore((s) => s.bundle);
  const [selected, setSelected] = useState<Selected>(null);
  const comments = bundle?.comments ?? [];
  const allGithub = bundle?.detail.comments ?? [];
  const allReviews = bundle?.detail.review_bodies ?? [];

  const [filter, setFilter] = useState("");
  const hit = (...fields: (string | null | undefined)[]) => {
    const q = filter.trim().toLowerCase();
    if (!q) return true;
    return fields.some((f) => (f ?? "").toLowerCase().includes(q));
  };
  // Threads match on anything you'd remember them by: what was said
  // anywhere in the thread, who said it, and where it sits.
  const threads = groupThreads(comments)
    .filter((t) => t.root.tag !== "conflict")
    .filter((t) =>
      hit(
        t.root.body,
        t.root.author_name,
        t.root.path,
        t.root.tag,
        ...t.replies.flatMap((r) => [r.body, r.author_name]),
      ),
    );
  const open = threads.filter((t) => t.root.status === "open");
  const triaged = threads.filter((t) => t.root.status !== "open" && t.root.status !== "archived");
  const archived = threads.filter((t) => t.root.status === "archived");

  const githubComments = allGithub.filter((c) => hit(c.body, c.author.login, c.path));
  const reviewBodies = allReviews.filter((r) => hit(r.body, r.author.login));

  return (
    <div className="flex flex-col gap-1.5 p-3">
      <ConflictSection />
      <Button
        onClick={() => {
          setSelected({ kind: "new-general" });
        }}
        title="a PR-level local comment, not tied to any code line"
      >
        <MessageSquarePlus size={12} /> general comment
      </Button>

      {comments.length + allGithub.length + allReviews.length > 3 ? (
        <CommentFilter filter={filter} setFilter={setFilter} />
      ) : null}
      {threads.length === 0 && githubComments.length === 0 && reviewBodies.length === 0 ? (
        <div className="p-4 text-center text-xs leading-relaxed text-muted">
          {filter.trim()
            ? `nothing matches “${filter.trim()}”`
            : "no comments yet — hover a diff line and hit +, or run an agent review"}
        </div>
      ) : null}
      {open.length > 0 ? <SectionLabel text={`open · local (${String(open.length)})`} /> : null}
      {open.map((t) => (
        <ThreadSummary key={t.root.id} thread={t} onOpen={setSelected} />
      ))}

      {triaged.length > 0 ? (
        <SectionLabel text={`resolved · local (${String(triaged.length)})`} />
      ) : null}
      {triaged.map((t) => (
        <ThreadSummary key={t.root.id} thread={t} onOpen={setSelected} />
      ))}

      {githubComments.length + reviewBodies.length > 0 ? (
        <>
          <SectionLabel
            text={`on github (${String(githubComments.length + reviewBodies.length)})`}
          />
          {reviewBodies.map((r) => (
            <ReviewSummary key={r.id} review={r} onOpen={setSelected} />
          ))}
          {githubComments.map((c) => (
            <GithubSummary key={c.id} comment={c} onOpen={setSelected} />
          ))}
        </>
      ) : null}

      {archived.length > 0 ? (
        <details className="mt-1">
          <summary className="cursor-pointer text-[11px] uppercase tracking-wide text-muted hover:text-cream">
            archived · local ({archived.length})
          </summary>
          <div className="mt-1.5 flex flex-col gap-1.5 opacity-75">
            {archived.map((t) => (
              <ThreadSummary key={t.root.id} thread={t} onOpen={setSelected} />
            ))}
          </div>
        </details>
      ) : null}

      <SelectedModals
        selected={selected}
        setSelected={setSelected}
        comments={comments}
        allGithub={allGithub}
        allReviews={allReviews}
      />
    </div>
  );
}

function SectionLabel({ text }: { text: string }): ReactElement {
  return <div className="mt-1.5 text-[11px] uppercase tracking-wide text-muted">{text}</div>;
}

/** The search box above the comment list; Escape clears it. */
function CommentFilter({
  filter,
  setFilter,
}: {
  filter: string;
  setFilter: (filter: string) => void;
}): ReactElement {
  return (
    <div className="flex h-7 items-center gap-1.5 rounded-lg border border-edge bg-panel-2 px-2 focus-within:border-sky">
      <Search size={11} className="shrink-0 text-muted" />
      <input
        value={filter}
        onChange={(e) => {
          setFilter(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape" && filter) {
            e.stopPropagation();
            setFilter("");
          }
        }}
        placeholder="search comments…"
        className="h-full min-w-0 flex-1 bg-transparent text-[11px] text-cream outline-none placeholder:text-muted"
      />
      {filter ? (
        <button
          type="button"
          title="clear"
          onClick={() => {
            setFilter("");
          }}
          className="shrink-0 text-muted transition-colors hover:text-cream"
        >
          <X size={11} />
        </button>
      ) : null}
    </div>
  );
}

/** Whichever floating pane the current selection opens (if any). */
function SelectedModals({
  selected,
  setSelected,
  comments,
  allGithub,
  allReviews,
}: {
  selected: Selected;
  setSelected: (selected: Selected) => void;
  comments: PrBundle["comments"];
  allGithub: PrBundle["detail"]["comments"];
  allReviews: PrBundle["detail"]["review_bodies"];
}): ReactElement {
  // Resolve the selection against live store data so the modal updates
  // as replies stream in.
  const selectedThread =
    selected?.kind === "local"
      ? (groupThreads(comments).find((t) => t.root.id === selected.id) ?? null)
      : null;
  const selectedGithub =
    selected?.kind === "github" ? (allGithub.find((c) => c.id === selected.id) ?? null) : null;
  const selectedReview =
    selected?.kind === "review" ? (allReviews.find((r) => r.id === selected.id) ?? null) : null;

  return (
    <>
      {selectedThread ? (
        <LocalThreadModal
          thread={selectedThread}
          onClose={() => {
            setSelected(null);
          }}
        />
      ) : null}

      {selectedGithub ? (
        <GithubCommentModal
          comment={selectedGithub}
          threads={groupThreads(comments).filter(
            (t) => t.root.github_comment_id === selectedGithub.id,
          )}
          githubReplies={allGithub.filter((c) => c.in_reply_to_id === selectedGithub.id)}
          onClose={() => {
            setSelected(null);
          }}
        />
      ) : null}

      {selectedReview ? (
        <ReviewModal
          review={selectedReview}
          threads={groupThreads(comments).filter(
            (t) => t.root.github_comment_id === selectedReview.id,
          )}
          onClose={() => {
            setSelected(null);
          }}
        />
      ) : null}

      {selected?.kind === "new-general" ? (
        <Modal
          title="new general comment (local)"
          onClose={() => {
            setSelected(null);
          }}
        >
          <ReplySection
            path=""
            line={0}
            onDone={() => {
              setSelected(null);
            }}
          />
        </Modal>
      ) : null}
    </>
  );
}
