// Small formatting helpers shared across components.

export function shortSha(sha: string): string {
  return sha.slice(0, 7);
}

export function relativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  const seconds = Math.round((Date.now() - then) / 1000);
  if (Number.isNaN(seconds)) return "";
  if (seconds < 60) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${String(hours)}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${String(days)}d ago`;
  const months = Math.round(days / 30);
  if (months < 12) return `${String(months)}mo ago`;
  return `${String(Math.round(months / 12))}y ago`;
}

export function repoSlug(repo: { owner: string; name: string }): string {
  return `${repo.owner}/${repo.name}`;
}

/** DOM id for a file's diff card, used by the file tree to scroll. */
export function fileAnchorId(path: string): string {
  return `diff-${path}`;
}

/** Does this body @mention the given user (word-boundary, any case)? */
export function mentionsUser(body: string, login: string | null): boolean {
  if (!login) return false;
  const escaped = login.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`@${escaped}(?![\\w-])`, "i").test(body);
}
