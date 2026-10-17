import { describe, expect, it } from "vitest";

import { fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
  it("scores an empty or whitespace query as a neutral match", () => {
    expect(fuzzyScore("", "anything")).toBe(0);
    expect(fuzzyScore("   ", "anything")).toBe(0);
  });

  it("ranks contiguous substrings by how early they appear", () => {
    expect(fuzzyScore("fix", "fix the bug")).toBe(1000);
    expect(fuzzyScore("bug", "fix the bug")).toBe(1000 - 8);
  });

  it("is case-insensitive", () => {
    expect(fuzzyScore("FIX", "Fix the bug")).toBe(1000);
  });

  it("matches in-order subsequences below any substring hit", () => {
    const score = fuzzyScore("ftb", "fix the bug");
    expect(score).not.toBeNull();
    expect(score).toBeLessThan(1000 - "fix the bug".length);
  });

  it("prefers adjacent subsequence hits over scattered ones", () => {
    // "ab" then a gap: two adjacent hits (5 + 5) beat two scattered (1 + 1).
    const tight = fuzzyScore("a b", "ab-x");
    const loose = fuzzyScore("a b", "xa-xb");
    expect(tight).not.toBeNull();
    expect(loose).not.toBeNull();
    expect(tight ?? 0).toBeGreaterThan(loose ?? 0);
  });

  it("returns null when the characters are missing or out of order", () => {
    expect(fuzzyScore("xyz", "fix the bug")).toBeNull();
    expect(fuzzyScore("gubx", "fix the bug")).toBeNull();
  });
});
