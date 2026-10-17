// Chat-style scrolling: keep the newest message in view as a thread
// grows, but stop the moment someone scrolls up to read something —
// yanking them back to the bottom mid-sentence is worse than making
// them scroll down themselves.

import { useEffect, useRef } from "react";
import type { RefObject } from "react";

/** How close to the bottom still counts as "following along". */
const SLACK_PX = 64;

function scrollParent(node: HTMLElement | null): HTMLElement | null {
  let el = node?.parentElement ?? null;
  while (el) {
    const overflow = getComputedStyle(el).overflowY;
    if ((overflow === "auto" || overflow === "scroll") && el.scrollHeight > el.clientHeight) {
      return el;
    }
    el = el.parentElement;
  }
  return null;
}

/**
 * Scrolls `ref` (a sentinel at the end of the content) into view
 * whenever `signal` changes, as long as the reader hasn't scrolled up.
 * Works against whichever ancestor actually scrolls, so the same thread
 * behaves the same inline and inside a modal.
 */
export function useStickToBottom(ref: RefObject<HTMLElement | null>, signal: string): void {
  const following = useRef(true);

  useEffect(() => {
    const el = scrollParent(ref.current);
    if (!el) return;
    const onScroll = () => {
      following.current = el.scrollHeight - el.scrollTop - el.clientHeight <= SLACK_PX;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      el.removeEventListener("scroll", onScroll);
    };
  }, [ref, signal]);

  useEffect(() => {
    if (!following.current) return;
    // After paint: the new message has to be laid out before we can
    // scroll past it.
    const frame = requestAnimationFrame(() => {
      ref.current?.scrollIntoView({ block: "end" });
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [ref, signal]);
}
