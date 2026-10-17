import { describe, expect, it } from "vitest";

import { makeComment } from "../test/fixtures";
import { groupThreads } from "./threads";

describe("groupThreads", () => {
  it("attaches replies to their root, oldest first", () => {
    const root = makeComment({ id: "r" });
    const late = makeComment({ id: "b", parent_id: "r", created_at: "2026-01-03T00:00:00Z" });
    const early = makeComment({ id: "a", parent_id: "r", created_at: "2026-01-02T00:00:00Z" });
    const threads = groupThreads([late, root, early]);
    expect(threads).toHaveLength(1);
    expect(threads[0]?.root.id).toBe("r");
    expect(threads[0]?.replies.map((c) => c.id)).toEqual(["a", "b"]);
  });

  it("keeps separate roots as separate threads", () => {
    const threads = groupThreads([makeComment({ id: "x" }), makeComment({ id: "y" })]);
    expect(threads.map((t) => t.root.id)).toEqual(["x", "y"]);
  });

  it("clusters orphaned replies under the oldest orphan of the same parent", () => {
    const first = makeComment({ id: "o1", parent_id: "gone", created_at: "2026-01-02T00:00:00Z" });
    const second = makeComment({
      id: "o2",
      parent_id: "gone",
      created_at: "2026-01-03T00:00:00Z",
    });
    const threads = groupThreads([second, first]);
    expect(threads).toHaveLength(1);
    expect(threads[0]?.root.id).toBe("o1");
    expect(threads[0]?.replies.map((c) => c.id)).toEqual(["o2"]);
  });
});
