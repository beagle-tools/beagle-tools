import { describe, expect, it } from "vitest";

import { COMMANDS_FILE, cycleTab, DIAGRAMS_TAB, SECTIONS, TABS, tabsFor } from "./sections";

describe("SECTIONS", () => {
  it("lists the nine sections of the on-disk format, in tab order", () => {
    expect(SECTIONS.map((s) => s.file)).toEqual([
      "summary.md",
      "timeline.md",
      "root-cause.md",
      "impact.md",
      "remediation.md",
      "commands.md",
      "final-review.md",
      "notes.md",
      "log.md",
    ]);
  });

  it("has unique files and titles", () => {
    expect(new Set(SECTIONS.map((s) => s.file)).size).toBe(SECTIONS.length);
    expect(new Set(SECTIONS.map((s) => s.title)).size).toBe(SECTIONS.length);
  });
});

describe("TABS", () => {
  it("inserts Diagrams after Final Review, matching the TUI's tab order", () => {
    expect(TABS).toHaveLength(10);
    expect(TABS[7]).toEqual(DIAGRAMS_TAB);
    expect(TABS.map((t) => t.title)).toEqual([
      "Summary",
      "Timeline",
      "Root Cause",
      "Impact",
      "Fix",
      "Commands",
      "Final Review",
      "Diagrams",
      "Notes",
      "Log",
    ]);
  });

  it("keeps the diagrams pseudo-file distinct from every real section", () => {
    expect(SECTIONS.some((s) => s.file === DIAGRAMS_TAB.file)).toBe(false);
  });
});

describe("tabsFor", () => {
  it("hides the opt-in Commands tab unless the workspace has commands.md", () => {
    expect(tabsFor(false).some((t) => t.file === COMMANDS_FILE)).toBe(false);
    expect(tabsFor(false)).toHaveLength(9);
    expect(tabsFor(true)).toEqual(TABS);
  });
});

describe("cycleTab", () => {
  it("wraps in both directions", () => {
    expect(cycleTab("summary.md", 1)).toBe("timeline.md");
    expect(cycleTab("summary.md", -1)).toBe("log.md");
    expect(cycleTab("log.md", 1)).toBe("summary.md");
  });

  it("skips Commands without it and visits it with it", () => {
    expect(cycleTab("remediation.md", 1)).toBe("final-review.md");
    expect(cycleTab("remediation.md", 1, tabsFor(true))).toBe("commands.md");
  });

  it("recovers from an unknown file by starting at the first tab", () => {
    expect(cycleTab("nonsense.md", 1)).toBe("timeline.md");
  });
});
