import { describe, expect, it } from "vitest";

import { fuzzyMatch, highlightRuns } from "./fuzzy";

describe("fuzzyMatch", () => {
  it("matches characters in order, not necessarily together", () => {
    expect(fuzzyMatch("trp", "ticket-report")).not.toBeNull();
    expect(fuzzyMatch("rtk", "ticket-report")).toBeNull();
  });

  it("an empty query matches everything with no positions", () => {
    expect(fuzzyMatch("  ", "anything")).toEqual({ score: 0, positions: [] });
  });

  it("prefers word starts and tight spans", () => {
    const tight = fuzzyMatch("ch", "code-health");
    expect(tight?.positions).toEqual([0, 5]);
    const exact = fuzzyMatch("scan", "code-health-scan")?.score ?? 0;
    const scattered = fuzzyMatch("scan", "s-c-a-n-something")?.score ?? 0;
    expect(exact).toBeGreaterThan(scattered);
  });
});

describe("highlightRuns", () => {
  it("splits text into matched and unmatched runs", () => {
    expect(highlightRuns("abcd", [1, 2])).toEqual([
      { text: "a", hit: false },
      { text: "bc", hit: true },
      { text: "d", hit: false },
    ]);
  });
});
