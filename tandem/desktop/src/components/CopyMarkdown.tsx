// Copy-as-markdown buttons. Small on purpose: they sit in rows that are
// already busy, and copying is a thing you reach for without looking.

import { Check, Copy } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import { pushInfo } from "../state/toasts";

export function CopyMarkdown({
  text,
  label,
  title,
}: {
  text: () => string;
  /** Shown next to the icon; omitted for the per-message button. */
  label?: string;
  title: string;
}): ReactElement {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      title={title}
      onClick={(e) => {
        e.preventDefault();
        navigator.clipboard
          .writeText(text())
          .then(() => {
            setCopied(true);
            pushInfo(label === undefined ? "message copied" : "conversation copied");
            setTimeout(() => {
              setCopied(false);
            }, 1500);
          })
          .catch(console.warn);
      }}
      className="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-[10px] text-muted transition-colors hover:bg-panel-2 hover:text-cream"
    >
      {copied ? <Check size={11} className="text-moss" /> : <Copy size={11} />}
      {label}
    </button>
  );
}
