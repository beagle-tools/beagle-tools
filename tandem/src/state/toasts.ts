// System-wide toast alerts (bottom-right). Errors are red; known
// "GitHub said no but that's fine" responses downgrade to amber
// warnings with friendlier wording. Every toast keeps the raw text for
// the copy button.

import { create } from "zustand";

export type ToastKind = "error" | "warn" | "info";

export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
  raw: string;
}

interface ToastStore {
  toasts: Toast[];
  push: (kind: ToastKind, text: string, raw?: string) => void;
  dismiss: (id: number) => void;
}

let nextId = 1;

export const useToasts = create<ToastStore>((set) => ({
  toasts: [],
  push: (kind, text, raw) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { id, kind, text, raw: raw ?? text }] }));
    setTimeout(
      () => {
        set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
      },
      kind === "error" ? 15000 : 8000,
    );
  },
  dismiss: (id) => {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },
}));

/** Downgrade well-known non-problems to warnings with human wording. */
const SOFTENERS: [RegExp, string][] = [
  [/no new commits on the base branch/i, "Branch is already up to date with base."],
  [
    /auto[- ]merge is not allowed/i,
    "Auto-merge isn't enabled for this repository (a GitHub repo setting).",
  ],
  [/rate limit/i, "GitHub rate limit hit — give it a minute."],
  [/not mergeable|405/i, "GitHub refused the merge — requirements aren't met yet."],
];

export function pushGithubError(raw: string): void {
  for (const [pattern, friendly] of SOFTENERS) {
    if (pattern.test(raw)) {
      useToasts.getState().push("warn", friendly, raw);
      return;
    }
  }
  useToasts.getState().push("error", raw, raw);
}

export function pushInfo(text: string): void {
  useToasts.getState().push("info", text);
}
