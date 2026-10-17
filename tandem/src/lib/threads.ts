// Threading for local review comments: pure, so the comment panel, the
// diff and the thread pane all group the same way.

import type { LocalComment } from "./types";

/** Group local comments into threads: roots with their replies. */
export function groupThreads(
  comments: LocalComment[],
): { root: LocalComment; replies: LocalComment[] }[] {
  const byId = new Map(comments.map((c) => [c.id, c]));
  // Replies whose parent was deleted: cluster siblings under the oldest
  // orphan so the thread survives instead of shattering into cards.
  const orphans = new Map<string, LocalComment[]>();
  for (const c of comments) {
    if (c.parent_id && !byId.has(c.parent_id)) {
      const group = orphans.get(c.parent_id) ?? [];
      group.push(c);
      orphans.set(c.parent_id, group);
    }
  }
  const promoted = new Set<string>();
  const adoptedBy = new Map<string, string>();
  for (const group of orphans.values()) {
    group.sort((a, b) => a.created_at.localeCompare(b.created_at));
    const head = group[0];
    if (!head) continue;
    promoted.add(head.id);
    for (const rest of group.slice(1)) adoptedBy.set(rest.id, head.id);
  }
  const roots = comments.filter(
    (c) => !c.parent_id || (!byId.has(c.parent_id) && promoted.has(c.id)),
  );
  return roots.map((root) => ({
    root,
    replies: comments
      .filter((c) => c.parent_id === root.id || adoptedBy.get(c.id) === root.id)
      .sort((a, b) => a.created_at.localeCompare(b.created_at)),
  }));
}
