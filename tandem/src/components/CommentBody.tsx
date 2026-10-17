// A comment's text, which you can take over. Agent comments are drafts:
// reword one, keep what it used to say, then post your version. Edits
// apply to your own comments too — it's the same affordance.

import { Check, History, Pencil, X } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { relativeTime } from "../lib/format";
import { ipc } from "../lib/ipc";
import type { CommentRevision, LocalComment } from "../lib/types";
import { pushInfo } from "../state/toasts";
import { MarkdownBody } from "./Markdown";

export function CommentBody({ comment }: { comment: LocalComment }): ReactElement {
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [history, setHistory] = useState<CommentRevision[] | null>(null);
  const [showHistory, setShowHistory] = useState(false);

  const save = () => {
    if (draft === null || saving) return;
    const next = draft.trim();
    if (!next || next === comment.body) {
      setDraft(null);
      return;
    }
    setSaving(true);
    ipc
      .updateCommentBody(
        comment.id,
        next,
        `${comment.repo.owner}/${comment.repo.name}`,
        comment.pr_number,
      )
      .then(() => {
        // The command emits tandem://comments-updated, which the store
        // already listens for — the list reloads itself.
        setDraft(null);
        setHistory(null);
      })
      .catch((e: unknown) => {
        console.error("edit failed", e);
        pushInfo("couldn't save that edit");
      })
      .finally(() => {
        setSaving(false);
      });
  };

  const toggleHistory = () => {
    const next = !showHistory;
    setShowHistory(next);
    if (!next || history !== null) return;
    ipc
      .listCommentRevisions(comment.id)
      .then(setHistory)
      .catch((e: unknown) => {
        console.warn("couldn't load history", e);
        setHistory([]);
      });
  };

  if (draft !== null) {
    return (
      <div className="flex flex-col gap-1.5">
        <textarea
          autoFocus
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) save();
            if (e.key === "Escape") setDraft(null);
          }}
          className="min-h-32 w-full resize-y rounded-md border border-edge bg-ground p-2 text-xs leading-relaxed text-cream outline-none focus:border-sky"
        />
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={save}
            disabled={saving}
            title="save (⌘↵) — the previous version is kept"
            className="inline-flex items-center gap-1 rounded-md bg-sky-deep px-2 py-1 text-[11px] font-medium text-white transition-colors hover:bg-sky disabled:opacity-40"
          >
            <Check size={11} /> save
          </button>
          <button
            type="button"
            onClick={() => {
              setDraft(null);
            }}
            title="discard (esc)"
            className="inline-flex items-center gap-1 rounded-md bg-panel-2 px-2 py-1 text-[11px] text-muted ring-1 ring-edge/60 transition-colors hover:text-cream"
          >
            <X size={11} /> cancel
          </button>
          <span className="ml-auto text-[10px] text-muted">
            {comment.author_kind === "agent" ? "editing the agent's words" : "editing your comment"}
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="group/body">
      <MarkdownBody text={comment.body} />
      <div className="mt-1 flex items-center gap-2 text-[10px] text-muted">
        <button
          type="button"
          onClick={() => {
            setDraft(comment.body);
          }}
          title={
            comment.posted_github_id === null
              ? "edit this comment"
              : "edit — note this comment is already on GitHub, where it won't change"
          }
          className="inline-flex items-center gap-1 opacity-0 transition-opacity hover:text-cream group-hover/body:opacity-100"
        >
          <Pencil size={10} /> edit
        </button>
        {comment.edit_count > 0 ? (
          <button
            type="button"
            onClick={toggleHistory}
            title="what this comment said before"
            className="inline-flex items-center gap-1 hover:text-cream"
          >
            <History size={10} /> edited{" "}
            {comment.edit_count === 1 ? "once" : `${String(comment.edit_count)}×`}
          </button>
        ) : null}
      </div>
      {showHistory ? <Revisions revisions={history} /> : null}
    </div>
  );
}

function Revisions({ revisions }: { revisions: CommentRevision[] | null }) {
  if (revisions === null) {
    return <div className="mt-1 text-[10px] text-muted">loading history…</div>;
  }
  return (
    <div className="mt-1.5 space-y-1.5 rounded-md border border-edge/60 bg-panel-2/40 p-2">
      {revisions.map((revision, i) => (
        <div key={revision.id}>
          <div className="mb-0.5 text-[10px] text-muted">
            v{i + 1} · {revision.editor} · replaced {relativeTime(revision.edited_at)}
          </div>
          <div className="whitespace-pre-wrap break-words font-mono text-[10px] leading-relaxed text-muted">
            {revision.body}
          </div>
        </div>
      ))}
    </div>
  );
}
