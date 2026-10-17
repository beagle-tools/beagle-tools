import { describe, expect, it, vi } from "vitest";

import { makeComment } from "../test/fixtures";
import type { GithubComment } from "./types";

// The opener plugin talks to the Tauri runtime; these tests only cover
// the pure URL builders, so stub it out.
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(() => Promise.resolve()) }));

const { githubCommentUrl, postedCommentUrl, prUrl } = await import("./open");

const repo = { owner: "acme", name: "widgets" };

describe("prUrl", () => {
  it("builds the github.com pull URL", () => {
    expect(prUrl(repo, 42)).toBe("https://github.com/acme/widgets/pull/42");
  });
});

describe("githubCommentUrl", () => {
  it("uses #discussion_r for inline comments and #issuecomment for PR comments", () => {
    const inline = { id: 7, path: "a.ts" } as GithubComment;
    const general = { id: 8, path: null } as GithubComment;
    expect(githubCommentUrl(repo, 1, inline)).toBe(
      "https://github.com/acme/widgets/pull/1#discussion_r7",
    );
    expect(githubCommentUrl(repo, 1, general)).toBe(
      "https://github.com/acme/widgets/pull/1#issuecomment-8",
    );
  });
});

describe("postedCommentUrl", () => {
  it("is null until the comment has been posted", () => {
    expect(postedCommentUrl(makeComment())).toBeNull();
  });

  it("links line comments as review comments and PR-level ones as issue comments", () => {
    expect(postedCommentUrl(makeComment({ posted_github_id: 5 }))).toBe(
      "https://github.com/acme/widgets/pull/1#discussion_r5",
    );
    expect(postedCommentUrl(makeComment({ posted_github_id: 6, path: "", line: 0 }))).toBe(
      "https://github.com/acme/widgets/pull/1#issuecomment-6",
    );
  });
});
