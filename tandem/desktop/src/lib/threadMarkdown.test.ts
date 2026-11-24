import { describe, expect, it } from "vitest";

import { makeComment } from "../test/fixtures";
import { githubDiscussionMarkdown, localThreadMarkdown, transcript } from "./threadMarkdown";
import type { GithubComment } from "./types";

describe("transcript", () => {
  it("separates speakers and omits the heading when there is none", () => {
    const md = transcript(null, [
      { author: "a", body: " one \n" },
      { author: "b", body: "two" },
    ]);
    expect(md).toBe("**a**\n\none\n\n---\n\n**b**\n\ntwo");
  });

  it("puts the heading first", () => {
    expect(transcript("src/x.rs:3", [{ author: "a", body: "hi" }])).toBe(
      "### src/x.rs:3\n\n**a**\n\nhi",
    );
  });
});

describe("localThreadMarkdown", () => {
  it("anchors a ranged thread and keeps reply order", () => {
    const root = makeComment({ path: "src/a.ts", line: 4, end_line: 6, author_name: "me" });
    const reply = makeComment({ id: "r2", parent_id: root.id, author_name: "bot", body: "ok" });
    const md = localThreadMarkdown(root, [reply]);
    expect(md.startsWith("### src/a.ts:4-6\n\n**me**")).toBe(true);
    expect(md.endsWith("**bot**\n\nok")).toBe(true);
  });

  it("drops the heading for a PR-level comment", () => {
    const root = makeComment({ path: "", line: 0, author_name: "me", body: "pr-wide" });
    expect(localThreadMarkdown(root, [])).toBe("**me**\n\npr-wide");
  });
});

describe("githubDiscussionMarkdown", () => {
  it("labels each side of the conversation", () => {
    const reply = { author: { login: "octo" }, body: "gh reply" } as unknown as GithubComment;
    const root = makeComment({ author_name: "me", body: "local" });
    const md = githubDiscussionMarkdown(
      { author: "orig", body: "first" },
      [reply],
      [{ root, replies: [] }],
      null,
    );
    expect(md).toBe(
      "**orig**\n\nfirst\n\n---\n\n**octo (github)**\n\ngh reply\n\n---\n\n**me (tandem)**\n\nlocal",
    );
  });
});
