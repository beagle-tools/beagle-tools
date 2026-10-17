// The GitHub action buttons in the PR title bar (ready for review, copy
// URL, approve), split out of PrHeader.tsx to honor the 400-line rule.

import { Check, GitPullRequestArrow, Link, Loader, Sparkles } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { ipc } from "../lib/ipc";
import { useAppStore } from "../state/store";
import { pushInfo } from "../state/toasts";
import { Button, Spinner } from "./ui";

/** Take your own draft out of draft. Only rendered on a draft you
 * authored — GitHub rejects it from anyone else. */
export function ReadyForReviewButton({
  repo,
  number,
}: {
  repo: string;
  number: number;
}): ReactElement {
  const refreshBundle = useAppStore((s) => s.refreshBundle);
  const [working, setWorking] = useState(false);
  if (working) return <Spinner label="opening for review…" />;
  return (
    <Button
      kind="primary"
      title="mark this PR ready for review on GitHub (takes it out of draft)"
      onClick={() => {
        setWorking(true);
        ipc
          .markReadyForReview(repo, number)
          .then(() => {
            pushInfo("marked ready for review");
            return refreshBundle();
          })
          .catch((e: unknown) => {
            console.error("ready for review failed", e);
            pushInfo("couldn't mark it ready — check GitHub");
          })
          .finally(() => {
            setWorking(false);
          });
      }}
    >
      <GitPullRequestArrow size={12} /> ready for review
    </Button>
  );
}

/** Copy-URL with a transient success check in place of the link icon. */
export function CopyUrlButton({ url }: { url: string }): ReactElement {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      onClick={() => {
        navigator.clipboard
          .writeText(url)
          .then(() => {
            setCopied(true);
            pushInfo("PR URL copied");
            setTimeout(() => {
              setCopied(false);
            }, 1500);
          })
          .catch(console.warn);
      }}
      title="copy the PR URL"
    >
      {copied ? <Check size={12} className="text-moss" /> : <Link size={12} />}
    </Button>
  );
}

/// Approve on GitHub via a popover: optional multi-line markdown review
/// body (with AI polish), explicit confirm.
export function ApproveButton(): ReactElement {
  const approvePr = useAppStore((s) => s.approvePr);
  const [open, setOpen] = useState(false);
  const [working, setWorking] = useState(false);
  const [polishing, setPolishing] = useState(false);
  const [body, setBody] = useState("");

  const submit = () => {
    setWorking(true);
    void approvePr(body.trim() ? body.trim() : null).then(() => {
      setWorking(false);
      setOpen(false);
      setBody("");
    });
  };

  const polish = () => {
    if (!body.trim() || polishing) return;
    setPolishing(true);
    ipc
      .polishText(body)
      .then(setBody)
      .catch((e: unknown) => {
        console.error("polish failed", e);
      })
      .finally(() => {
        setPolishing(false);
      });
  };

  return (
    <span className="relative">
      <Button
        kind="primary"
        title="approve this PR on GitHub (opens a confirm panel)"
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        <Check size={12} /> approve
      </Button>

      {open ? (
        <div className="animate-fade-up absolute right-0 top-full z-40 mt-2 w-96 rounded-xl border border-edge bg-panel p-3 shadow-2xl">
          <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-muted">
            approve on github
          </div>
          <div className="relative">
            <textarea
              autoFocus
              value={body}
              onChange={(e) => {
                setBody(e.target.value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) submit();
                if (e.key === "Escape") setOpen(false);
              }}
              placeholder={
                "optional review comment — markdown supported\n(posted as your review body, \u2318\u21B5 to approve)"
              }
              className="min-h-28 w-full resize-y rounded-md border border-edge bg-ground p-2 pr-8 text-xs leading-relaxed text-cream outline-none focus:border-sky"
            />
            <button
              type="button"
              onClick={polish}
              disabled={polishing || !body.trim()}
              title="polish — fix typos & grammar with AI"
              className="absolute right-1.5 top-1.5 inline-flex size-6 items-center justify-center rounded-md text-muted transition-colors hover:bg-panel-2 hover:text-amber disabled:pointer-events-none disabled:opacity-30"
            >
              {polishing ? <Loader size={12} className="animate-spin" /> : <Sparkles size={12} />}
            </button>
          </div>
          <div className="mt-2 flex items-center gap-1.5">
            {working ? (
              <Spinner label="approving…" />
            ) : (
              <>
                <Button
                  kind="danger"
                  title="this WILL submit an approving review on GitHub"
                  onClick={submit}
                >
                  <Check size={12} /> confirm approve
                </Button>
                <Button
                  onClick={() => {
                    setOpen(false);
                  }}
                >
                  cancel
                </Button>
              </>
            )}
            <span className="ml-auto text-[10px] text-muted">
              {body.trim() ? `${String(body.trim().length)} chars` : "no comment"}
            </span>
          </div>
        </div>
      ) : null}
    </span>
  );
}
