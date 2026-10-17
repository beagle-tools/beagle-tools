// Keyboard parity with agenttop's DAG page. The TUI's keys are muscle
// memory; a desktop window that needs the mouse for the same job is a
// downgrade even when it looks better.
//
// Guarded against firing while typing: a `d` in the log filter or a job
// parameter must stay a `d`.

import { useEffect } from "react";

export interface Shortcuts {
  onDag: () => void;
  onPinChain: () => void;
  onEscape: () => void;
  onKill: () => void;
}

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}

export function useShortcuts(handlers: Shortcuts): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target) || e.metaKey || e.ctrlKey || e.altKey) return;
      switch (e.key) {
        case "d":
          handlers.onDag();
          break;
        // Shift-D in agenttop: pin the selected run's chain.
        case "D":
          handlers.onPinChain();
          break;
        case "Escape":
          handlers.onEscape();
          break;
        case "x":
          handlers.onKill();
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [handlers]);
}
