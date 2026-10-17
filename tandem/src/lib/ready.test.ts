import { describe, expect, it } from "vitest";

import { makePr } from "../test/fixtures";
import { checksFailing, hasMergeConflicts, isReadyToMerge, prEdgeClass } from "./ready";

describe("isReadyToMerge", () => {
  it("prefers review decision + check rollup when either is present", () => {
    expect(isReadyToMerge(makePr({ review_decision: "APPROVED", checks_state: "SUCCESS" }))).toBe(
      true,
    );
    // A null rollup (no check suites) does not block.
    expect(isReadyToMerge(makePr({ review_decision: "APPROVED", checks_state: null }))).toBe(true);
    expect(isReadyToMerge(makePr({ review_decision: null, checks_state: "SUCCESS" }))).toBe(true);
    expect(
      isReadyToMerge(makePr({ review_decision: "REVIEW_REQUIRED", checks_state: "SUCCESS" })),
    ).toBe(false);
    expect(isReadyToMerge(makePr({ review_decision: "APPROVED", checks_state: "PENDING" }))).toBe(
      false,
    );
  });

  it("ignores a 'blocked' mergeable_state when rollups are present (merge queues)", () => {
    const pr = makePr({
      mergeable_state: "blocked",
      review_decision: "APPROVED",
      checks_state: "SUCCESS",
    });
    expect(isReadyToMerge(pr)).toBe(true);
  });

  it("falls back to mergeable_state === clean without rollups", () => {
    expect(isReadyToMerge(makePr({ mergeable_state: "clean" }))).toBe(true);
    expect(isReadyToMerge(makePr({ mergeable_state: "blocked" }))).toBe(false);
    expect(isReadyToMerge(makePr())).toBe(false);
  });

  it("is never ready as a draft, with conflicts, or with unresolved threads", () => {
    const green = { review_decision: "APPROVED", checks_state: "SUCCESS" };
    expect(isReadyToMerge(makePr({ ...green, draft: true }))).toBe(false);
    expect(isReadyToMerge(makePr({ ...green, mergeable_state: "dirty" }))).toBe(false);
    expect(isReadyToMerge(makePr({ ...green, unresolved_threads: 1 }))).toBe(false);
  });
});

describe("checksFailing / hasMergeConflicts", () => {
  it("treats FAILURE and ERROR rollups as failing", () => {
    expect(checksFailing(makePr({ checks_state: "FAILURE" }))).toBe(true);
    expect(checksFailing(makePr({ checks_state: "ERROR" }))).toBe(true);
    expect(checksFailing(makePr({ checks_state: "PENDING" }))).toBe(false);
  });

  it("reports conflicts only for a dirty merge state", () => {
    expect(hasMergeConflicts(makePr({ mergeable_state: "dirty" }))).toBe(true);
    expect(hasMergeConflicts(makePr({ mergeable_state: "behind" }))).toBe(false);
  });
});

describe("prEdgeClass", () => {
  it("is green when ready, red when failing or conflicted, empty otherwise", () => {
    expect(prEdgeClass(makePr({ mergeable_state: "clean" }))).toContain("border-l-moss");
    expect(prEdgeClass(makePr({ checks_state: "FAILURE" }))).toContain("border-l-ember");
    expect(prEdgeClass(makePr({ mergeable_state: "dirty" }))).toContain("border-l-ember");
    expect(prEdgeClass(makePr())).toBe("");
  });
});
