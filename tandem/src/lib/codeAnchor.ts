// Links inside agent prose. A reference can be three different things
// and each wants a different landing: a line in the diff you're reading,
// a file that isn't part of this PR (so: GitHub, on the base branch),
// or an ordinary web link. Nothing may ever navigate the webview
// itself — that unloads the SPA and dumps you back on the home page.

import { fileAnchorId } from "./format";
import { openExternal } from "./open";
import type { RepoRef } from "./types";

export interface CodeAnchor {
  path: string;
  /** Line on the new side; absent when the link names only a file. */
  line: number | null;
}

export type LinkTarget =
  /** A path in this repo: try the diff first, else GitHub. */
  { kind: "path"; anchor: CodeAnchor } | { kind: "external"; url: string } | { kind: "none" };

export function parseCodeAnchor(href: string | undefined): CodeAnchor | null {
  if (!href?.startsWith("tandem://")) return null;
  return parsePathWithLine(href.slice("tandem://".length));
}

/** `src/x.rs#L42` → path + line. */
function parsePathWithLine(rest: string): CodeAnchor | null {
  if (!rest) return null;
  const [raw, hash] = rest.split("#");
  const path = raw?.replace(/^\.?\//, "");
  if (!path) return null;
  const line = /^L(\d+)$/i.exec(hash ?? "");
  return { path, line: line?.[1] ? Number(line[1]) : null };
}

/** What a link in agent prose actually means. Bare paths count: agents
 * write `[README.md](README.md)` as readily as a tandem:// anchor. */
export function resolveLink(href: string | undefined): LinkTarget {
  if (!href || href.startsWith("#")) return { kind: "none" };
  const anchor = parseCodeAnchor(href);
  if (anchor) return { kind: "path", anchor };
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
    // Any other scheme (http, mailto, …) belongs to the browser.
    return /^https?:/i.test(href) ? { kind: "external", url: href } : { kind: "none" };
  }
  const path = parsePathWithLine(href);
  return path ? { kind: "path", anchor: path } : { kind: "none" };
}

/** GitHub's view of a file at a ref, line anchor included. */
export function blobUrl(repo: RepoRef, ref: string, anchor: CodeAnchor): string {
  const line = anchor.line === null ? "" : `#L${String(anchor.line)}`;
  return `https://github.com/${repo.owner}/${repo.name}/blob/${ref}/${anchor.path}${line}`;
}

/** Scroll to the anchored line. Returns false when the file isn't in
 * the diff on screen, which is the caller's cue to send you to GitHub
 * instead — a summary often cites files this PR never touched. */
export function scrollToCodeAnchor(anchor: CodeAnchor): boolean {
  const file = document.getElementById(fileAnchorId(anchor.path));
  if (!file) return false;
  const row =
    anchor.line === null
      ? null
      : file.querySelector<HTMLElement>(`[data-new-line="${String(anchor.line)}"]`);
  const target = row ?? file;
  target.scrollIntoView({ behavior: "smooth", block: row ? "center" : "start" });
  if (!row) return true;
  row.classList.add("flash-target");
  setTimeout(() => {
    row.classList.remove("flash-target");
  }, 3200);
  return true;
}

/** Follow a resolved link: the diff when it's there, GitHub when it
 * isn't. `ref` is the PR's base branch — a file outside the diff is
 * being discussed as it stands on the branch, not in this PR. */
export function followLink(target: LinkTarget, repo: RepoRef | null, ref: string): void {
  if (target.kind === "none") return;
  if (target.kind === "external") {
    openExternal(target.url);
    return;
  }
  if (scrollToCodeAnchor(target.anchor)) return;
  if (repo) openExternal(blobUrl(repo, ref, target.anchor));
}
