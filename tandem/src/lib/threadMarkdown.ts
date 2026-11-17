// Turning a thread into markdown you can paste somewhere else — a
// GitHub reply box, a Slack message, a ticket. The point is that what
// you paste reads like the conversation you had, with who said what
// still attached.

import type { GithubComment, LocalComment } from "./types";

export interface Said {
  author: string;
  body: string;
}

function block({ author, body }: Said): string {
  return `**${author}**\n\n${body.trim()}`;
}

/** A conversation, oldest first, with a heading when there's somewhere
 * to anchor it. */
export function transcript(heading: string | null, said: Said[]): string {
  const parts = said.map(block).join("\n\n---\n\n");
  return heading === null ? parts : `### ${heading}\n\n${parts}`;
}

function anchor(comment: LocalComment): string | null {
  if (!comment.path || comment.line <= 0) return null;
  const end =
    comment.end_line !== null && comment.end_line > comment.line
      ? `-${String(comment.end_line)}`
      : "";
  return `${comment.path}:${String(comment.line)}${end}`;
}

/** One local thread: root then replies. */
export function localThreadMarkdown(root: LocalComment, replies: LocalComment[]): string {
  const said = [root, ...replies].map((c) => ({ author: c.author_name, body: c.body }));
  return transcript(anchor(root), said);
}

/** A GitHub comment and everything said about it — on GitHub and in
 * Tandem — in the order it happened on each side. */
export function githubDiscussionMarkdown(
  original: Said,
  githubReplies: GithubComment[],
  localThreads: { root: LocalComment; replies: LocalComment[] }[],
  heading: string | null,
): string {
  const said: Said[] = [original];
  for (const reply of githubReplies) {
    said.push({ author: `${reply.author.login} (github)`, body: reply.body });
  }
  for (const thread of localThreads) {
    for (const comment of [thread.root, ...thread.replies]) {
      said.push({ author: `${comment.author_name} (tandem)`, body: comment.body });
    }
  }
  return transcript(heading, said);
}
