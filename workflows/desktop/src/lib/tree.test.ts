import { describe, expect, it } from "vitest";

import { treePrefixes } from "./tree";

describe("treePrefixes", () => {
  it("gives roots no guide", () => {
    expect(treePrefixes([0, 0])).toEqual(["", ""]);
  });

  it("marks the last sibling with └─ and earlier ones with ├─", () => {
    expect(treePrefixes([0, 1, 1])).toEqual(["", "├─ ", "└─ "]);
  });

  it("carries │ down past an ancestor that still has siblings", () => {
    expect(treePrefixes([0, 1, 2, 1])).toEqual(["", "├─ ", "│  └─ ", "└─ "]);
  });

  it("pads with spaces under a finished branch", () => {
    expect(treePrefixes([0, 1, 2])).toEqual(["", "└─ ", "   └─ "]);
  });

  it("resets between roots", () => {
    expect(treePrefixes([0, 1, 0, 1])).toEqual(["", "└─ ", "", "└─ "]);
  });
});
