// Compose a reply that goes STRAIGHT to GitHub — no local thread.
// Still gated behind an explicit two-step confirm, like every write.

import { Send } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { useAppStore } from "../state/store";
import { pushGithubError } from "../state/toasts";
import { MentionInput } from "./MentionInput";
import { Button, Spinner } from "./ui";

interface GithubReplyComposerProps {
  /** Review-comment id to thread under; omit for a plain PR comment. */
  reviewCommentId?: number | undefined;
  /** Anchor a brand-new inline comment to a diff line. */
  anchor?: { path: string; line: number; sideNew: boolean } | undefined;
  onDone?: (() => void) | undefined;
}

export function GithubReplyComposer(props: GithubReplyComposerProps): ReactElement | null {
  const bundle = useAppStore((s) => s.bundle);
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  const [body, setBody] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [working, setWorking] = useState(false);

  if (!bundle) return null;
  const pr = bundle.detail.pull_request;

  const post = () => {
    setWorking(true);
    void ipc
      .replyOnGithub({
        repo: `${pr.repo.owner}/${pr.repo.name}`,
        number: pr.number,
        body: body.trim(),
        review_comment_id: props.reviewCommentId ?? null,
        path: props.anchor?.path ?? null,
        line: props.anchor?.line ?? null,
        side_new: props.anchor?.sideNew ?? null,
      })
      .then(async () => {
        setBody("");
        setConfirming(false);
        await refreshBundle();
        props.onDone?.();
      })
      .catch((e: unknown) => {
        pushGithubError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setWorking(false);
      });
  };

  return (
    <div className="flex flex-col gap-2">
      <MentionInput
        peopleOnly
        value={body}
        onChange={(v) => {
          setBody(v);
          setConfirming(false);
        }}
        onSubmit={() => {
          if (body.trim()) setConfirming(true);
        }}
        placeholder="reply on github — markdown, @ mentions real users, posts under your account (⌘↵)"
      />
      <div className="flex items-center gap-1.5">
        {working ? (
          <Spinner label="posting to github…" />
        ) : confirming ? (
          <>
            <Button kind="danger" onClick={post} title="this WILL post to GitHub">
              <Send size={11} /> confirm post
            </Button>
            <Button
              onClick={() => {
                setConfirming(false);
              }}
            >
              cancel
            </Button>
          </>
        ) : (
          <Button
            kind="primary"
            disabled={!body.trim()}
            onClick={() => {
              setConfirming(true);
            }}
          >
            <Send size={11} /> post reply to github
          </Button>
        )}
      </div>
    </div>
  );
}
