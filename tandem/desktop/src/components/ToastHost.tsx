// Bottom-right toast stack: colored by severity, copyable, dismissable.

import type { ReactElement } from "react";
import { Copy, X } from "lucide-react";

import { pushInfo, useToasts } from "../state/toasts";

const KIND_STYLE = {
  error: "border-l-ember text-cream",
  warn: "border-l-amber text-cream",
  info: "border-l-sky text-cream",
};

export function ToastHost(): ReactElement | null {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-[100] flex w-96 flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`animate-fade-up flex items-start gap-2 rounded-lg border border-edge border-l-4 bg-panel px-3 py-2.5 text-xs shadow-2xl ${KIND_STYLE[toast.kind]}`}
        >
          <span className="min-w-0 flex-1 whitespace-pre-wrap break-words leading-relaxed">
            {toast.text}
          </span>
          <button
            type="button"
            title="copy the raw message"
            onClick={() => {
              navigator.clipboard
                .writeText(toast.raw)
                .then(() => {
                  pushInfo("copied");
                })
                .catch(console.warn);
            }}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted hover:bg-panel-2 hover:text-cream"
          >
            <Copy size={12} />
          </button>
          <button
            type="button"
            title="dismiss"
            onClick={() => {
              dismiss(toast.id);
            }}
            className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted hover:bg-panel-2 hover:text-cream"
          >
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
