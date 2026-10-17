// Open things in the system browser (via the Tauri opener plugin).
// GitHub deep links: review (inline) comments use #discussion_r<id>,
// plain PR comments use #issuecomment-<id>.

import { openUrl } from "@tauri-apps/plugin-opener";

import type { GithubComment, LocalComment, RepoRef } from "./types";

export function openExternal(url: string): void {
  openUrl(url).catch((e: unknown) => {
    console.error("failed to open url", url, e);
  });
}

export function prUrl(repo: RepoRef, number: number): string {
  return `https://github.com/${repo.owner}/${repo.name}/pull/${String(number)}`;
}

export function githubCommentUrl(repo: RepoRef, number: number, comment: GithubComment): string {
  const anchor =
    comment.path !== null
      ? `#discussion_r${String(comment.id)}`
      : `#issuecomment-${String(comment.id)}`;
  return `${prUrl(repo, number)}${anchor}`;
}

/** Link for a local comment that has been posted to GitHub. */
export function postedCommentUrl(comment: LocalComment): string | null {
  if (comment.posted_github_id === null) return null;
  const anchor =
    comment.path && comment.line > 0
      ? `#discussion_r${String(comment.posted_github_id)}`
      : `#issuecomment-${String(comment.posted_github_id)}`;
  return `${prUrl(comment.repo, comment.pr_number)}${anchor}`;
}
