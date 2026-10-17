import { describe, expect, it } from "vitest";

import { makePr } from "../test/fixtures";
import { PR_SORTS, sanitizePrSort, sortPrs } from "./sort";

const prs = [
  makePr({ number: 2, created_at: "2026-01-02", updated_at: "2026-01-05" }),
  makePr({ number: 3, created_at: "2026-01-01", updated_at: "2026-01-03" }),
  makePr({ number: 1, created_at: "2026-01-03", updated_at: "2026-01-04" }),
];
const numbers = (list: typeof prs) => list.map((pr) => pr.number);

describe("sortPrs", () => {
  it("orders by opened date both ways", () => {
    expect(numbers(sortPrs(prs, "opened-asc"))).toEqual([3, 2, 1]);
    expect(numbers(sortPrs(prs, "opened-desc"))).toEqual([1, 2, 3]);
  });

  it("orders by last update, newest first", () => {
    expect(numbers(sortPrs(prs, "updated-desc"))).toEqual([2, 1, 3]);
  });

  it("orders by number both ways", () => {
    expect(numbers(sortPrs(prs, "number-asc"))).toEqual([1, 2, 3]);
    expect(numbers(sortPrs(prs, "number-desc"))).toEqual([3, 2, 1]);
  });

  it("does not mutate its input", () => {
    sortPrs(prs, "number-asc");
    expect(numbers(prs)).toEqual([2, 3, 1]);
  });
});

describe("sanitizePrSort", () => {
  it("passes every known sort through", () => {
    for (const [sort] of PR_SORTS) expect(sanitizePrSort(sort)).toBe(sort);
  });

  it("falls back to newest-opened for unknown values", () => {
    expect(sanitizePrSort("bogus")).toBe("opened-desc");
    expect(sanitizePrSort("")).toBe("opened-desc");
  });
});
