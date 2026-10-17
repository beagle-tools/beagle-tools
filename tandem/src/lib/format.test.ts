import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fileAnchorId, mentionsUser, relativeTime, repoSlug, shortSha } from "./format";

describe("shortSha", () => {
  it("keeps the first seven characters", () => {
    expect(shortSha("0123456789abcdef")).toBe("0123456");
    expect(shortSha("abc")).toBe("abc");
  });
});

describe("relativeTime", () => {
  const now = new Date("2026-06-15T12:00:00Z");
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(now);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const ago = (seconds: number) => new Date(now.getTime() - seconds * 1000).toISOString();

  it("says 'just now' under a minute", () => {
    expect(relativeTime(ago(0))).toBe("just now");
    expect(relativeTime(ago(59))).toBe("just now");
  });

  it("steps through minutes, hours, days, months and years", () => {
    expect(relativeTime(ago(5 * 60))).toBe("5m ago");
    expect(relativeTime(ago(3 * 3600))).toBe("3h ago");
    expect(relativeTime(ago(2 * 86400))).toBe("2d ago");
    expect(relativeTime(ago(60 * 86400))).toBe("2mo ago");
    expect(relativeTime(ago(400 * 86400))).toBe("1y ago");
  });

  it("returns an empty string for an unparseable timestamp", () => {
    expect(relativeTime("not a date")).toBe("");
  });
});

describe("repoSlug / fileAnchorId", () => {
  it("formats owner/name and diff anchors", () => {
    expect(repoSlug({ owner: "acme", name: "widgets" })).toBe("acme/widgets");
    expect(fileAnchorId("src/a.ts")).toBe("diff-src/a.ts");
  });
});

describe("mentionsUser", () => {
  it("matches @login case-insensitively", () => {
    expect(mentionsUser("thanks @Alice!", "alice")).toBe(true);
  });

  it("requires a word boundary after the login", () => {
    expect(mentionsUser("cc @alice-bot", "alice")).toBe(false);
    expect(mentionsUser("cc @alicex", "alice")).toBe(false);
  });

  it("escapes regex metacharacters in the login", () => {
    expect(mentionsUser("@a.b here", "a.b")).toBe(true);
    expect(mentionsUser("@axb here", "a.b")).toBe(false);
  });

  it("is false without a login", () => {
    expect(mentionsUser("@alice", null)).toBe(false);
    expect(mentionsUser("@alice", "")).toBe(false);
  });
});
